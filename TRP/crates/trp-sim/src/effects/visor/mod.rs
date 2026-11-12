//! The face shield's mask over the first-person view.
//! After the inventory spec, "The face shield's mask" (`VisorEffect`,
//! `PlayerCameraController.OnFaceShieldComponentChanged`, the camera's
//! `HysteresisFilter`): which
//! shield the camera goes by, as the game's two slot observers hand it on,
//! its kind of mask, the fade in and out as it comes down and goes up, and
//! the walk's swing of the mask.

use trp_data::formats::curve::Curve;
use trp_data::character::visor::MaskKind;

use crate::base::hysteresis::Hysteresis;
use crate::items::inventory::{self, Equipment, Id};

/// A face shield as the camera sees it: which thing it is, its kind of
/// mask, whether it is down (`Togglable` on, or none), and its hits and
/// seed (`FaceShieldComponent.Hits`, `HitSeed`: a mark is a change heard,
/// `HitsChanged`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shield {
    pub id: Id,
    pub mask: MaskKind,
    pub down: bool,
    pub hits: u8,
    pub seed: u8,
}

/// The first face shield of the thing in a slot, the thing itself first,
/// each part before the parts fitted to it (`SlotObserver.GetItemComponent`:
/// `GetItemComponentsInChildren<FaceShieldComponent>().FirstOrDefault()`).
pub fn shield_in(equipment: &Equipment, slot: &str) -> Option<Shield> {
    let shield = inventory::face_shield(equipment.worn(slot)?)?;
    Some(Shield { id: shield.id, mask: shield.kind.face_shield_mask, down: !inventory::has_switch(shield) || shield.on, hits: shield.hits, seed: shield.hit_seed })
}

/// The most hits whose cracks are drawn (`OnFaceShieldComponentChanged`:
/// `Mathf.Min(Hits, 20)`).
pub const MOST_CRACKS: u8 = 20;

/// The cracks the camera last drew (`DrawScratches`): the shield's seed,
/// how many, and how many drawings there have been (each a clearing and a
/// drawing anew of the cracks' picture, at every change handed on with a
/// shield). Nought drawings: the picture never drawn (empty).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cracks {
    pub seed: u8,
    pub count: u8,
    pub drawings: u64,
}

/// One crack's quad (`VisorEffect.Draw`) in the target's own numbers
/// before the shader's offset: its lower left and upper right corners
/// (the crack picture's (0, 0) and (1, 1)) and its turn (`_Rotation`, the
/// degrees drawn, which the shader takes as radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrackQuad {
    pub low: [f32; 2],
    pub high: [f32; 2],
    pub rotation: f32,
}

/// The quads of `count` cracks (`DrawScratches`): the engine's random
/// numbers seeded with the shield's seed (`Random.InitState`), then for
/// each four drawn in turn, x and y from −1 to 1, a turn from 0 to 360, a
/// size k from 1.6 to 2.6, making the quad from `(x − k/a, −y − k)` to
/// `(x + k/a, −y + k)`, `a` the camera's aspect.
pub fn crack_quads(seed: u8, count: u8, aspect: f32) -> Vec<CrackQuad> {
    let mut random = crate::base::unity_random::UnityRandom::new(i32::from(seed));
    (0..count)
        .map(|_| {
            let (x, y) = (random.range(-1.0, 1.0), random.range(-1.0, 1.0));
            let rotation = random.range(0.0, 360.0);
            let k = random.range(1.6, 2.6);
            let across = k / aspect;
            CrackQuad { low: [x - across, -y - k], high: [x + across, -y + k], rotation }
        })
        .collect()
}

/// The goggles' key (`Player.ToggleGoggles`; inventory spec, "A switch on
/// the head"): the switch of what is worn on the head flipped, at once.
/// The part switched and how it stands now; none with nothing switchable
/// worn there.
pub fn toggle_goggles(equipment: &mut Equipment) -> Option<(Id, bool)> {
    let head = equipment.worn("Headwear")?.id;
    inventory::toggle(&mut equipment.item, head)
}

/// What follows a switch flipped on what is worn on the head
/// (`PlayerToggleOperation.Dispose`): where the movement state lets the
/// hands act, the switch's gesture is asked of them, which is refused in
/// a sprint, on a mount and while the thing in hand is not at rest (a
/// reload: `courses8`'s visors, the switch made all the same). None where
/// it was not asked; else whether it plays.
pub fn switch_gesture(movement: &mut crate::Movement, face_shield: bool, on: bool) -> Option<bool> {
    movement.state_suits_hands().then(|| movement.set_interaction(crate::Interaction::of_switch(face_shield, on)))
}

/// What the character hears of what is worn on the head (inventory spec,
/// "A switch on the head"; `Player.PlayFaceShieldSound`,
/// `PlayNightVisionSound`, `PlayThermalVisionSound`, `PlayToggleSound`):
/// for a face shield, night vision and thermal vision, the first part of
/// that kind on the headwear's thing (`SlotObserver`), whether it is there
/// and on (one with no switch counts as on), as last heard. Its sound plays
/// whenever that changes: at a switch, and as well when the thing is put
/// on or taken off with it on (`courses2\6-helmet`: the ZSh-1-2M discarded
/// with its shield down, the Caiman put on with its GPNVG-18 on).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SwitchSounds {
    heard: [bool; 3],
}

impl SwitchSounds {
    /// As the equipment stands now, already heard (a recording begun with
    /// the head as it was). The game's player begins with nothing heard
    /// (`Default`), and its observers' first call, made at once
    /// (`BindableEvent.Bind`), sounds what is on as it spawns.
    pub fn as_worn(equipment: &Equipment) -> Self {
        Self { heard: Self::states(equipment) }
    }

    /// Each kind's first part on the head, there and on.
    fn states(equipment: &Equipment) -> [bool; 3] {
        let first = |kind: &dyn Fn(&inventory::Item) -> bool| {
            let worn = equipment.worn("Headwear")?;
            inventory::with_parts(worn).find(|part| kind(part)).map(|part| !inventory::has_switch(part) || part.on)
        };
        [
            first(&|part| part.kind.face_shield),
            first(&|part| inventory::is(part, "NightVision")),
            first(&|part| inventory::is(part, "ThermalVision")),
        ]
        .map(|on| on == Some(true))
    }

    /// The sounds to play now (`trp_data::sound::TOGGLE_SOUNDS`' places: a
    /// face shield's on and off, night vision's, thermal vision's), one for
    /// each kind whose state changed since it was last heard.
    pub fn hear(&mut self, equipment: &Equipment) -> Vec<usize> {
        let now = Self::states(equipment);
        let sounds = (0..3).filter(|&kind| now[kind] != self.heard[kind]).map(|kind| kind * 2 + usize::from(!now[kind])).collect();
        self.heard = now;
        sounds
    }
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
    cracks: Cracks,
}

/// Below this the effect only copies the frame (`OnRenderImageImpl`).
const DRAWN_FROM: f32 = 0.01;

impl Visor {
    pub fn new(figures: VisorFigures) -> Self {
        let filter = Hysteresis::new(figures.forward.clone(), figures.backward.clone());
        Self { figures, filter, heard: false, head: None, face: None, live: false, mask: MaskKind::None, intensity: 0.0, clock: 0.0, swing: [0.0; 2], rotation: 0.0, cracks: Cracks::default() }
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
    /// (`SetMask`; none keeps the last) and has its cracks drawn anew
    /// (`DrawScratches`, after `BreakCompletely`, whose edge cracks the
    /// drawing's clearing wipes at once: nothing of them is kept), and the
    /// filter is set going down where it is down, up otherwise, at once the
    /// first time.
    fn changed(&mut self, shield: Option<Shield>) {
        let initial = !self.live;
        self.live = true;
        if let Some(shield) = shield {
            self.mask = shield.mask;
            self.cracks = Cracks { seed: shield.seed, count: shield.hits.min(MOST_CRACKS), drawings: self.cracks.drawings + 1 };
        }
        if let Some(value) = self.filter.set(shield.is_some_and(|shield| shield.down), initial) {
            self.intensity = value;
        }
    }

    /// The filter moved on (`HysteresisFilter.Update`). In the game's frame
    /// this comes before the frame's switch is heard (`courses8`'s visors:
    /// on a switch's frame the filter's clock has not moved, and a switch
    /// turned back mid-way finds it moved on that frame toward the old
    /// way), so a frame is `update`, then `observe`, then `frame`.
    pub fn update(&mut self, dt: f32) {
        if let Some(value) = self.filter.update(dt) {
            self.intensity = value;
        }
    }

    /// The camera's filter, for setting beside a recording.
    pub fn filter(&self) -> &Hysteresis {
        &self.filter
    }

    /// The cracks last drawn (`DrawScratches`).
    pub fn cracks(&self) -> Cracks {
        self.cracks
    }

    /// What is drawn this frame, with while the mask is drawn its swing
    /// (`SetSwing`, as the effect renders, after the frame's updates):
    /// none while the walk's step frequency is below 0.01 (mounted);
    /// standing (a speed below 0.001), back toward nothing by ten times
    /// the frame's time, its clock to 0; moving, its clock on by the step
    /// frequency and the three curves read at it, each repeating after its
    /// last key.
    pub fn frame(&mut self, dt: f32, step_frequency: f32, speed: f32) -> VisorFrame {
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

/// A hinge on what is worn on the head (`HelmetDress`; body animator spec,
/// "A worn thing's parts and a helmet's hinge"): the thing whose model
/// carries it as last seen, that thing's face shield and whether it was
/// to be down, and the filter that turns it.
#[derive(Debug, Clone)]
pub struct ShieldHinge {
    worn: Option<Id>,
    shield: Option<Id>,
    down: bool,
    filter: Hysteresis,
}

impl ShieldHinge {
    pub fn new(forward: Curve, backward: Curve) -> Self {
        Self { worn: None, shield: None, down: true, filter: Hysteresis::new(forward, backward) }
    }

    /// A frame, given the thing whose model carries the hinge as it is
    /// worn now (none: not worn, its model put away). That thing put on or
    /// fitted has the hinge set at once (`Init`, `UpdateState(initial)`, as
    /// its model is made); while it stays, the filter moves on first
    /// (`HysteresisFilter.Update`, before the frame's switch, as recorded:
    /// `courses8`'s visors) and then its face shield changed or switched
    /// sets the filter's way (`SetFaceShield`, `OnFaceShieldChanged`). The
    /// value given, if one was: the share of the way from the turn up to
    /// the turn down.
    pub fn frame(&mut self, worn: Option<&inventory::Item>, dt: f32) -> Option<f32> {
        let Some(worn) = worn else {
            self.worn = None;
            return None;
        };
        let shield = inventory::face_shield(worn).map(|shield| shield.id);
        let down = inventory::hinge_down(worn);
        if self.worn != Some(worn.id) {
            (self.worn, self.shield, self.down) = (Some(worn.id), shield, down);
            return self.filter.set(down, true);
        }
        let given = self.filter.update(dt);
        if self.shield != shield || self.down != down {
            (self.shield, self.down) = (shield, down);
            self.filter.set(down, false);
        }
        given
    }

    /// Whether the thing carrying it is worn, as last seen.
    pub fn worn(&self) -> bool {
        self.worn.is_some()
    }

    /// Its filter, for setting beside a recording.
    pub fn filter(&self) -> &Hysteresis {
        &self.filter
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
mod tests;
