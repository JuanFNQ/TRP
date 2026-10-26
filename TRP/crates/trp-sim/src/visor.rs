//! The face shield's mask over the first person's view (inventory spec, "The
//! face shield's mask"; `VisorEffect`, `PlayerCameraController.
//! OnFaceShieldComponentChanged`, the camera's `HysteresisFilter`): which
//! shield the camera goes by, as the game's two slot observers hand it on,
//! its kind of mask, the fade in and out as it comes down and goes up, and
//! the walk's swing of the mask.

use trp_data::curve::Curve;
use trp_data::visor::MaskKind;

use crate::hysteresis::Hysteresis;
use crate::inventory::{self, Equipment, Id};

/// A face shield as the camera sees it: which thing it is, its kind of
/// mask, and whether it is down (`Togglable` on, or none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shield {
    pub id: Id,
    pub mask: MaskKind,
    pub down: bool,
}

/// The first face shield of the thing in a slot, the thing itself first,
/// each part before the parts fitted to it (`SlotObserver.GetItemComponent`:
/// `GetItemComponentsInChildren<FaceShieldComponent>().FirstOrDefault()`).
pub fn shield_in(equipment: &Equipment, slot: &str) -> Option<Shield> {
    let shield = inventory::face_shield(equipment.worn(slot)?)?;
    Some(Shield { id: shield.id, mask: shield.kind.face_shield_mask, down: !inventory::has_switch(shield) || shield.on })
}

/// The mask's figures (`trp_data::visor::VisorData`'s curves).
#[derive(Debug, Clone)]
pub struct VisorFigures {
    pub forward: Curve,
    pub backward: Curve,
    pub edge_fade: Curve,
    pub dx: Curve,
    pub dy: Curve,
    pub walk_rotation: Curve,
}

/// What the mask draws this frame (`VisorEffect`'s values as it renders).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisorFrame {
    /// `_Intensity`: 0 (nothing) to 1 (down).
    pub intensity: f32,
    /// `_EdgeFade`: the lower edge's fade.
    pub edge_fade: f32,
    /// `_SwingValue` and `_Rotation` (radians).
    pub swing: [f32; 2],
    pub rotation: f32,
    pub mask: MaskKind,
}

/// The camera's mask: the two observers' shields as last heard, whether
/// the camera has heard one yet (`_onFaceShieldChangedIsLive`), the mask
/// set, the filter and the intensity it gave, and the walk's swing.
#[derive(Debug, Clone)]
pub struct Visor {
    figures: VisorFigures,
    filter: Hysteresis,
    heard: bool,
    head: Option<Shield>,
    face: Option<Shield>,
    live: bool,
    mask: MaskKind,
    intensity: f32,
    clock: f32,
    swing: [f32; 2],
    rotation: f32,
}

/// Below this the effect only copies the frame (`OnRenderImageImpl`).
const DRAWN_FROM: f32 = 0.01;

impl Visor {
    pub fn new(figures: VisorFigures) -> Self {
        let filter = Hysteresis::new(figures.forward.clone(), figures.backward.clone());
        Self { figures, filter, heard: false, head: None, face: None, live: false, mask: MaskKind::None, intensity: 0.0, clock: 0.0, swing: [0.0; 2], rotation: 0.0 }
    }

    /// The shields in the headwear and face cover slots this frame. The
    /// observers' events as the game has them: as the camera is made both
    /// are heard, the head's first (`Construct`, each `Bind` running at
    /// once); then the head's shield changed (another, none, or switched)
    /// is handed on only while the face has none (`method_1`), and the
    /// face's only while the head has none (`method_2`).
    pub fn observe(&mut self, head: Option<Shield>, face: Option<Shield>) {
        let first = !self.heard;
        self.heard = true;
        if first || head != self.head {
            self.head = head;
            if face.is_none() {
                self.changed(head);
            }
        }
        if first || face != self.face {
            self.face = face;
            if self.head.is_none() {
                self.changed(face);
            }
        }
    }

    /// `OnFaceShieldComponentChanged`: a shield sets the mask's kind
    /// (`SetMask`; none keeps the last), and the filter is set going down
    /// where it is down, up otherwise, at once the first time.
    fn changed(&mut self, shield: Option<Shield>) {
        let initial = !self.live;
        self.live = true;
        if let Some(shield) = shield {
            self.mask = shield.mask;
        }
        if let Some(value) = self.filter.set(shield.is_some_and(|shield| shield.down), initial) {
            self.intensity = value;
        }
    }

    /// A frame: the filter moves on (`HysteresisFilter.Update`), and while
    /// the mask is drawn its swing (`SetSwing`): none while the walk's step
    /// frequency is below 0.01 (mounted); standing (a speed below 0.001),
    /// back toward nothing by ten times the frame's time, its clock to 0;
    /// moving, its clock on by the step frequency and the three curves read
    /// at it, each repeating after its last key.
    pub fn frame(&mut self, dt: f32, step_frequency: f32, speed: f32) -> VisorFrame {
        if let Some(value) = self.filter.update(dt) {
            self.intensity = value;
        }
        if self.intensity >= DRAWN_FROM && step_frequency >= 0.01 {
            if speed < 0.001 {
                let share = (dt * 10.0).clamp(0.0, 1.0);
                self.swing = self.swing.map(|value| value + (0.0 - value) * share);
                self.rotation += (0.0 - self.rotation) * share;
                self.clock = 0.0;
            } else {
                self.clock += dt * step_frequency;
                self.swing = [looped(&self.figures.dx, self.clock), looped(&self.figures.dy, self.clock)];
                self.rotation = looped(&self.figures.walk_rotation, self.clock);
            }
        }
        VisorFrame { intensity: self.intensity, edge_fade: self.figures.edge_fade.evaluate(self.intensity), swing: self.swing, rotation: self.rotation, mask: self.mask }
    }

    /// Whether the mask is drawn at all (`SetIntensity`: on while above
    /// nothing; below 0.01 it only copies the frame).
    pub fn drawn(frame: &VisorFrame) -> bool {
        frame.intensity >= DRAWN_FROM
    }
}

/// A curve that repeats after its last key (stored wrap 1). `PROVISIONAL`:
/// wrap 1 taken as the engine's loop from what is known of its numbering,
/// checked once (the breath's curves, which repeat, are stored so).
fn looped(curve: &Curve, time: f32) -> f32 {
    let (Some(first), Some(last)) = (curve.keys.first(), curve.keys.last()) else { return 0.0 };
    let span = last.time - first.time;
    if span <= 0.0 {
        return first.value;
    }
    curve.evaluate(first.time + (time - first.time).rem_euclid(span))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The camera's filter as the install has it (`Cam2`'s
    /// `HysteresisFilter`): down after 0.6 s over 0.2 s, up likewise.
    fn figures() -> VisorFigures {
        let inf = f32::INFINITY;
        let key = |time, value, in_slope, out_slope| trp_data::curve::Key { time, value, in_slope, out_slope };
        VisorFigures {
            forward: Curve { keys: vec![key(0.0, 0.0, inf, inf), key(0.60171777, 0.0, 0.0, 5.0000005), key(0.80171776, 1.0, 5.0000005, inf)] },
            backward: Curve { keys: vec![key(0.0, 0.0, 3.862559, 3.862559), key(0.19999999, 1.0, 5.0000005, -0.0022513764), key(1.3423328, 0.9974282, inf, inf)] },
            edge_fade: Curve::linear(&[(0.0, 0.1), (1.0, 0.0)]),
            dx: Curve::linear(&[(0.0, 0.0), (0.5, 0.001), (1.0, 0.0)]),
            dy: Curve::linear(&[(0.0, 0.0), (1.0, 0.0)]),
            walk_rotation: Curve::linear(&[(0.0, 0.0), (1.0, 0.0)]),
        }
    }

    fn shield(id: Id, down: bool) -> Option<Shield> {
        Some(Shield { id, mask: MaskKind::Wide, down })
    }

    /// Worn down from the start it is on at once; flipped up it holds for
    /// 0.6 s and is gone by 0.8 s; down again, nothing for 0.6 s, then on
    /// by 0.8 s.
    #[test]
    fn the_mask_fades_as_the_shield_moves() {
        let mut visor = Visor::new(figures());
        visor.observe(shield(1, true), None);
        assert_eq!(visor.frame(0.0, 0.7, 0.0).intensity, 1.0, "at once as the camera is made");
        visor.observe(shield(1, false), None);
        let mut at = |visor: &mut Visor, seconds: f32| {
            let mut frame = visor.frame(0.0, 0.7, 0.0);
            for _ in 0..(seconds * 100.0).round() as usize {
                frame = visor.frame(0.01, 0.7, 0.0);
            }
            frame.intensity
        };
        assert!(at(&mut visor, 0.5) > 0.99, "held while the hand reaches up");
        assert!(at(&mut visor, 0.35) < 0.01, "up by 0.8 s");
        visor.observe(shield(1, true), None);
        assert!(at(&mut visor, 0.55) < 0.01, "nothing for 0.6 s");
        assert!(at(&mut visor, 0.3) > 0.99, "down by 0.8 s");
        // Taken off, it goes as if flipped up, its mask kept.
        visor.observe(None, None);
        assert!(at(&mut visor, 1.0) < 0.01);
        assert_eq!(visor.frame(0.0, 0.7, 0.0).mask, MaskKind::Wide);
    }

    /// The face cover's shield counts only while the head has none, and a
    /// change of the head's is not heard while the face has one.
    #[test]
    fn the_head_s_shield_and_the_face_s() {
        let mut visor = Visor::new(figures());
        let welding = Some(Shield { id: 2, mask: MaskKind::Wide, down: true });
        visor.observe(None, welding);
        assert_eq!(visor.frame(0.0, 0.7, 0.0).intensity, 1.0, "the face's, at once");
        visor.observe(Some(Shield { id: 3, mask: MaskKind::Narrow, down: false }), welding);
        assert_eq!(visor.frame(0.0, 0.7, 0.0).mask, MaskKind::Wide, "the head's not heard while the face has one");
    }

    /// Walking, the swing follows the curves on a clock run by the step
    /// frequency; standing, it settles back.
    #[test]
    fn the_mask_swings_with_the_walk() {
        let mut visor = Visor::new(figures());
        visor.observe(shield(1, true), None);
        let mut frame = visor.frame(0.0, 1.0, 1.0);
        for _ in 0..50 {
            frame = visor.frame(0.01, 1.0, 1.0);
        }
        assert!((frame.swing[0] - 0.001).abs() < 1e-5, "{:?}", frame.swing);
        for _ in 0..100 {
            frame = visor.frame(0.01, 1.0, 0.0);
        }
        assert!(frame.swing[0].abs() < 1e-6, "{:?}", frame.swing);
    }
}
