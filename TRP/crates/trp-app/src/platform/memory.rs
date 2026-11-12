//! What the run holds in memory and in the world, noted in its log.
//!
//! What the run holds in memory, in its log: the process's resident set,
//! its peak and its private bytes (the system's counters), what the system
//! counts it as using of the graphics card's own memory (DXGI's
//! `QueryVideoMemoryInfo`, whatever the backend), and, where the graphics
//! library keeps one (DirectX 12, not Vulkan), its allocator's report of
//! what it has allocated (the pictures, meshes and buffers together), as the run begins (the warm-up's
//! end), every `EVERY` seconds after and as it ends, so that what is read
//! when can be measured in memory as well as in time (`perf.sh` reads the
//! last line). The pictures alone: the mip streaming's own line.

use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;

/// Seconds between two notes after the run has begun.
const EVERY: f32 = 30.0;

/// Megabytes.
fn mb(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 20) as f64
}

/// The memory as it stands, in a line: what the process holds, and what is
/// allocated on the graphics card where the engine's device is given.
pub fn describe(device: Option<&RenderDevice>) -> String {
    let mut line = match platform::resident() {
        Some([resident, peak, private]) => format!("{:.0} MB resident (at most {:.0}), {:.0} MB private", mb(resident), mb(peak), mb(private)),
        None => "the resident memory not known".to_owned(),
    };
    if let Some(used) = platform::card_used() {
        line.push_str(&format!("; {:.0} MB of the graphics card's memory in use", mb(used)));
    }
    if let Some(report) = device.and_then(|device| device.wgpu_device().generate_allocator_report()) {
        line.push_str(&format!(
            " ({:.0} MB allocated by the graphics library in {:.0} MB of blocks, {} allocations)",
            mb(report.total_allocated_bytes),
            mb(report.total_reserved_bytes),
            report.allocations.len()
        ));
    }
    line
}

/// What the world holds, counted: every entity (the parked ones too), and
/// the meshes, materials and pictures the engine keeps. A count that only
/// grows over a long run names what is not let go.
type Counted<'w, 's> = (Query<'w, 's, Has<bevy::ecs::entity_disabling::Disabled>, crate::platform::park::EvenParked>, Query<'w, 's, (), (With<Mesh3d>, crate::platform::park::EvenParked)>, Res<'w, Assets<Mesh>>, Res<'w, Assets<StandardMaterial>>, Res<'w, Assets<Image>>);

/// Notes the memory and the counts as the run begins and every `EVERY`
/// seconds after, and as it is told to end.
pub fn note_memory(warmup: Res<crate::Warmup>, device: Option<Res<RenderDevice>>, time: Res<Time<Real>>, mut exits: MessageReader<AppExit>, mut next: Local<Option<f32>>, (entities, drawn, meshes, materials, images): Counted) {
    let ending = exits.read().next().is_some();
    if warmup.0 && !ending {
        return;
    }
    let now = time.elapsed_secs();
    let when = match *next {
        _ if ending => "as the run ends",
        None => "as the run begins",
        Some(at) if now >= at => "",
        Some(_) => return,
    };
    *next = Some(now + EVERY);
    let (all, parked) = entities.iter().fold((0, 0), |(all, parked), disabled| (all + 1, parked + usize::from(disabled)));
    let counts = format!("{all} entities ({parked} parked, {} with a mesh), {} meshes, {} materials, {} pictures", drawn.iter().count(), meshes.len(), materials.len(), images.len());
    info!("memory{}{when}: {}; {counts}", if when.is_empty() { "" } else { " " }, describe(device.as_deref()));
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod platform {
    /// `PROCESS_MEMORY_COUNTERS_EX` (psapi.h).
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn K32GetProcessMemoryInfo(process: *mut core::ffi::c_void, counters: *mut Counters, cb: u32) -> i32;
    }

    /// The working set, its peak and the private bytes.
    pub fn resident() -> Option<[u64; 3]> {
        let mut counters = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
        // SAFETY: the process's own pseudo-handle, and a structure of the size it is told.
        let done = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
        (done != 0).then(|| [counters.working_set_size as u64, counters.peak_working_set_size as u64, counters.private_usage as u64])
    }

    type Object = *mut *const usize;

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    /// `DXGI_QUERY_VIDEO_MEMORY_INFO`.
    #[repr(C)]
    #[derive(Default)]
    struct VideoMemory {
        budget: u64,
        current_usage: u64,
        available_for_reservation: u64,
        current_reservation: u64,
    }

    const IID_FACTORY1: Guid = Guid(0x770aae78, 0xf26f, 0x4dba, [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87]);
    const IID_ADAPTER3: Guid = Guid(0x645967a4, 0x1392, 0x4310, [0xa7, 0x98, 0x80, 0x53, 0xce, 0x3e, 0x93, 0xfd]);
    /// Places in the interfaces' tables: `IUnknown::QueryInterface` and
    /// `Release`, `IDXGIFactory1::EnumAdapters1`,
    /// `IDXGIAdapter3::QueryVideoMemoryInfo`.
    const QUERY_INTERFACE: usize = 0;
    const RELEASE: usize = 2;
    const ENUM_ADAPTERS1: usize = 12;
    const QUERY_VIDEO_MEMORY_INFO: usize = 14;

    #[link(name = "dxgi")]
    extern "system" {
        fn CreateDXGIFactory1(iid: *const Guid, factory: *mut Object) -> i32;
    }

    /// The method at a place in an object's table.
    ///
    /// SAFETY: `object` is a live COM object whose table has that place.
    unsafe fn method(object: Object, place: usize) -> usize {
        unsafe { *(*object).add(place) }
    }

    unsafe fn release(object: Object) {
        let release: extern "system" fn(Object) -> u32 = unsafe { std::mem::transmute(method(object, RELEASE)) };
        release(object);
    }

    /// What this process uses of the cards' own memory (the local segment
    /// group), over every adapter.
    pub fn card_used() -> Option<u64> {
        let mut factory: Object = std::ptr::null_mut();
        // SAFETY: the interface's id and a place for the factory it makes.
        if unsafe { CreateDXGIFactory1(&IID_FACTORY1, &mut factory) } < 0 || factory.is_null() {
            return None;
        }
        let mut used = None;
        // SAFETY: each call is on a live object at its place in the table
        // (dxgi1_4.h), with the arguments it is declared with; every object
        // got is released.
        unsafe {
            let enum_adapters: extern "system" fn(Object, u32, *mut Object) -> i32 = std::mem::transmute(method(factory, ENUM_ADAPTERS1));
            for index in 0.. {
                let mut adapter: Object = std::ptr::null_mut();
                if enum_adapters(factory, index, &mut adapter) < 0 || adapter.is_null() {
                    break;
                }
                let query: extern "system" fn(Object, *const Guid, *mut Object) -> i32 = std::mem::transmute(method(adapter, QUERY_INTERFACE));
                let mut third: Object = std::ptr::null_mut();
                if query(adapter, &IID_ADAPTER3, &mut third) >= 0 && !third.is_null() {
                    let memory_info: extern "system" fn(Object, u32, u32, *mut VideoMemory) -> i32 = std::mem::transmute(method(third, QUERY_VIDEO_MEMORY_INFO));
                    let mut info = VideoMemory::default();
                    if memory_info(third, 0, 0, &mut info) >= 0 {
                        *used.get_or_insert(0) += info.current_usage;
                    }
                    release(third);
                }
                release(adapter);
            }
            release(factory);
        }
        used
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn resident() -> Option<[u64; 3]> {
        None
    }

    pub fn card_used() -> Option<u64> {
        None
    }
}
