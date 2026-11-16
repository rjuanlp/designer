//! DESIGNER PATCH: opens textures and fences shared by another Direct3D device
//! so the window renderer can sample them in scene order.
//!
//! This file is not part of upstream GPUI 0.2.2. See `vendor/README.md` in the
//! Designer repository.

use anyhow::{Context, Result};
use collections::{FxHashMap, FxHashSet};
use windows::{
    Win32::{Foundation::HANDLE, Graphics::Direct3D11::*},
    core::Interface,
};

use crate::{ExternalFencePoint, ExternalTexture};

/// Opened objects are released after this many frames without being painted.
const UNUSED_FRAMES_BEFORE_RELEASE: u64 = 120;
/// Upper bound for remembered failures, so a misbehaving producer cannot grow
/// the set without limit.
const MAX_REPORTED_FAILURES: usize = 1024;

struct OpenedTexture {
    view: [Option<ID3D11ShaderResourceView>; 1],
    width: u32,
    height: u32,
    last_used: u64,
}

struct OpenedFence {
    fence: ID3D11Fence,
    last_used: u64,
}

/// A texture view that is ready to be bound, with its real dimensions.
pub(crate) struct PreparedExternalTexture {
    pub(crate) view: [Option<ID3D11ShaderResourceView>; 1],
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Default)]
pub(crate) struct ExternalTextureCache {
    frame: u64,
    textures: FxHashMap<u64, OpenedTexture>,
    fences: FxHashMap<u64, OpenedFence>,
    reported_failures: FxHashSet<u64>,
}

impl ExternalTextureCache {
    pub(crate) fn begin_frame(&mut self) {
        self.frame += 1;
    }

    /// Release objects that were not painted recently.
    pub(crate) fn end_frame(&mut self) {
        let frame = self.frame;
        let is_recent = |last_used: u64| last_used + UNUSED_FRAMES_BEFORE_RELEASE >= frame;
        self.textures.retain(|_, texture| is_recent(texture.last_used));
        self.fences.retain(|_, fence| is_recent(fence.last_used));
    }

    /// Drop every opened object. They belong to a device, so this must run
    /// before that device is released after a device loss.
    pub(crate) fn clear(&mut self) {
        self.textures.clear();
        self.fences.clear();
        self.reported_failures.clear();
    }

    /// Returns true the first time a failure for `id` is seen, so callers log
    /// a broken texture once instead of on every frame.
    pub(crate) fn should_report_failure(&mut self, id: u64) -> bool {
        if self.reported_failures.len() >= MAX_REPORTED_FAILURES {
            self.reported_failures.clear();
        }
        self.reported_failures.insert(id)
    }

    /// Open (or reuse) the shared texture and queue the producer wait.
    pub(crate) fn prepare(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        texture: &ExternalTexture,
    ) -> Result<PreparedExternalTexture> {
        let frame = self.frame;
        if !self.textures.contains_key(&texture.id) {
            let opened = open_texture(device, texture.shared_handle, frame)?;
            self.textures.insert(texture.id, opened);
        }
        if let Some(point) = &texture.wait {
            let fence = self.fence(device, point)?;
            let context: ID3D11DeviceContext4 = device_context
                .cast()
                .context("Direct3D 11.3 context is required for shared fences")?;
            // Queues a GPU-side wait: later commands on this context run only
            // after the producer has signaled. The CPU does not block here.
            unsafe { context.Wait(&fence, point.value) }
                .context("Queueing wait on shared fence")?;
        }
        let opened = self
            .textures
            .get_mut(&texture.id)
            .context("External texture disappeared from the cache")?;
        opened.last_used = frame;
        Ok(PreparedExternalTexture {
            view: opened.view.clone(),
            width: opened.width,
            height: opened.height,
        })
    }

    /// Queue the consumer signal after the sampling draw call.
    pub(crate) fn finish(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        texture: &ExternalTexture,
    ) -> Result<()> {
        let Some(point) = &texture.signal else {
            return Ok(());
        };
        let fence = self.fence(device, point)?;
        let context: ID3D11DeviceContext4 = device_context
            .cast()
            .context("Direct3D 11.3 context is required for shared fences")?;
        unsafe { context.Signal(&fence, point.value) }.context("Queueing signal on shared fence")
    }

    fn fence(&mut self, device: &ID3D11Device, point: &ExternalFencePoint) -> Result<ID3D11Fence> {
        let frame = self.frame;
        if let Some(opened) = self.fences.get_mut(&point.fence_id) {
            opened.last_used = frame;
            return Ok(opened.fence.clone());
        }
        let device: ID3D11Device5 = device
            .cast()
            .context("Direct3D 11.4 device is required for shared fences")?;
        let mut fence: Option<ID3D11Fence> = None;
        // The handle is only read by the kernel, which validates it; an invalid
        // or wrongly typed handle produces an error instead of undefined behavior.
        unsafe { device.OpenSharedFence(handle(point.shared_handle), &mut fence) }
            .context("Opening shared fence")?;
        let fence = fence.context("Shared fence was not returned")?;
        self.fences.insert(
            point.fence_id,
            OpenedFence {
                fence: fence.clone(),
                last_used: frame,
            },
        );
        Ok(fence)
    }
}

fn handle(raw: isize) -> HANDLE {
    HANDLE(raw as *mut core::ffi::c_void)
}

fn open_texture(device: &ID3D11Device, shared_handle: isize, frame: u64) -> Result<OpenedTexture> {
    let device: ID3D11Device1 = device
        .cast()
        .context("Direct3D 11.1 device is required for shared textures")?;
    // The handle is only read by the kernel, which validates it; an invalid or
    // wrongly typed handle produces an error instead of undefined behavior.
    let texture: ID3D11Texture2D = unsafe { device.OpenSharedResource1(handle(shared_handle)) }
        .context("Opening shared texture (is it on the same adapter?)")?;
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    let mut view = None;
    unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut view)) }
        .context("Creating view for shared texture")?;
    Ok(OpenedTexture {
        view: [view],
        width: desc.Width,
        height: desc.Height,
        last_used: frame,
    })
}
