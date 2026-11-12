//! The battle view's state, the quick bar's glow and the picker's cells.

use super::*;

/// The panels' state as their scripts keep it.
#[derive(Resource, Default)]
pub(super) struct BattleState {
    /// Whether the battle screen is up (the inventory shut and the
    /// character alive); each time it comes up its panels are shown afresh
    /// (`BattleUIScreen.Show`), by the settings as they then stand.
    pub(super) up: bool,
    pub(super) modes: BattleModes,
    pub(super) stance: Fader,
    pub(super) health: Fader,
    pub(super) quick: Fader,
    /// The changes of the hands begun, as last seen (`HandsHeld`).
    pub(super) hands_seen: u32,
    pub(super) seen: Option<Seen>,
    pub(super) ladder: Ladder,
    /// The stance the figure was last set for (`_currentStanceImageType`)
    /// and the slider's value; the speed slider's value and most; the
    /// noise's level; each notch's being had.
    stance_type: f32,
    pub(super) stance_value: f32,
    pub(super) speed: [f32; 2],
    pub(super) noise: usize,
    pub(super) notches: [bool; 21],
    /// The sprint holds the stance panel up (`_sprintDisplayBattleStance`);
    /// the health panel was last raised a while (`AutoHide`).
    pub(super) sprint_holds: bool,
    pub(super) health_autohide: bool,
    /// The effects' icons shown: each one's effect, by the game's name, and
    /// how many of it.
    pub(super) effects: Vec<(&'static str, usize)>,
    /// The mount's icon (`BattleUiBipodPanel`): the side's number of the
    /// mount shown (`ChangeMountImage`: none, not mounted), the red flash
    /// under way and how far into it (`OnMountUnavailable`), and the
    /// searches refused as last seen.
    pub(super) mount_side: Option<i32>,
    pub(super) mount_flash: Option<f32>,
    pub(super) refusals_seen: u32,
    /// Whether the magazine picker was shown, as last seen.
    pub(super) picking: bool,
    /// The quick bar's pistols' glow under way after a malfunction
    /// (`ShowQuickdrawGlowForPistols`), and the quick draws of the
    /// holster's weapon begun, as last seen (`HandsHeld::quickdraws`).
    pub(super) quickdraw: Option<QuickdrawGlow>,
    pub(super) quickdraws_seen: u32,
}

/// The quick draw's glow on the battle bar's pistols after a malfunction
/// (`InventoryScreenQuickAccessPanel.ShowQuickdrawGlowForPistols`,
/// `QuickSlotView.ShowQuickdrawGlow`): the views it runs on, by their key;
/// how far into it; and how many legs it runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct QuickdrawGlow {
    pub(super) slots: Vec<String>,
    pub(super) at: f32,
    pub(super) legs: u32,
}

/// The glow's leg, its colour and its alpha at the top
/// (`ShowQuickdrawGlow`: the view's `InstallPlace` set to (84, 193, 255) at
/// alpha 0, then `DOFade(0.3, 0.175)` looped back and forth, `LoopType.Yoyo`).
const GLOW_LEG: f32 = 0.175;

pub(super) const GLOW_RGB: [u8; 3] = [84, 193, 255];

const GLOW_TOP: f32 = 0.3;

/// How many legs a glow of so many seconds runs (`Mathf.CeilToInt(duration
/// / 0.175)`): 28 for the install's 4.85 (`TimeToQuickdrawPistol` 5 less the
/// 150 ms), so 4.9 s.
pub(super) fn glow_legs(duration: f32) -> u32 {
    (duration / GLOW_LEG).ceil().max(0.0) as u32
}

/// The glow's alpha so far into it, none once its legs have run (the view
/// then back at rest: the tween's end calls `method_0`): each leg 0.175 s,
/// the first and every other up from 0 to 0.3 by DOTween's default ease
/// (out quad, as the panels' fades), the rest the same played backwards.
pub(super) fn glow_alpha(at: f32, legs: u32) -> Option<f32> {
    if at < 0.0 || at >= legs as f32 * GLOW_LEG {
        return None;
    }
    let leg = (at / GLOW_LEG).floor() as u32;
    let x = ((at - leg as f32 * GLOW_LEG) / GLOW_LEG).clamp(0.0, 1.0);
    let x = if leg % 2 == 0 { x } else { 1.0 - x };
    Some(GLOW_TOP * (1.0 - (1.0 - x) * (1.0 - x)))
}

/// Whether a quick view shows a slot of the equipment (`BoundSlotView`: the
/// sheath's, the holster's, the sling's, the back's), not a key's binding
/// (`BoundItemView`) nor the grenade's.
fn slot_view(key: &str) -> bool {
    matches!(key, "ItemV" | "V" | "Item1" | "1" | "Item2" | "2" | "Item3" | "3")
}

/// The views a malfunction's glow runs on (`ShowQuickdrawGlowForPistols`):
/// each slot view whose thing is a weapon of the class "pistol"
/// (`WeapClass`), but one with a malfunction the shooter knows of
/// (`ShowQuickdrawGlow` returns for it).
pub(super) fn glowing_slots<'a>(keys: impl Iterator<Item = &'a str>, thing: impl Fn(&str) -> Option<&'a Item>, pistol: impl Fn(&Item) -> bool) -> Vec<String> {
    keys.filter(|key| slot_view(key))
        .filter(|key| thing(key).is_some_and(|item| SlotMark::of(item) == Some(SlotMark::Weapon) && pistol(item)))
        .map(str::to_owned)
        .collect()
}

/// Whether a weapon is of the class "pistol" (its template's `weapClass`).
pub(super) fn is_pistol(db: &trp_data::Database, item: &Item) -> bool {
    db.items.get(&item.kind.template).and_then(|template| template.props.get("weapClass")).and_then(|class| class.as_str()) == Some("pistol")
}

/// A view of a thing in the magazine picker, by its place among what the
/// picker offers (`ammo_picker::Shown::choices`).
#[derive(Component)]
pub(super) struct PickerCell(pub(super) usize);

/// The `layer` the picker's views of things are drawn on, apart from the
/// inventory's.
pub(super) const PICKER_LAYER: u8 = 200;

/// The red mount icon's alpha so far into its flash of `length` seconds
/// (`OnMountUnavailable`: `DOFade` to whole in half of it, then out in the
/// other half, DOTween's default ease, out quad).
pub(super) fn mount_flash_alpha(at: f32, length: f32) -> f32 {
    let half = (length / 2.0).max(1e-6);
    let out_quad = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        1.0 - (1.0 - x) * (1.0 - x)
    };
    if at < half {
        out_quad(at / half)
    } else {
        1.0 - out_quad((at - half) / half)
    }
}

impl BattleState {
    /// `BattleStancePanel.ChangeStanceImage`: the figure nearest the stance
    /// (lying, -0.35), stepped to from the last one reached; the slider at
    /// its stance.
    pub(super) fn stance_image(&mut self, figures: &[(usize, f32)], kind: f32) {
        if (self.stance_type - kind).abs() < f32::EPSILON || figures.is_empty() {
            return;
        }
        self.stance_type = kind;
        let mut closest = 0;
        for (index, (_, factor)) in figures.iter().enumerate() {
            if (kind - factor).abs() < (kind - figures[closest].1).abs() {
                closest = index;
            }
        }
        self.ladder.to = closest;
        self.ladder.shown = self.ladder.reached;
        self.ladder.step();
        self.stance_value = figures[closest].1;
    }

    /// `ChangeMaxSpeed`: each notch had where it is no higher than the
    /// limit's share of the top speed (rounded to two places, as the
    /// game's double rounds, a half to even); the speed set to the clamped
    /// one.
    pub(super) fn max_speed(&mut self, limit: f32, clamped: f32, max: f32) {
        for (index, had) in self.notches.iter_mut().enumerate() {
            let at = if index == 20 { 1.0 } else { 0.05 * index as f32 } as f64;
            let over = ((at - (limit / max) as f64) * 100.0).round_ties_even() / 100.0;
            *had = over <= f32::EPSILON as f64;
        }
        self.speed = [clamped, max];
    }

    /// `BattleUIScreen.method_14`, on a change of the hands begun and a
    /// malfunction made known (where the screen is up): the quick bar
    /// raised, a while where it shows so (`AnimatedShow(Autohide)`), not
    /// where it never shows.
    pub(super) fn raise_quick(&mut self, modes: BattleModes) {
        if modes.quick != Shows::Never {
            self.quick.show(modes.quick == Shows::Autohide);
        }
    }

    /// `BattleUIScreen.PullUpBattleStancePanel`.
    pub(super) fn pull_up(&mut self, modes: BattleModes) {
        if !self.sprint_holds && modes.stamina != Shows::Never {
            self.stance.show(modes.stamina == Shows::Autohide);
        }
    }

    /// `BattleUIScreen.DecideHealthVisibility` (no effect of the app's is
    /// critical: only lethal intoxication is).
    pub(super) fn decide_health(&mut self, initial: bool, modes: BattleModes) {
        match modes.health {
            Shows::Never => self.health.hide(0.0),
            Shows::Always => {
                self.health_autohide = false;
                self.health.show(false);
            }
            Shows::Autohide if initial => {
                self.health_autohide = true;
                self.health.show(true);
            }
            Shows::Autohide => self.health.hide(0.0),
        }
    }
}

/// `MovementContext.CovertNoiseLevel`: 1 lying, 2 sprinting; else 0 where
/// the covert movement is nearly whole (`CovertEfficiency`, over 0.9: none
/// with a leg damaged, else the speed's share of the top under 0.4 to 0.2
/// times the Covert Movement skill's speed, as its level gives it
/// (`skills`; 1 untrained), less the walk's overweight),
/// 2 up off the ground faster than 0.8 of the top, and 1 otherwise.
pub(super) fn noise_level(controller: &Controller) -> usize {
    let m = &controller.movement;
    if m.prone {
        return 1;
    }
    if m.sprinting {
        return 2;
    }
    let ratio = m.clamped_speed() / m.settings.max_speed.max(1e-6);
    let covert = 0.4 * m.sounds.covert().speed;
    let efficiency = if m.conditions.leg_damaged() { 0.0 } else { ((ratio - covert) / -0.2).clamp(0.0, 1.0) * (1.0 - m.stamina.settings.walk_overweight) };
    if efficiency > 0.9 {
        0
    } else if m.pose_level > 0.01 && ratio > 0.8 {
        2
    } else {
        1
    }
}
