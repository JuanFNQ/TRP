//! The hands that hold a knife or another melee weapon.
//! After `docs/spec/grenades-and-knife.md`, "The knife's hands": its rig from
//! the item's prefab, its animator, and its controller (`KnifeController`):
//! drawn, at rest, a slash on the left button and a stab on the right, the
//! blade live between the stroke's two marks, a hit glancing off; and the
//! quick stroke (`QuickKnifeKickController`), one stroke and back. This is
//! that controller on the rig's own animator. The host sweeps the blade for
//! what it meets while it is live (`KnifeEvent::Live`) and says when it hit
//! (`hit`), and hands the body's animator what the controller tells it
//! (`BodyAction`, `BodySwingSpeed`, `BodyDeflected`: the strokes a character
//! seen from outside plays; grenades-and-knife spec, "The knife's strokes
//! seen from outside").

use crate::weapons::weapon_animator::{WeaponAnimator, WeaponAnimatorData};

mod blade;
mod working;

pub use blade::*;

/// The engine's physics step (`Time.fixedDeltaTime`: a sixtieth of a second
/// in every recording's header, `fixed_dt`), which the quick stroke's
/// animator is moved on by as it is made.
const FIXED_DT: f32 = 1.0 / 60.0;

/// A stroke's kind (`EKickType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Slash,
    Stab,
}

/// What a melee weapon's item says of its strokes (its `KnifeComponent`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnifeSettings {
    /// How far each stroke reaches (`PrimaryDistance`, `SecondryDistance`).
    pub slash_reach: f32,
    pub stab_reach: f32,
    /// What each stroke costs (`PrimaryConsumption`, `SecondryConsumption`),
    /// and a stroke glancing off a deflecting surface besides
    /// (`DeflectionConsumption`).
    pub slash_cost: f32,
    pub stab_cost: f32,
    pub deflection_cost: f32,
    /// Each stroke's damage (`knifeHitSlashDam`, `knifeHitStabDam`).
    pub slash_damage: f32,
    pub stab_damage: f32,
    /// Each stroke's penetration (`SlashPenetration`, `StabPenetration`;
    /// every melee weapon of the install 1), and the stamina a blow burns
    /// on the character it reaches for each point of its damage
    /// (`StaminaBurnRate`; 0 but the three hammers' 1, 2 and 1): what its
    /// hit carries (`BaseKnifeController.ProcessHit`).
    pub slash_penetration: f32,
    pub stab_penetration: f32,
    pub stamina_burn_rate: f32,
    /// Whether it is drawn on the character while sheathed
    /// (`DisplayOnModel`; three of the install's twenty-five). The game
    /// gives the hit of such a one a sound of its own
    /// (`combat::impacts::Impact::knife`).
    pub shown_on_model: bool,
}

impl KnifeSettings {
    /// A melee weapon's figures from its item, as the database has them.
    pub fn of(props: &serde_json::Value) -> Self {
        let number = |name: &str| props[name].as_f64().unwrap_or(0.0) as f32;
        Self {
            slash_reach: number("PrimaryDistance"),
            stab_reach: number("SecondryDistance"),
            slash_cost: number("PrimaryConsumption"),
            stab_cost: number("SecondryConsumption"),
            deflection_cost: number("DeflectionConsumption"),
            slash_damage: number("knifeHitSlashDam"),
            stab_damage: number("knifeHitStabDam"),
            slash_penetration: number("SlashPenetration"),
            stab_penetration: number("StabPenetration"),
            stamina_burn_rate: number("StaminaBurnRate"),
            shown_on_model: props["DisplayOnModel"].as_bool().unwrap_or(false),
        }
    }
}

/// What the knife's hands tell the host.
#[derive(Debug, Clone, PartialEq)]
pub enum KnifeEvent {
    /// Drawn: the draw's mark (`WeapIn`).
    Appeared,
    /// Put away (`WeapOut`): what is taken in hand next is made now.
    Hidden,
    /// The blade is live (`FiringBullet`): the stroke's stamina is to be
    /// spent (`ConsumeAsMelee`) and the host sweeps the blade every frame,
    /// by this reach along the look, until `Dead`; at most one hit a stroke.
    Live { stroke: Stroke, reach: f32, cost: f32 },
    /// The stroke is over (`FireEnd`).
    Dead,
    /// The quick stroke is over: the hands go back to what they held.
    Used,
    Sound(String),
    /// The body's animator told what to do (a `ThirdAction` mark of the
    /// blade's states: `Body::set_weapon_action`).
    BodyAction(i32),
    /// The body's animator told the swing's speed and whether the stroke
    /// glanced off, as the blade's own is (`KnifeController.SetMeleeSpeed`,
    /// `SetDeflected`, which tell both animators): for `Body::
    /// set_swing_speed` and `Body::set_deflected`. Given at the next step's
    /// start, before its marks.
    BodySwingSpeed(f32),
    BodyDeflected(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Away,
    Spawn,
    Idle,
    /// A stroke (`Kick`).
    Kick,
    /// The quick stroke (`QuickKnifeKickOperation`), until its end lets
    /// the controller go.
    Quick,
    Remove,
}

pub struct KnifeHands {
    animator: WeaponAnimator,
    settings: KnifeSettings,
    operation: Operation,
    stroke: Stroke,
    /// The blade is live and has not hit yet.
    live: bool,
    hide_asked: bool,
    drawn_smooth: bool,
    /// The character's melee speed as the host last told it
    /// (`Physical.MeleeSpeed`), which the put-away tells the body.
    melee_speed: f32,
    /// What the controller told the body's animator since the last step.
    told_body: Vec<KnifeEvent>,
    /// The swing's way, a hit's direction (`Swing`): nought with each
    /// controller made (a draw, a quick stroke).
    swing: Swing,
}

#[cfg(test)]
mod tests;
