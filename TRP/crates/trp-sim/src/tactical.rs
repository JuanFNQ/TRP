//! Tactical devices (tactical-devices spec): what each key asks of a
//! weapon's devices in either activation mode (`FirearmHandsInputTranslator`,
//! `GamePlayerOwner.ReleaseTactical`), the weapon's hip penalty they change
//! (`FirearmController.UpdateHipInaccuracy`), and the head light's switch
//! with its gesture, its two waits and its lock (`Player.SwitchHeadLights`
//! and its coroutine). Which device a weapon's part is, its state and its
//! modes are the inventory's (`inventory::Light`, `Kind::light_modes`).

use crate::inventory::{is, Id, Item, Light};

/// How the weapon's key works (`TacticalInputMode`): a press switches
/// each device, or the devices are on while the key is held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Press,
    Hold,
}

impl InputMode {
    /// The setting's word, as the game's settings and its HUD's locale
    /// name it ("Press", "Hold").
    pub fn word(self) -> &'static str {
        match self {
            Self::Press => "Press",
            Self::Hold => "Hold",
        }
    }

    pub fn of_word(word: &str) -> Option<Self> {
        match word {
            "Press" | "0" => Some(Self::Press),
            "Hold" | "1" => Some(Self::Hold),
            _ => None,
        }
    }

    /// The other one (`ToggleTacticalInteraction`, T+B).
    pub fn flipped(self) -> Self {
        match self {
            Self::Press => Self::Hold,
            Self::Hold => Self::Press,
        }
    }
}

/// What a key asks of the weapon's devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKey {
    /// `Tactical` (T) in Press mode: each device switched on its own.
    Toggle,
    /// `Tactical` in Hold mode: all on.
    Down,
    /// `ReleaseTactical` (T let go) in Hold mode, and every event that
    /// lets Hold mode's lights go (the hands changing, a death, a screen
    /// over the battle, the window's focus lost): all off.
    Up,
    /// `NextTacticalDevice` (Ctrl+T), either mode: each device's mode one
    /// up.
    Next,
}

/// A request to the firearm (`SetLightsState`): each device's new state,
/// whether it passes any refusal (`force`), whether the hands play their
/// switch (`ModSwitch`), and whether the player's tactical sound goes
/// with it once taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub states: Vec<(Id, Light)>,
    pub forced: bool,
    pub animated: bool,
    pub sound: bool,
}

/// Every device in a thing's slots, its parts' too, in the order the game
/// walks them (`CompoundItem.AllSlots`: a thing's slots in its kind's
/// order, then each of those in turn the same way).
pub fn devices(item: &Item) -> Vec<&Item> {
    fn below<'a>(item: &'a Item, out: &mut Vec<&'a Item>) {
        let kids: Vec<&Item> = item.slots.iter().flatten().collect();
        out.extend(kids.iter().copied().filter(|kid| kid.kind.light_modes.is_some()));
        for kid in kids {
            below(kid, out);
        }
    }
    let mut out = Vec::new();
    below(item, &mut out);
    out
}

/// A mode one up, wrapped by the device's count where it has one
/// (`LightComponent.SelectedMode`'s setter).
pub fn next_mode(mode: u32, modes: u32) -> u32 {
    if modes > 0 {
        (mode + 1) % modes
    } else {
        mode + 1
    }
}

/// What a key asks of a weapon's devices, or nothing where it has none or
/// the key asks nothing (Hold mode's key down with all on already, let go
/// with none on).
pub fn weapon_request(weapon: &Item, key: WeaponKey) -> Option<Request> {
    let found = devices(weapon);
    if found.is_empty() {
        return None;
    }
    let all = |on: bool| found.iter().map(|d| (d.id, Light { on, mode: d.light.mode })).collect::<Vec<_>>();
    match key {
        WeaponKey::Toggle => Some(Request { states: found.iter().map(|d| (d.id, Light { on: !d.light.on, mode: d.light.mode })).collect(), forced: false, animated: true, sound: false }),
        WeaponKey::Down => (!found.iter().all(|d| d.light.on)).then(|| Request { states: all(true), forced: false, animated: false, sound: true }),
        WeaponKey::Up => found.iter().any(|d| d.light.on).then(|| Request { states: all(false), forced: true, animated: false, sound: true }),
        WeaponKey::Next => Some(Request {
            states: found.iter().map(|d| (d.id, Light { on: d.light.on, mode: next_mode(d.light.mode, d.kind.light_modes.unwrap_or(0)) })).collect(),
            forced: false,
            animated: true,
            sound: false,
        }),
    }
}

/// Writes each device's state into the thing it is in (`LightComponent`'s
/// two figures). Whether every one was found.
pub fn apply(root: &mut Item, states: &[(Id, Light)]) -> bool {
    let mut all = true;
    for &(id, light) in states {
        match root.find_mut(id) {
            Some(item) if item.kind.light_modes.is_some() => {
                let modes = item.kind.light_modes.unwrap_or(0);
                item.light = Light { on: light.on, mode: if modes > 0 { light.mode % modes } else { light.mode } };
            }
            _ => all = false,
        }
    }
    all
}

/// Whether a combo unit on the weapon is on (any mode; a light-only one
/// too; a `Flashlight` is not one).
pub fn combo_on(weapon: &Item) -> bool {
    devices(weapon).iter().any(|d| d.light.on && is(d, "TacticalCombo"))
}

/// The hip wander's scale (`UpdateHipInaccuracy`, written into the breath's
/// `HipPenalty`): by the weapon's total ergonomics E, 0.3 − E/400 with a
/// combo unit on, else 1 − clamp(E/250 − 0.15, 0, 1).
pub fn hip_penalty(ergonomics: f32, combo_on: bool) -> f32 {
    if combo_on {
        0.3 - ergonomics / 400.0
    } else {
        1.0 - (ergonomics / 250.0 - 0.15).clamp(0.0, 1.0)
    }
}

/// The head light's coroutine: the devices' models and the sound after
/// 0.8 s, the lock lifted 0.6 s after that (tactical-devices spec, "The
/// head light").
pub const HEAD_SWITCH_AFTER: f32 = 0.8;
pub const HEAD_UNLOCK_AFTER: f32 = 0.6;

/// The worn things a head device can be fitted to (the model under the
/// head's bone that the game collects them from).
pub const HEAD_SLOTS: [&str; 4] = ["Headwear", "FaceCover", "Eyewear", "Earpiece"];

/// What the head light's coroutine does on a frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeadStep {
    /// The devices' models switched and the tactical sound played.
    pub switched: bool,
    /// The lock lifted.
    pub unlocked: bool,
}

/// The head light: its lock (`IsHeadLightsAnimationActive`) and where its
/// coroutine is, by the seconds since its last wait began.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HeadLight {
    /// Seconds into the first wait, the switch still to come.
    to_switch: Option<f32>,
    /// Seconds into the second, the lock still on.
    to_unlock: Option<f32>,
}

impl HeadLight {
    pub fn locked(&self) -> bool {
        self.to_switch.is_some() || self.to_unlock.is_some()
    }

    /// A press taken: the lock on and the coroutine started. Refused (false)
    /// while the lock is on; the hands' state and the head's devices are the
    /// caller's to have asked.
    pub fn press(&mut self) -> bool {
        if self.locked() {
            return false;
        }
        self.to_switch = Some(0.0);
        true
    }

    /// A frame of `dt` seconds after the one the press was on.
    pub fn step(&mut self, dt: f32) -> HeadStep {
        let mut out = HeadStep::default();
        if let Some(t) = self.to_switch.as_mut() {
            *t += dt;
            if *t >= HEAD_SWITCH_AFTER {
                self.to_switch = None;
                self.to_unlock = Some(0.0);
                out.switched = true;
                return out;
            }
        } else if let Some(t) = self.to_unlock.as_mut() {
            *t += dt;
            if *t >= HEAD_UNLOCK_AFTER {
                self.to_unlock = None;
                out.unlocked = true;
            }
        }
        out
    }
}

/// The head's devices: every device on what is worn on the head, its
/// parts' too, in the order the slots are listed.
pub fn head_devices<'a>(worn: impl Iterator<Item = &'a Item>) -> Vec<&'a Item> {
    let mut out = Vec::new();
    for thing in worn {
        if thing.kind.light_modes.is_some() {
            out.push(thing);
        }
        out.extend(devices(thing));
    }
    out
}

/// What a head light key asks (`SwitchHeadLights(toggle, switchMode)`):
/// H each device's on flipped, Ctrl+H each one's mode one up.
pub fn head_request(found: &[&Item], toggle: bool, switch_mode: bool) -> Vec<(Id, Light)> {
    found
        .iter()
        .map(|d| {
            let mut light = d.light;
            if toggle {
                light.on = !light.on;
            }
            if switch_mode {
                light.mode = next_mode(light.mode, d.kind.light_modes.unwrap_or(0));
            }
            (d.id, light)
        })
        .collect()
}

/// A Unity rotation (`xyzw`) from its Euler angles in degrees
/// (`Quaternion.Euler`: about z, then x, then y).
pub fn quat_of_euler([x, y, z]: [f32; 3]) -> [f32; 4] {
    let half = |a: f32| (a.to_radians() / 2.0).sin_cos();
    let ((sx, cx), (sy, cy), (sz, cz)) = (half(x), half(y), half(z));
    // q = qy · qx · qz
    let [ax, ay, az, aw] = [cy * sx, sy * cx, -sy * sx, cy * cx];
    [ax * cz + ay * sz, ay * cz - ax * sz, aw * sz + az * cz, aw * cz - az * sz]
}

/// A Unity rotation's Euler angles in degrees, each in [0, 360)
/// (`Quaternion.eulerAngles`).
pub fn euler_of([x, y, z, w]: [f32; 4]) -> [f32; 3] {
    let pitch = (2.0 * (w * x - y * z)).clamp(-1.0, 1.0).asin();
    let yaw = (2.0 * (w * y + x * z)).atan2(1.0 - 2.0 * (x * x + y * y));
    let roll = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (x * x + z * z));
    [pitch, yaw, roll].map(|a| a.to_degrees().rem_euclid(360.0))
}

/// A vector turned by a rotation (`xyzw`).
pub fn turn([x, y, z, w]: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // v + 2w(q×v) + 2q×(q×v)
    let q = [x, y, z];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(q, v).map(|c| 2.0 * c);
    let u = cross(q, t);
    [v[0] + w * t[0] + u[0], v[1] + w * t[1] + u[1], v[2] + w * t[2] + u[2]]
}

/// A head device's light or laser placed by the camera, in the game's axes
/// (`HeadLightsValidator.Update`, each frame for one's own player): at the
/// camera, moved 0.2 m to its side, up and ahead along its own axes as
/// turned the frame before, then turned as the camera, 10° down and 3°
/// toward the middle, and by the field of view below 50 (`fov`, its whole
/// degrees taken). Its place and its new turn.
pub fn head_pose(camera_at: [f32; 3], camera_turn: [f32; 4], fov: f32, right: bool, turned_before: [f32; 4]) -> ([f32; 3], [f32; 4]) {
    let below = (50.0 - fov.floor()).clamp(0.0, 50.0);
    let euler = euler_of(camera_turn);
    let offset = if right { [0.2, 0.2, 0.2] } else { [-0.2, 0.2, 0.2] };
    let moved = turn(turned_before, offset);
    let place = [camera_at[0] + moved[0], camera_at[1] + moved[1], camera_at[2] + moved[2]];
    let added = if right { [10.0, -3.0, 0.0] } else { [10.0, 3.0, 0.0] };
    let new = [euler[0] + added[0] - 0.2 * below, euler[1] + added[1] - 0.45 * below, euler[2] + added[2]];
    (place, quat_of_euler(new))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Kind;
    use std::sync::Arc;

    fn device(class: &str, modes: u32) -> Item {
        Item::new(Arc::new(Kind { light_modes: Some(modes), class_names: vec![class.into()], ..Kind::default() }))
    }

    #[test]
    fn hip_penalty_by_ergonomics_and_combo() {
        // The recorded figures (`courses6`, course 7): the M60E6 FDE's E
        // 4.75 and the M4A1's 45.57, off and with the combo unit on.
        assert!((hip_penalty(4.75, false) - 1.0).abs() < 1e-6);
        assert!((hip_penalty(4.75, true) - 0.288125).abs() < 1e-6);
        assert!((hip_penalty(45.57, false) - 0.96772).abs() < 1e-5);
        assert!((hip_penalty(45.57, true) - 0.186075).abs() < 1e-5);
        assert!((hip_penalty(76.26, false) - 0.84496).abs() < 1e-5);
    }

    #[test]
    fn euler_round_trip_as_unity_orders_it() {
        for angles in [[10.0, 91.0, 0.0], [350.0, 270.5, 2.0], [0.0, 0.0, 0.0], [45.0, 30.0, 20.0]] {
            let back = euler_of(quat_of_euler(angles));
            for (a, b) in angles.iter().zip(back) {
                let d = (a - b).rem_euclid(360.0);
                assert!(d < 1e-3 || d > 360.0 - 1e-3, "{angles:?} came back {back:?}");
            }
        }
        // 90° about y turns forward (+z) to the right (+x).
        let f = turn(quat_of_euler([0.0, 90.0, 0.0]), [0.0, 0.0, 1.0]);
        assert!((f[0] - 1.0).abs() < 1e-5 && f[2].abs() < 1e-5);
        // 10° about x pitches forward down (Unity: x positive is down).
        let f = turn(quat_of_euler([10.0, 0.0, 0.0]), [0.0, 0.0, 1.0]);
        assert!(f[1] < 0.0 && (f[1] + 10f32.to_radians().sin()).abs() < 1e-5);
    }

    /// The recorded head light (`courses6`, course 7, the M60E6 FDE's
    /// `04-head-light`, frame 300): the camera at (0.1204, 1.5166, −0.0973)
    /// from the player's origin turned 91.08° (yaw) and −0.576° (pitch) at a
    /// field of view of 74, the WF-501B's light on the left at (0.3641,
    /// 1.6812, 0.0858) pointing (0.9840, −0.1637, −0.0703). Placed twice: the
    /// second time the light moves along its turn of the first, as the game's
    /// does along the frame before's.
    #[test]
    fn the_head_light_sits_where_the_recording_has_it() {
        let camera = quat_of_euler([-0.576, 91.08, 0.0]);
        let (_, first) = head_pose([0.1204, 1.5166, -0.0973], camera, 74.0, false, [0.0, 0.0, 0.0, 1.0]);
        let (at, turned) = head_pose([0.1204, 1.5166, -0.0973], camera, 74.0, false, first);
        for (ours, theirs) in at.iter().zip([0.3641, 1.6812, 0.0858]) {
            assert!((ours - theirs).abs() < 0.004, "{at:?}");
        }
        let way = turn(turned, [0.0, 0.0, 1.0]);
        for (ours, theirs) in way.iter().zip([0.9840, -0.1637, -0.0703]) {
            assert!((ours - theirs).abs() < 0.003, "{way:?}");
        }
    }

    #[test]
    fn modes_wrap_by_their_count() {
        assert_eq!((next_mode(2, 3), next_mode(0, 1), next_mode(1, 4)), (0, 0, 2));
    }

    #[test]
    fn head_light_switches_at_0_8_and_unlocks_at_1_4() {
        let mut head = HeadLight::default();
        assert!(head.press() && head.locked() && !head.press());
        let dt = 1.0 / 120.0;
        let (mut t, mut switched, mut unlocked) = (0.0, None, None);
        while unlocked.is_none() {
            t += dt;
            let step = head.step(dt);
            if step.switched {
                switched = Some(t);
            }
            if step.unlocked {
                unlocked = Some(t);
            }
        }
        let (s, u) = (switched.unwrap(), unlocked.unwrap());
        assert!((0.8..0.8 + dt * 1.01).contains(&s) && (1.4..1.4 + dt * 2.01).contains(&u) && !head.locked());
    }

    /// A weapon with a combo unit in its slot and a flashlight on the unit's.
    fn weapon_with_two() -> (Item, Id, Id) {
        use trp_data::inventory::Holder;
        let holder = |name: &str| Holder { name: name.into(), ..Holder::default() };
        let mut combo = Item::new(Arc::new(Kind { slots: vec![holder("mod_flashlight")], light_modes: Some(3), class_names: vec!["TacticalCombo".into()], ..Kind::default() }));
        let flashlight = device("Flashlight", 1);
        let (combo_id, flashlight_id) = (combo.id, flashlight.id);
        combo.slots[0] = Some(flashlight);
        let mut weapon = Item::new(Arc::new(Kind { slots: vec![holder("mod_tactical")], ..Kind::default() }));
        weapon.slots[0] = Some(combo);
        (weapon, combo_id, flashlight_id)
    }

    #[test]
    fn keys_ask_of_every_device_nested_ones_too() {
        let (mut weapon, combo, flashlight) = weapon_with_two();
        assert_eq!(devices(&weapon).iter().map(|d| d.id).collect::<Vec<_>>(), vec![combo, flashlight]);
        // Press mode flips each one on its own: on and off swap.
        apply(&mut weapon, &[(flashlight, Light { on: true, mode: 0 })]);
        let toggle = weapon_request(&weapon, WeaponKey::Toggle).unwrap();
        assert!(toggle.animated && !toggle.forced && !toggle.sound);
        assert_eq!(toggle.states, vec![(combo, Light { on: true, mode: 0 }), (flashlight, Light { on: false, mode: 0 })]);
        // Hold mode: down asks only where not all are on, up only where any is.
        let down = weapon_request(&weapon, WeaponKey::Down).unwrap();
        assert!(!down.animated && down.sound && down.states.iter().all(|(_, l)| l.on));
        apply(&mut weapon, &down.states);
        assert!(weapon_request(&weapon, WeaponKey::Down).is_none() && combo_on(&weapon));
        let up = weapon_request(&weapon, WeaponKey::Up).unwrap();
        assert!(up.forced && !up.animated && up.sound);
        apply(&mut weapon, &up.states);
        assert!(weapon_request(&weapon, WeaponKey::Up).is_none() && !combo_on(&weapon));
        // Ctrl+T: each mode up by its own count, an off one's too.
        let next = weapon_request(&weapon, WeaponKey::Next).unwrap();
        assert_eq!(next.states, vec![(combo, Light { on: false, mode: 1 }), (flashlight, Light { on: false, mode: 0 })]);
        // A thing with none asks nothing.
        assert_eq!(weapon_request(&device("Flashlight", 1), WeaponKey::Toggle), None);
    }

    #[test]
    fn a_flashlight_alone_is_no_combo_unit() {
        let (mut weapon, _, flashlight) = weapon_with_two();
        apply(&mut weapon, &[(flashlight, Light { on: true, mode: 0 })]);
        assert!(!combo_on(&weapon));
    }
}
