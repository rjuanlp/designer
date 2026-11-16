//! Offscreen Vello smoke test. This does not validate GPUI composition or FPS.
use anyhow::{Context, Result, ensure};
use designer_render::{scene_at, tokens};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, peniko::Color, wgpu};

fn main() -> Result<()> {
    pollster::block_on(run())
}

async fn run() -> Result<()> {
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
    println!(
        "Adapter: {} ({:?}); driver: {}",
        info.name, info.backend, info.driver_info
    );
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .context("GPU device creation failed")?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Designer Vello probe"),
        size: wgpu::Extent3d {
            width: tokens::WIDTH,
            height: tokens::HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = Renderer::new(&device, RendererOptions::default())?;
    let [r, g, b] = tokens::BACKGROUND;
    for frame in 0..120 {
        let scene = scene_at(f64::from(frame) / 60.0).context("Invalid probe time")?;
        renderer.render_to_texture(
            &device,
            &queue,
            &scene,
            &view,
            &RenderParams {
                base_color: Color::from_rgb8(r, g, b),
                width: tokens::WIDTH,
                height: tokens::HEIGHT,
                antialiasing_method: AaConfig::Area,
            },
        )?;
    }
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(30)),
    })?;
    println!(
        "120 offscreen frames submitted and completed. GPUI composition and frame pacing are NOT tested."
    );
    Ok(())
}
