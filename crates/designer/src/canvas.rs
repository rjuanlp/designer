//! Renders a document page with Vello into shared textures for GPUI.
//!
//! Rendering is on demand: a frame is produced only when the content
//! generation, the view or the target size changes. Otherwise the latest
//! frame is painted again.
//!
//! The view is fitted to the page content once and then kept, so the content
//! does not jump while it is being edited.

use anyhow::{Context as _, Result, ensure};
use designer_core::{NodeId, Page, PageId};
use designer_gpu_bridge::{SharedCanvas, SharedFrame};
use designer_render::document::{ViewTransform, content_bounds, fit_view, page_scene};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, peniko::Color, wgpu};

use crate::theme::metrics;

/// Old canvases are kept for this many frame requests after a resize, so
/// frames already handed to the compositor can still be opened.
const RETIRED_CANVAS_REQUESTS: u32 = 8;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    adapter: String,
}

/// Smallest zoom: 5%.
pub const MIN_ZOOM: f64 = 0.05;
/// Largest zoom: 6400%.
pub const MAX_ZOOM: f64 = 64.0;

/// Everything that determines the pixels of a frame.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FrameKey {
    page: PageId,
    generation: u64,
    view_epoch: u64,
    view_revision: u64,
    width: u32,
    height: u32,
}

/// What the canvas element should do after asking for a frame.
pub struct CanvasFrame {
    /// The frame to paint, if any exists yet.
    pub frame: Option<SharedFrame>,
    /// The requested content could not be rendered yet because the compositor
    /// has not released a target; ask again on the next frame.
    pub retry: bool,
}

/// Owns the GPU device, the Vello renderer and the shared targets.
pub struct CanvasHost {
    gpu: Result<Gpu, String>,
    canvas: Option<SharedCanvas>,
    retired: Vec<(SharedCanvas, u32)>,
    rendered: Option<FrameKey>,
    view: Option<(u64, PageId, ViewTransform)>,
    placement: Option<Placement>,
    /// Changes whenever the view is zoomed, panned or fitted by the user.
    view_revision: u64,
}

/// Where the canvas element is in the window.
#[derive(Clone, Copy)]
struct Placement {
    /// Left edge in logical window pixels.
    x: f32,
    /// Top edge in logical window pixels.
    y: f32,
    /// Physical pixels per logical pixel.
    scale: f32,
    /// Size of the canvas in physical pixels.
    width: u32,
    /// Size of the canvas in physical pixels.
    height: u32,
}

impl CanvasHost {
    /// Create the GPU device. A failure is kept and reported through
    /// [`Self::error`] instead of stopping the application.
    pub fn new() -> Self {
        Self {
            gpu: pollster::block_on(create_gpu()).map_err(|error| format!("{error:#}")),
            canvas: None,
            retired: Vec::new(),
            rendered: None,
            view: None,
            placement: None,
            view_revision: 0,
        }
    }

    /// Record where the canvas element was painted, for [`Self::page_point`].
    pub fn set_placement(&mut self, x: f32, y: f32, scale: f32, width: u32, height: u32) {
        self.placement = Some(Placement {
            x,
            y,
            scale,
            width,
            height,
        });
    }

    /// The page point under a position in logical window pixels, once the
    /// canvas has been painted.
    pub fn page_point(&self, window_x: f32, window_y: f32) -> Option<(f64, f64)> {
        let (placement, (_, _, view)) = (self.placement?, self.view?);
        Some(view.to_page(
            f64::from((window_x - placement.x) * placement.scale),
            f64::from((window_y - placement.y) * placement.scale),
        ))
    }

    /// Current zoom, where 1.0 shows one document unit per logical pixel.
    pub fn zoom(&self) -> Option<f64> {
        let (placement, (_, _, view)) = (self.placement?, self.view?);
        Some(view.scale / f64::from(placement.scale))
    }

    /// Multiply the zoom, keeping the page point under a position in logical
    /// window pixels where it is. Without a position the canvas center stays.
    pub fn zoom_by(&mut self, factor: f64, position: Option<(f32, f32)>) -> bool {
        let Some(zoom) = self.zoom() else {
            return false;
        };
        self.zoom_to(zoom * factor, position)
    }

    /// Set the zoom; see [`Self::zoom_by`] for the fixed point.
    pub fn zoom_to(&mut self, zoom: f64, position: Option<(f32, f32)>) -> bool {
        let (Some(placement), Some((epoch, page, mut view))) = (self.placement, self.view) else {
            return false;
        };
        if !zoom.is_finite() {
            return false;
        }
        let dpi = f64::from(placement.scale);
        let (x, y) = match position {
            Some((x, y)) => (
                f64::from(x - placement.x) * dpi,
                f64::from(y - placement.y) * dpi,
            ),
            None => (
                f64::from(placement.width) / 2.0,
                f64::from(placement.height) / 2.0,
            ),
        };
        let before = view;
        view.zoom_about(zoom.clamp(MIN_ZOOM, MAX_ZOOM) * dpi, x, y);
        self.set_view(epoch, page, view, before)
    }

    /// Move the view by a distance in logical pixels.
    pub fn pan(&mut self, dx: f32, dy: f32) -> bool {
        let (Some(placement), Some((epoch, page, mut view))) = (self.placement, self.view) else {
            return false;
        };
        let before = view;
        view.offset_x += f64::from(dx * placement.scale);
        view.offset_y += f64::from(dy * placement.scale);
        self.set_view(epoch, page, view, before)
    }

    /// Fit the page content in the canvas. Before the first frame there is
    /// no canvas size yet and the first frame fits by itself.
    pub fn fit(&mut self, page: &Page) {
        let (Some(placement), Some((epoch, _, _))) = (self.placement, self.view) else {
            return;
        };
        let view = fitted_view(page, placement.width, placement.height);
        self.view = Some((epoch, page.id, view));
        self.view_revision += 1;
    }

    fn set_view(
        &mut self,
        epoch: u64,
        page: PageId,
        view: ViewTransform,
        before: ViewTransform,
    ) -> bool {
        let usable = [view.scale, view.offset_x, view.offset_y]
            .iter()
            .all(|value| value.is_finite());
        if !usable || view == before {
            return false;
        }
        self.view = Some((epoch, page, view));
        self.view_revision += 1;
        true
    }

    /// Name of the adapter used for rendering.
    pub fn adapter(&self) -> Option<&str> {
        self.gpu.as_ref().ok().map(|gpu| gpu.adapter.as_str())
    }

    /// Why the canvas cannot render, if it cannot.
    pub fn error(&self) -> Option<&str> {
        self.gpu.as_ref().err().map(String::as_str)
    }

    /// Get the frame for a page at a physical size, rendering it if needed.
    ///
    /// `generation` must change whenever the page content or the selection
    /// changes. `view_epoch` must change when the view should be fitted to
    /// the content again, as after opening a document.
    ///
    /// An error disables the canvas; it is then reported by [`Self::error`].
    pub fn frame(
        &mut self,
        page: &Page,
        selection: Option<NodeId>,
        generation: u64,
        view_epoch: u64,
        width: u32,
        height: u32,
    ) -> Result<CanvasFrame> {
        let result = self.frame_inner(page, selection, generation, view_epoch, width, height);
        if let Err(error) = &result {
            self.gpu = Err(format!("{error:#}"));
            self.canvas = None;
            self.rendered = None;
        }
        result
    }

    fn frame_inner(
        &mut self,
        page: &Page,
        selection: Option<NodeId>,
        generation: u64,
        view_epoch: u64,
        width: u32,
        height: u32,
    ) -> Result<CanvasFrame> {
        let gpu = match &mut self.gpu {
            Ok(gpu) => gpu,
            Err(message) => anyhow::bail!("{message}"),
        };
        self.retired.retain_mut(|(_, requests_left)| {
            *requests_left = requests_left.saturating_sub(1);
            *requests_left > 0
        });
        let stale = self
            .canvas
            .as_ref()
            .is_some_and(|canvas| canvas.width() != width || canvas.height() != height);
        if stale && let Some(old) = self.canvas.take() {
            self.retired.push((old, RETIRED_CANVAS_REQUESTS));
            self.rendered = None;
        }
        if self.canvas.is_none() {
            self.canvas = Some(SharedCanvas::new(&gpu.device, width, height)?);
        }
        let canvas = self.canvas.as_mut().context("Shared canvas missing")?;

        let key = FrameKey {
            page: page.id,
            generation,
            view_epoch,
            view_revision: self.view_revision,
            width,
            height,
        };
        if self.rendered == Some(key)
            && let Some(frame) = canvas.latest()
        {
            return Ok(CanvasFrame {
                frame: Some(frame),
                retry: false,
            });
        }

        let view = match self.view {
            Some((epoch, id, view)) if epoch == view_epoch && id == page.id => view,
            _ => {
                let view = fitted_view(page, width, height);
                self.view = Some((view_epoch, page.id, view));
                view
            }
        };
        let scene = page_scene(page, view, selection);
        let background = page.background;
        let params = RenderParams {
            base_color: Color::from_rgba8(background.r, background.g, background.b, background.a),
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        let (device, queue, renderer) = (&gpu.device, &gpu.queue, &mut gpu.renderer);
        let frame = canvas.render(queue, |view| {
            renderer
                .render_to_texture(device, queue, &scene, view, &params)
                .map_err(Into::into)
        })?;
        // Lets wgpu recycle finished resources. Never blocks.
        gpu.device.poll(wgpu::PollType::Poll)?;
        Ok(match frame {
            Some(frame) => {
                self.rendered = Some(key);
                CanvasFrame {
                    frame: Some(frame),
                    retry: false,
                }
            }
            None => CanvasFrame {
                frame: canvas.latest(),
                retry: true,
            },
        })
    }
}

async fn create_gpu() -> Result<Gpu> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        })
        .await
        .context("No compatible Direct3D 12 adapter was found")?;
    let info = adapter.get_info();
    ensure!(
        info.device_type != wgpu::DeviceType::Cpu,
        "Only a software adapter is available"
    );
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .context("The GPU device could not be created")?;
    let renderer = Renderer::new(&device, RendererOptions::default())
        .context("The Vello renderer could not be created")?;
    Ok(Gpu {
        device,
        queue,
        renderer,
        adapter: info.name,
    })
}

/// The view that shows all the content of a page in a canvas of a physical
/// size.
fn fitted_view(page: &Page, width: u32, height: u32) -> ViewTransform {
    content_bounds(page)
        .map(|bounds| {
            fit_view(
                bounds,
                f64::from(width),
                f64::from(height),
                metrics::CANVAS_MARGIN,
            )
        })
        // An empty page starts with its origin inside the margin.
        .unwrap_or(ViewTransform {
            scale: 1.0,
            offset_x: metrics::CANVAS_MARGIN,
            offset_y: metrics::CANVAS_MARGIN,
        })
}
