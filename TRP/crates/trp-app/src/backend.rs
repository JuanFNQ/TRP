//! Which graphics backend the renderer draws through: Vulkan unless the
//! options or the command line say DirectX 12, and the other one where the
//! one chosen has no graphics card on the machine (`docs/spec/settings.md`,
//! "The renderer"). The drawing alone: both draw the same picture, and
//! nothing of the simulation changes.
//!
//! Measured on 2026-10-15 (`docs/findings.md`, "Added 2026-10-15, later"):
//! on Vulkan Bevy's materials are bindless (they share one table of
//! pictures, so the parts of many weapons are drawn together), on DirectX
//! 12 they cannot be (wgpu's backend has no arrays of buffers); uncapped a
//! frame takes 2.10 ms against 2.69 walking at the range and 2.93 against
//! 4.64 among 43 rifles, and what is drawn is ready 0.5 s into a run against
//! 0.7. With vsync the screen's 165 frames come as evenly on either (the
//! motion's ripple the same), Vulkan's own clock a little less even (7% of
//! frames a tenth off the beat against 3%, a late one made up by the next).
//! Bevy 0.16's Vulkan had handed them out in clumps of four.

use bevy::render::settings::Backends;

/// A backend the options can choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    Vulkan,
    Dx12,
}

impl Renderer {
    /// The default: Vulkan, as measured above.
    pub const DEFAULT: Renderer = Renderer::Vulkan;

    /// Its name in the options and the command line, and as shown.
    pub fn key(self) -> &'static str {
        match self {
            Renderer::Vulkan => "vulkan",
            Renderer::Dx12 => "dx12",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Renderer::Vulkan => "VULKAN",
            Renderer::Dx12 => "DIRECTX 12",
        }
    }

    pub fn from_key(key: &str) -> Option<Renderer> {
        match key {
            "vulkan" => Some(Renderer::Vulkan),
            "dx12" | "directx" | "d3d12" => Some(Renderer::Dx12),
            _ => None,
        }
    }

    fn other(self) -> Renderer {
        match self {
            Renderer::Vulkan => Renderer::Dx12,
            Renderer::Dx12 => Renderer::Vulkan,
        }
    }

    fn backends(self) -> Backends {
        match self {
            Renderer::Vulkan => Backends::VULKAN,
            Renderer::Dx12 => Backends::DX12,
        }
    }
}

/// The backend to draw through, and what the run's first lines say of it:
/// the one wanted where it has a graphics card that is not the processor
/// drawing in its stead; else the other, where that has one; else the one
/// wanted all the same (and the engine says what it finds).
pub fn choose(wanted: Renderer, has_card: impl Fn(Renderer) -> bool) -> (Renderer, String) {
    if has_card(wanted) {
        return (wanted, format!("renderer: {}", wanted.word()));
    }
    let other = wanted.other();
    if has_card(other) {
        return (other, format!("renderer: {} ({} finds no graphics card on this machine)", other.word(), wanted.word()));
    }
    (wanted, format!("renderer: {} (neither backend finds a graphics card; the engine will say what it finds)", wanted.word()))
}

/// Whether a backend finds a graphics card here, one that is not the
/// processor drawing in its stead. Asked of the graphics library before
/// the engine starts: some 0.1 s.
pub fn has_card(renderer: Renderer) -> bool {
    let backends = renderer.backends();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, ..Default::default() });
    instance.enumerate_adapters(backends).iter().any(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
}

/// The engine's choice of backends for a renderer.
pub fn backends(renderer: Renderer) -> Backends {
    renderer.backends()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_wanted_unless_it_has_no_card() {
        let only = |card: Option<Renderer>| move |renderer: Renderer| Some(renderer) == card;
        assert_eq!(choose(Renderer::Vulkan, only(Some(Renderer::Vulkan))).0, Renderer::Vulkan);
        assert_eq!(choose(Renderer::Vulkan, |_| true).0, Renderer::Vulkan);
        assert_eq!(choose(Renderer::Dx12, |_| true).0, Renderer::Dx12);
        assert_eq!(choose(Renderer::Vulkan, only(Some(Renderer::Dx12))).0, Renderer::Dx12);
        assert_eq!(choose(Renderer::Dx12, only(Some(Renderer::Vulkan))).0, Renderer::Vulkan);
        assert_eq!(choose(Renderer::Vulkan, only(None)).0, Renderer::Vulkan);
        assert_eq!(Renderer::from_key(Renderer::Dx12.key()), Some(Renderer::Dx12));
        assert_eq!(Renderer::from_key(Renderer::Vulkan.key()), Some(Renderer::Vulkan));
    }
}
