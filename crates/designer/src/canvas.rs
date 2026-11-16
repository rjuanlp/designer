//! Renders a document page with Vello into shared textures for GPUI.
//!
//! Rendering is on demand: a frame is produced only when the page, the
//! selection or the target size changes. Otherwise the latest frame is
//! painted again.

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

/// Everything that determines the pixels of a frame.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FrameKey {
    page: PageId,
    selection: Option<NodeId>,
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
        }
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
    /// An error disables the canvas; it is then reported by [`Self::error`].
    pub fn frame(
        &mut self,
        page: &Page,
        selection: Option<NodeId>,
        width: u32,
        height: u32,
    ) -> Result<CanvasFrame> {
        let result = self.frame_inner(page, selection, width, height);
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
            selection,
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

        let view = content_bounds(page)
            .map(|bounds| {
                fit_view(
                    bounds,
                    f64::from(width),
                    f64::from(height),
                    metrics::CANVAS_MARGIN,
                )
            })
            .unwrap_or(ViewTransform::IDENTITY);
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
