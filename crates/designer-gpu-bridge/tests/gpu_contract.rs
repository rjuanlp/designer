//! Hardware contract tests for the shared canvas. They need a Direct3D 12
//! adapter, so they are ignored by default; `scripts/verify.ps1 -Gpu` runs them.
#![cfg(windows)]

use designer_gpu_bridge::{BridgeError, SharedCanvas};
use designer_render::scene_at;
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, peniko::Color, wgpu};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;

fn device() -> (wgpu::Device, wgpu::Queue) {
    pollster::block_on(async {
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
            .expect("a DX12 adapter");
        adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("a DX12 device")
    })
}

#[test]
#[ignore = "requires a hardware Direct3D 12 adapter"]
fn rejects_sizes_the_device_cannot_allocate() {
    let (device, _) = device();
    for (width, height) in [(0, HEIGHT), (WIDTH, 0), (u32::MAX, HEIGHT)] {
        assert!(matches!(
            SharedCanvas::new(&device, width, height),
            Err(BridgeError::InvalidSize { .. })
        ));
    }
}

#[test]
#[ignore = "requires a hardware Direct3D 12 adapter"]
fn stops_rendering_when_no_compositor_releases_targets() {
    let (device, queue) = device();
    let mut renderer = Renderer::new(&device, RendererOptions::default()).expect("renderer");
    let mut canvas = SharedCanvas::new(&device, WIDTH, HEIGHT).expect("shared canvas");
    assert!(canvas.latest().is_none());

    let scene = scene_at(0.0).expect("valid time");
    let params = RenderParams {
        base_color: Color::BLACK,
        width: WIDTH,
        height: HEIGHT,
        antialiasing_method: AaConfig::Area,
    };
    let mut seen = Vec::new();
    // Nothing ever signals the release fence here, so only the unused targets
    // can be handed out. After that the canvas must refuse to overwrite
    // frames a compositor could still be reading, without blocking.
    loop {
        let frame = canvas
            .render(&queue, |view| {
                renderer
                    .render_to_texture(&device, &queue, &scene, view, &params)
                    .map_err(Into::into)
            })
            .expect("render");
        let Some(frame) = frame else { break };
        assert_eq!(frame.generation, seen.len() as u64 + 1);
        assert_eq!(frame.ready.value, frame.generation);
        assert_eq!(frame.released.value, frame.generation);
        assert!(!seen.contains(&frame.texture_id), "a target was reused");
        assert_eq!(canvas.latest(), Some(frame));
        seen.push(frame.texture_id);
        assert!(seen.len() <= 16, "backpressure never engaged");
    }
    assert!(seen.len() >= 2, "expected more than one target");

    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .expect("GPU work completes");
}

#[test]
#[ignore = "requires a hardware Direct3D 12 adapter"]
fn failed_draw_is_reported_and_not_published() {
    let (device, queue) = device();
    let mut canvas = SharedCanvas::new(&device, WIDTH, HEIGHT).expect("shared canvas");
    let result = canvas.render(&queue, |_| Err("draw failed".into()));
    assert!(matches!(result, Err(BridgeError::Render(_))));
    assert!(canvas.latest().is_none());
}
