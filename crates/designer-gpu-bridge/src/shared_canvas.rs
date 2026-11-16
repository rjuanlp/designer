//! A small ring of shared render targets with fence-based hand-over.

use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

use vello::wgpu::{self, hal::api::Dx12};
use windows::{
    Win32::{
        Foundation::{CloseHandle, GENERIC_ALL, HANDLE},
        Graphics::{
            Direct3D12::{
                D3D12_FENCE_FLAG_SHARED, D3D12_HEAP_FLAG_SHARED, D3D12_HEAP_PROPERTIES,
                D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DESC, D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS,
                D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COMMON,
                D3D12_TEXTURE_LAYOUT_UNKNOWN, ID3D12Device, ID3D12Fence, ID3D12Resource,
            },
            Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
        },
    },
    core::PCWSTR,
};

/// Pixel format of every shared target. Vello requires this storage format.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Three targets let one be displayed, one be in flight on the compositor and
/// one be written, so rendering rarely has to skip a frame.
const SLOT_COUNT: usize = 3;

/// Process-wide identities, so the compositor never confuses a new object with
/// a closed one whose numeric handle value was reused by the system.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Errors reported by the bridge.
#[derive(Debug)]
pub enum BridgeError {
    /// The wgpu device or queue is not backed by Direct3D 12.
    NotDirectX12,
    /// A target dimension is zero or exceeds the Direct3D 12 texture limit.
    InvalidSize {
        /// Requested width in physical pixels.
        width: u32,
        /// Requested height in physical pixels.
        height: u32,
    },
    /// A Direct3D call failed. The message names the failed operation.
    Graphics(String),
    /// The caller's drawing closure failed.
    Render(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDirectX12 => write!(f, "the wgpu device is not a Direct3D 12 device"),
            Self::InvalidSize { width, height } => {
                write!(f, "invalid shared target size {width}x{height}")
            }
            Self::Graphics(message) => write!(f, "{message}"),
            Self::Render(error) => write!(f, "rendering into the shared target failed: {error}"),
        }
    }
}

impl Error for BridgeError {}

fn graphics(operation: &str) -> impl FnOnce(windows::core::Error) -> BridgeError + '_ {
    move |error| BridgeError::Graphics(format!("{operation}: {error}"))
}

/// An NT handle owned by this process and closed on drop.
struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn value(&self) -> isize {
        self.0.0 as isize
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by `CreateSharedHandle`, is owned
        // exclusively by this value and is closed exactly once here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// A value on one of the shared fences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FencePoint {
    /// Identity of the fence, unique within the process.
    pub fence_id: u64,
    /// NT shared handle value of the fence. Owned by the [`SharedCanvas`].
    pub shared_handle: isize,
    /// Fence value to wait for or to signal.
    pub value: u64,
}

/// A completed frame, described with plain values a compositor can consume.
///
/// The handles are owned by the [`SharedCanvas`] that produced the frame and
/// are valid only while that canvas is alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedFrame {
    /// Identity of the texture, unique within the process.
    pub texture_id: u64,
    /// NT shared handle value of the texture.
    pub texture_handle: isize,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Monotonic frame number within the canvas, starting at 1.
    pub generation: u64,
    /// Signaled by this side when the frame content is complete. The
    /// compositor must wait for it on its GPU queue before sampling.
    pub ready: FencePoint,
    /// The compositor must signal this after its last sampling command for
    /// the frame, so the texture can be written again.
    pub released: FencePoint,
}

struct SharedFence {
    id: u64,
    fence: ID3D12Fence,
    handle: OwnedHandle,
}

impl SharedFence {
    fn new(device: &ID3D12Device) -> Result<Self, BridgeError> {
        // SAFETY: `device` is a live Direct3D 12 device; the call only
        // allocates a new fence object.
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_SHARED) }
            .map_err(graphics("creating shared fence"))?;
        // SAFETY: `fence` was created on `device` with sharing enabled, and
        // null attributes/name are documented as valid.
        let handle =
            unsafe { device.CreateSharedHandle(&fence, None, GENERIC_ALL.0, PCWSTR::null()) }
                .map_err(graphics("sharing fence"))?;
        Ok(Self {
            id: next_id(),
            fence,
            handle: OwnedHandle(handle),
        })
    }

    fn point(&self, value: u64) -> FencePoint {
        FencePoint {
            fence_id: self.id,
            shared_handle: self.handle.value(),
            value,
        }
    }
}

struct Slot {
    id: u64,
    handle: OwnedHandle,
    view: wgpu::TextureView,
    /// Keeps the wgpu wrapper, and through it the Direct3D resource, alive.
    _texture: wgpu::Texture,
    /// Generation last rendered into this slot, if any.
    generation: Option<u64>,
}

/// A ring of shared render targets of one size.
///
/// Create a new canvas when the size changes, and keep the old one alive for a
/// few compositor frames so frames already handed over can still be opened.
pub struct SharedCanvas {
    width: u32,
    height: u32,
    slots: Vec<Slot>,
    /// This side signals; the compositor waits.
    ready: SharedFence,
    /// The compositor signals; this side polls.
    released: SharedFence,
    generation: u64,
    latest: Option<usize>,
    /// Highest released value observed. Guards against a compositor that
    /// redraws an old frame and signals a lower value again.
    max_released: u64,
    adapter_luid: i64,
}

impl SharedCanvas {
    /// Create the shared targets on the Direct3D 12 device behind `device`.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Result<Self, BridgeError> {
        let limit = device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(BridgeError::InvalidSize { width, height });
        }
        // SAFETY: the guard is only used to borrow the raw device for the
        // calls below. Nothing is destroyed through it, and every object
        // created here is owned by this canvas or by wgpu.
        let hal_device = unsafe { device.as_hal::<Dx12>() }.ok_or(BridgeError::NotDirectX12)?;
        let raw = hal_device.raw_device();

        // SAFETY: `raw` is a live device; this is a plain property read.
        let luid = unsafe { raw.GetAdapterLuid() };
        let adapter_luid = (i64::from(luid.HighPart) << 32) | i64::from(luid.LowPart);

        let ready = SharedFence::new(raw)?;
        let released = SharedFence::new(raw)?;
        let mut slots = Vec::with_capacity(SLOT_COUNT);
        for _ in 0..SLOT_COUNT {
            slots.push(Self::create_slot(device, raw, width, height)?);
        }
        Ok(Self {
            width,
            height,
            slots,
            ready,
            released,
            generation: 0,
            latest: None,
            max_released: 0,
            adapter_luid,
        })
    }

    fn create_slot(
        device: &wgpu::Device,
        raw: &ID3D12Device,
        width: u32,
        height: u32,
    ) -> Result<Slot, BridgeError> {
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: u64::from(width),
            Height: height,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            // Unordered access: Vello writes the target from a compute shader.
            // Simultaneous access: the Direct3D 11 compositor may hold the
            // texture as a shader resource while this device has it in
            // another state; the fences, not resource states, order content.
            Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS
                | D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET
                | D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS,
        };
        let mut resource: Option<ID3D12Resource> = None;
        // SAFETY: both descriptors are valid for the duration of the call and
        // `resource` is a valid out-pointer for the requested interface.
        unsafe {
            raw.CreateCommittedResource(
                &heap,
                D3D12_HEAP_FLAG_SHARED,
                &desc,
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )
        }
        .map_err(graphics("creating shared texture"))?;
        let resource = resource
            .ok_or_else(|| BridgeError::Graphics("shared texture was not returned".to_owned()))?;
        // SAFETY: `resource` was created on `raw` in a shared heap, and null
        // attributes/name are documented as valid.
        let handle =
            unsafe { raw.CreateSharedHandle(&resource, None, GENERIC_ALL.0, PCWSTR::null()) }
                .map_err(graphics("sharing texture"))?;
        let handle = OwnedHandle(handle);

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        // SAFETY: the format, dimension, size, mip count and sample count
        // passed here are exactly those of `resource` as created above.
        let hal_texture = unsafe {
            wgpu::hal::dx12::Device::texture_from_raw(
                resource,
                TARGET_FORMAT,
                wgpu::TextureDimension::D2,
                size,
                1,
                1,
            )
        };
        // SAFETY: `hal_texture` wraps a resource created on this device's raw
        // device; the descriptor matches it; the usages are covered by its
        // creation flags; and a committed resource created in the COMMON
        // state corresponds to wgpu's UNINITIALIZED use. Every slot is fully
        // overwritten by the renderer before it is ever sampled.
        let texture = unsafe {
            device.create_texture_from_hal::<Dx12>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("Designer shared canvas target"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TARGET_FORMAT,
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::TextureUses::UNINITIALIZED,
            )
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Slot {
            id: next_id(),
            handle,
            view,
            _texture: texture,
            generation: None,
        })
    }

    /// Width of every target in physical pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of every target in physical pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Locally unique identifier of the adapter that owns the targets. A
    /// compositor on a different adapter cannot open them.
    pub fn adapter_luid(&self) -> i64 {
        self.adapter_luid
    }

    /// The most recently completed frame, for repainting without rendering.
    pub fn latest(&self) -> Option<SharedFrame> {
        let slot = &self.slots[self.latest?];
        Some(self.frame(slot, slot.generation?))
    }

    fn frame(&self, slot: &Slot, generation: u64) -> SharedFrame {
        SharedFrame {
            texture_id: slot.id,
            texture_handle: slot.handle.value(),
            width: self.width,
            height: self.height,
            generation,
            ready: self.ready.point(generation),
            released: self.released.point(generation),
        }
    }

    /// Pick a target the compositor can no longer be reading.
    ///
    /// A used target is free once it is not the latest frame and the
    /// compositor has finished a draw of any newer frame: compositor work is
    /// ordered, so its reads of the older frame completed before that.
    fn free_slot(&mut self) -> Option<usize> {
        // SAFETY: the fence is alive for as long as `self`; this only reads
        // its current value and never blocks.
        let completed = unsafe { self.released.fence.GetCompletedValue() };
        // A removed device reports u64::MAX; never treat that as a release.
        if completed != u64::MAX {
            self.max_released = self.max_released.max(completed);
        }
        let released = self.max_released;
        self.slots
            .iter()
            .enumerate()
            .filter(|(index, slot)| {
                Some(*index) != self.latest
                    && slot
                        .generation
                        .is_none_or(|generation| generation < released)
            })
            .min_by_key(|(_, slot)| slot.generation)
            .map(|(index, _)| index)
    }

    /// Render one frame into a free target and publish it.
    ///
    /// `draw` receives the target view and must submit its GPU work on
    /// `queue` before returning; the completion signal is attached to that
    /// submission. Returns `Ok(None)` without calling `draw` when the
    /// compositor has not released a target yet (for example while the window
    /// is minimized); the caller should then repaint [`Self::latest`].
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        draw: impl FnOnce(&wgpu::TextureView) -> Result<(), Box<dyn Error + Send + Sync>>,
    ) -> Result<Option<SharedFrame>, BridgeError> {
        let Some(index) = self.free_slot() else {
            return Ok(None);
        };
        // SAFETY: the guard is only used to stage and unstage a fence signal;
        // no queue resource is destroyed through it.
        let hal_queue = unsafe { queue.as_hal::<Dx12>() }.ok_or(BridgeError::NotDirectX12)?;

        self.generation += 1;
        let generation = self.generation;
        // The signal is issued on the queue right after the command lists of
        // the next submission, which is the one `draw` performs.
        hal_queue.add_signal_fence(self.ready.fence.clone(), generation);
        if let Err(error) = draw(&self.slots[index].view) {
            // If nothing was submitted the signal is still pending; drop it so
            // it cannot be attached to unrelated work. The generation is
            // never published, so no compositor waits for it either way.
            hal_queue.remove_signal_fence(&self.ready.fence);
            return Err(BridgeError::Render(error));
        }
        self.slots[index].generation = Some(generation);
        self.latest = Some(index);
        Ok(Some(self.frame(&self.slots[index], generation)))
    }
}
