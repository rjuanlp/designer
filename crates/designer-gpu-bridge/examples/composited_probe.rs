//! Integrated graphics gate probe: Vello content composited inside a GPUI
//! window through shared GPU textures, with no CPU readback.
//!
//! Run with `--translucent` to draw a transparent canvas with a half-alpha band
//! over a colored GPUI backdrop, which checks premultiplied-alpha blending.
//!
//! Run with `--frames N` to close automatically after N canvas frames and
//! print pacing figures. Those figures describe this small probe scene only;
//! they are not the project benchmark.

#[cfg(windows)]
mod windows_probe {
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    use anyhow::{Context as _, Result, ensure};
    use designer_gpu_bridge::{SharedCanvas, SharedFrame};
    use designer_render::{scene_at, tokens as scene_tokens};
    use gpui::{
        App, Application, Bounds, Context, ExternalFencePoint, ExternalTexture, Pixels, Window,
        WindowBounds, WindowOptions, canvas, deferred, div, prelude::*, px, rgb, rgba, size,
    };
    use vello::{
        AaConfig, RenderParams, Renderer, RendererOptions, Scene,
        kurbo::{Affine, Rect},
        peniko::{Color, Fill},
        wgpu,
    };

    mod tokens {
        pub const WIDTH: f32 = 960.0;
        pub const HEIGHT: f32 = 640.0;
        pub const BACKGROUND: u32 = 0x18191d;
        pub const FOREGROUND: u32 = 0xf2f4f8;
        pub const MUTED: u32 = 0x9aa0ac;
        pub const OVERLAY: u32 = 0x18191dcc;
        pub const OVERLAY_BORDER: u32 = 0x6957eb;
        /// GPUI color painted behind the canvas in the translucent check.
        pub const BACKDROP: u32 = 0x1f6f5c;
        /// Straight-alpha color of the middle band in the translucent check.
        pub const HALF_WHITE: [u8; 4] = [255, 255, 255, 128];
        pub const GAP: f32 = 16.0;
        pub const OVERLAY_INSET: f32 = 24.0;
        /// The canvas is shifted this far outside its container so the
        /// container's clip is visibly applied to the shared texture.
        pub const CLIP_OVERHANG: f32 = 96.0;
        /// Old canvases are kept this many frames after a resize so frames
        /// already handed to the compositor can still be opened.
        pub const RETIRED_CANVAS_FRAMES: u32 = 8;
    }

    struct CanvasState {
        device: wgpu::Device,
        queue: wgpu::Queue,
        renderer: Renderer,
        canvas: Option<SharedCanvas>,
        retired: Vec<(SharedCanvas, u32)>,
        started: Instant,
        last_paint: Option<Instant>,
        intervals: Vec<Duration>,
        work: Vec<Duration>,
        rendered: u32,
        skipped: u32,
        recreated: u32,
        frame_limit: Option<u32>,
        translucent: bool,
        reported: bool,
    }

    impl CanvasState {
        fn new(options: Options) -> Result<Self> {
            let (device, queue, adapter) = pollster::block_on(create_device())?;
            println!("Vello adapter: {adapter}");
            let renderer = Renderer::new(&device, RendererOptions::default())?;
            Ok(Self {
                device,
                queue,
                renderer,
                canvas: None,
                retired: Vec::new(),
                started: Instant::now(),
                last_paint: None,
                intervals: Vec::new(),
                work: Vec::new(),
                rendered: 0,
                skipped: 0,
                recreated: 0,
                frame_limit: options.frame_limit,
                translucent: options.translucent,
                reported: false,
            })
        }

        /// Render the next frame for a target of the given physical size.
        fn next_frame(&mut self, width: u32, height: u32) -> Result<Option<SharedFrame>> {
            let began = Instant::now();
            if let Some(previous) = self.last_paint.replace(began) {
                self.intervals.push(began - previous);
            }
            self.retired.retain_mut(|(_, frames_left)| {
                *frames_left = frames_left.saturating_sub(1);
                *frames_left > 0
            });
            let stale = self
                .canvas
                .as_ref()
                .is_some_and(|canvas| canvas.width() != width || canvas.height() != height);
            if stale && let Some(old) = self.canvas.take() {
                self.retired.push((old, tokens::RETIRED_CANVAS_FRAMES));
            }
            if self.canvas.is_none() {
                self.canvas = Some(SharedCanvas::new(&self.device, width, height)?);
                self.recreated += 1;
            }
            let canvas = self.canvas.as_mut().context("Shared canvas missing")?;

            let (scene, base_color) = if self.translucent {
                (alpha_bands(width, height), Color::TRANSPARENT)
            } else {
                let seconds = self.started.elapsed().as_secs_f64();
                let content = scene_at(seconds).context("Invalid probe time")?;
                let mut scene = Scene::new();
                scene.append(
                    &content,
                    Some(Affine::scale_non_uniform(
                        f64::from(width) / f64::from(scene_tokens::WIDTH),
                        f64::from(height) / f64::from(scene_tokens::HEIGHT),
                    )),
                );
                let [r, g, b] = scene_tokens::BACKGROUND;
                (scene, Color::from_rgb8(r, g, b))
            };
            let params = RenderParams {
                base_color,
                width,
                height,
                antialiasing_method: AaConfig::Area,
            };
            let (device, queue, renderer) = (&self.device, &self.queue, &mut self.renderer);
            let frame = canvas.render(queue, |view| {
                renderer
                    .render_to_texture(device, queue, &scene, view, &params)
                    .map_err(Into::into)
            })?;
            // Lets wgpu recycle finished resources. Never blocks.
            self.device.poll(wgpu::PollType::Poll)?;
            let frame = match frame {
                Some(frame) => {
                    self.rendered += 1;
                    Some(frame)
                }
                None => {
                    self.skipped += 1;
                    canvas.latest()
                }
            };
            self.work.push(began.elapsed());
            Ok(frame)
        }

        fn finished(&self) -> bool {
            self.frame_limit.is_some_and(|limit| self.rendered >= limit)
        }

        fn report(&mut self) {
            if std::mem::replace(&mut self.reported, true) {
                return;
            }
            println!(
                "Canvas frames rendered: {}; skipped waiting for compositor: {}; canvases created: {}",
                self.rendered, self.skipped, self.recreated
            );
            // Skip the first frames: they include shader and pipeline setup.
            let warmup = 30.min(self.intervals.len() / 2);
            print_distribution("Frame interval", &self.intervals[warmup..]);
            print_distribution(
                "UI-thread canvas work",
                &self.work[warmup.min(self.work.len())..],
            );
            println!(
                "These figures cover only this probe scene. They are not the 10,000-node benchmark."
            );
        }
    }

    fn print_distribution(label: &str, samples: &[Duration]) {
        if samples.is_empty() {
            println!("{label}: no samples");
            return;
        }
        let mut sorted = samples.to_vec();
        sorted.sort();
        let at = |fraction: f64| sorted[((sorted.len() - 1) as f64 * fraction).round() as usize];
        let mean = sorted.iter().sum::<Duration>() / sorted.len() as u32;
        println!(
            "{label}: n={} mean={:.2}ms p50={:.2}ms p95={:.2}ms p99={:.2}ms max={:.2}ms",
            sorted.len(),
            mean.as_secs_f64() * 1e3,
            at(0.50).as_secs_f64() * 1e3,
            at(0.95).as_secs_f64() * 1e3,
            at(0.99).as_secs_f64() * 1e3,
            at(1.0).as_secs_f64() * 1e3,
        );
    }

    async fn create_device() -> Result<(wgpu::Device, wgpu::Queue, String)> {
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
            .context("No compatible DX12 GPU adapter")?;
        let info = adapter.get_info();
        ensure!(
            info.device_type != wgpu::DeviceType::Cpu,
            "Software adapter is not hardware validation"
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .context("GPU device creation failed")?;
        Ok((device, queue, format!("{} ({:?})", info.name, info.backend)))
    }

    fn to_external(frame: SharedFrame) -> ExternalTexture {
        ExternalTexture {
            id: frame.texture_id,
            shared_handle: frame.texture_handle,
            wait: Some(ExternalFencePoint {
                fence_id: frame.ready.fence_id,
                shared_handle: frame.ready.shared_handle,
                value: frame.ready.value,
            }),
            signal: Some(ExternalFencePoint {
                fence_id: frame.released.fence_id,
                shared_handle: frame.released.shared_handle,
                value: frame.released.value,
            }),
        }
    }

    /// Three vertical bands: fully transparent, half-alpha white, opaque accent.
    fn alpha_bands(width: u32, height: u32) -> Scene {
        let (width, height) = (f64::from(width), f64::from(height));
        let third = width / 3.0;
        let mut scene = Scene::new();
        let [r, g, b, a] = tokens::HALF_WHITE;
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(r, g, b, a),
            None,
            &Rect::new(third, 0.0, 2.0 * third, height),
        );
        let [r, g, b] = scene_tokens::ACCENT;
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgb8(r, g, b),
            None,
            &Rect::new(2.0 * third, 0.0, width, height),
        );
        scene
    }

    struct Probe {
        translucent: bool,
        state: Rc<RefCell<CanvasState>>,
    }

    impl Render for Probe {
        fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            window.request_animation_frame();
            let state = self.state.clone();
            let translucent = self.translucent;
            // The alpha check samples known positions, so it does not shift.
            let overhang = if translucent {
                0.0
            } else {
                tokens::CLIP_OVERHANG
            };
            let hint = if translucent {
                "Bandas: transparente, blanco al 50 % y color opaco sobre un fondo de la interfaz."
            } else {
                "El lienzo está desplazado: la hoja debe verse recortada arriba y a la izquierda."
            };
            let vello_canvas = canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, (), window: &mut Window, cx: &mut App| {
                    let scale = window.scale_factor();
                    let width = (f32::from(bounds.size.width) * scale).round().max(1.0) as u32;
                    let height = (f32::from(bounds.size.height) * scale).round().max(1.0) as u32;
                    let mut state = state.borrow_mut();
                    match state.next_frame(width, height) {
                        Ok(Some(frame)) => {
                            window.paint_external_texture(bounds, to_external(frame));
                        }
                        Ok(None) => {}
                        Err(error) => {
                            eprintln!("Canvas frame failed: {error:#}");
                            state.report();
                            cx.quit();
                        }
                    }
                    if state.finished() {
                        state.report();
                        cx.quit();
                    }
                },
            )
            .absolute()
            .top(px(-overhang))
            .left(px(-overhang))
            .size_full();

            div()
                .size_full()
                .flex()
                .flex_col()
                .gap(px(tokens::GAP))
                .p(px(tokens::GAP))
                .bg(rgb(tokens::BACKGROUND))
                .text_color(rgb(tokens::FOREGROUND))
                .child("Designer · Prueba de composición Vello en GPUI")
                .child(
                    div()
                        .flex_1()
                        .relative()
                        .overflow_hidden()
                        .when(translucent, |backdrop| backdrop.bg(rgb(tokens::BACKDROP)))
                        .child(vello_canvas)
                        .child(
                            div()
                                .absolute()
                                .top(px(tokens::OVERLAY_INSET))
                                .left(px(tokens::OVERLAY_INSET))
                                .p(px(tokens::GAP))
                                .bg(rgba(tokens::OVERLAY))
                                .border_1()
                                .border_color(rgb(tokens::OVERLAY_BORDER))
                                .child("Panel de la interfaz sobre el lienzo"),
                        )
                        // Deferred elements (menus, tooltips) paint in a later
                        // pass; this one must still land above the canvas.
                        .child(
                            deferred(
                                div()
                                    .absolute()
                                    .bottom(px(tokens::OVERLAY_INSET))
                                    .right(px(tokens::OVERLAY_INSET))
                                    .p(px(tokens::GAP))
                                    .bg(rgba(tokens::OVERLAY))
                                    .border_1()
                                    .border_color(rgb(tokens::OVERLAY_BORDER))
                                    .child("Elemento diferido sobre el lienzo"),
                            )
                            .with_priority(1),
                        ),
                )
                .child(div().text_color(rgb(tokens::MUTED)).child(hint))
        }
    }

    /// Prints warnings and errors from GPUI, which reports renderer failures
    /// through the `log` facade and would otherwise stay silent.
    struct StderrLogger;

    impl log::Log for StderrLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Warn
        }

        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                eprintln!("[{}] {}", record.level(), record.args());
            }
        }

        fn flush(&self) {}
    }

    pub fn run() -> Result<()> {
        if log::set_logger(&StderrLogger).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
        let options = parse_options()?;
        let translucent = options.translucent;
        let state = Rc::new(RefCell::new(CanvasState::new(options)?));
        Application::new().run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(tokens::WIDTH), px(tokens::HEIGHT)), cx);
            let report_state = state.clone();
            if let Err(error) = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("Designer · Composited probe".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Probe { translucent, state }),
            ) {
                eprintln!("Window creation failed: {error}");
                cx.quit();
                return;
            }
            cx.activate(true);
            cx.on_window_closed(move |cx| {
                if cx.windows().is_empty() {
                    report_state.borrow_mut().report();
                    cx.quit();
                }
            })
            .detach();
        });
        Ok(())
    }

    struct Options {
        frame_limit: Option<u32>,
        translucent: bool,
    }

    fn parse_options() -> Result<Options> {
        let mut options = Options {
            frame_limit: None,
            translucent: false,
        };
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--translucent" => options.translucent = true,
                "--frames" => {
                    let value = arguments.next().context("--frames needs a number")?;
                    let frames: u32 = value.parse().context("--frames needs a positive integer")?;
                    ensure!(frames > 0, "--frames needs a positive integer");
                    options.frame_limit = Some(frames);
                }
                other => anyhow::bail!("Unknown argument: {other}"),
            }
        }
        Ok(options)
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    windows_probe::run()?;
    #[cfg(not(windows))]
    eprintln!("The composited probe currently targets Windows only.");
    Ok(())
}
