//! Effect sources, hit reactions and the listing of effects on screen.

use super::*;

/// The kinds of effect the accumulators take (of those the app has).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Contusion,
    LowHealth,
    Stun,
    Pain,
    PainKiller,
    Dehydration,
    Exhaustion,
    /// A stimulator's tunnel vision (`ITunnelVision`).
    TunnelVision,
    /// A medkit at work (`IMedEffect` whose item is a `MedKit`).
    MedKit,
    /// A berserk (`IBerserk`): the sharpening's alone. The wiggle's and the
    /// motion blur's tables name it (speed 4, scale 1.5; 1.4), but neither
    /// accumulator is made to take it, so neither ever does.
    Berserk,
}

impl Source {
    /// The wiggle's speed and scale for it (`_effectsSpeed`, `_effectsScale`).
    pub(super) fn wiggle(self) -> Option<[f32; 2]> {
        match self {
            Source::Contusion => Some([0.8, 8.0]),
            Source::LowHealth => Some([0.4, 5.0]),
            _ => None,
        }
    }

    /// The motion blur's goal for it (`_effectsAmount`).
    pub(super) fn blur(self) -> Option<f32> {
        match self {
            Source::Contusion => Some(0.8),
            Source::LowHealth => Some(0.3),
            _ => None,
        }
    }

    /// Whether the double vision takes it (`IContusion`, `IStun`).
    pub(super) fn doubles(self) -> bool {
        matches!(self, Source::Contusion | Source::Stun)
    }

    /// Whether pain's blur takes it (`IDehydration`, `IPain`, `IPainKiller`).
    pub(super) fn radial(self) -> bool {
        matches!(self, Source::Dehydration | Source::Pain | Source::PainKiller)
    }
}

/// An effect listed on the character: its id, kind, current strength,
/// strength, and whether it is residual (`EEffectState.Residued`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listed {
    pub id: u32,
    pub source: Source,
    pub current: f32,
    pub strength: f32,
    pub residual: bool,
    /// Started (or residual): the desaturation's accumulators take an
    /// effect as it starts (`EffectStartedEvent`).
    pub started: bool,
}

/// What a hit does to the view (`EffectsController.OnPlayerDamaged`): the
/// force reaction's push (`ForceEffector.AddForce`'s strength, and its
/// shares for the hands and the camera) and the hit blur's power.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitReaction {
    pub strength: f32,
    pub hands: f32,
    pub camera: f32,
    pub blur_power: f32,
    /// Blood on the screen (`BloodOnScreen.Hit`): any but a fall, of at
    /// least 0.1 damage (a second for the head).
    pub blood: bool,
}

/// A hit as the player is told of it (`Player.ReceiveDamage`: a shot, or
/// damage given to the player that no weapon did): its damage, what armour
/// took off it, the part, the kind, and whether the character is on
/// painkillers. Nothing for a kind the character did to itself or for none.
pub fn hit_reaction(damage: f32, absorbed: f32, part: Part, kind: DamageKind, on_painkillers: bool) -> Option<HitReaction> {
    if kind.self_inflicted() || kind == DamageKind::Undefined {
        return None;
    }
    let taken = damage + absorbed;
    let (hands, camera) = match part {
        Part::Head => (0.1, 1.3),
        Part::LeftArm | Part::RightArm => (0.15, 0.5),
        Part::LeftLeg | Part::RightLeg => (0.05, 0.3),
        _ => (0.05, 0.4),
    };
    let blur_power = if on_painkillers {
        taken
    } else if part == Part::Head {
        taken * 6.0
    } else {
        taken * 3.0
    };
    let blood = kind != DamageKind::Fall && damage >= 0.1;
    Some(HitReaction { strength: taken.max(0.0).sqrt() / AIM_PUNCH_COEFFICIENT, hands, camera, blur_power, blood })
}

/// `Mathf.InverseLerp`.
pub(super) fn inverse_lerp(a: f32, b: f32, value: f32) -> f32 {
    if a != b {
        ((value - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A curve looped over its keys' span (`WrapMode.Loop`).
pub(super) fn looped(curve: &Curve, t: f32) -> f32 {
    let (Some(first), Some(last)) = (curve.keys.first(), curve.keys.last()) else { return 0.0 };
    let span = last.time - first.time;
    if span <= 0.0 {
        return first.value;
    }
    curve.evaluate(first.time + (t - first.time).rem_euclid(span))
}

/// A hit's blur (`FastBlur` on the camera): on from a hit until its time is
/// up, its value by the curve over that time times the hit's power.
#[derive(Debug, Clone, Copy)]
pub(super) struct HitBlur {
    pub(super) on: bool,
    pub(super) timer: f32,
    pub(super) power: f32,
}

/// The listing of an accumulator of the base kind (`EffectAccumulator`):
/// the effects it takes, in the order they came, and whether it is on.
#[derive(Debug, Clone, Default)]
pub(super) struct Listing {
    pub(super) ids: Vec<u32>,
    pub(super) on: bool,
}

impl Listing {
    /// Those new of its kinds added (`AddEffect`), each turning it on.
    pub(super) fn add(&mut self, listed: &[Listed], takes: impl Fn(Source) -> bool) {
        for e in listed.iter().filter(|e| takes(e.source)) {
            if !self.ids.contains(&e.id) {
                self.ids.push(e.id);
                self.on = true;
            }
        }
    }

    /// Those gone taken off (`DeleteEffect`): whether that left it empty.
    pub(super) fn remove(&mut self, listed: &[Listed]) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| listed.iter().any(|e| e.id == *id));
        before > 0 && self.ids.is_empty()
    }

    /// The sum of the current strengths of those it takes (`CurrentAmount`).
    pub(super) fn sum(&self, listed: &[Listed]) -> f32 {
        self.ids.iter().filter_map(|id| listed.iter().find(|e| e.id == *id)).map(|e| e.current).sum()
    }
}
