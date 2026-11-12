//! Running a voice trace through the speakers and what they say.

use super::*;

pub(super) const KINDS: [DamageKind; 16] = [
    DamageKind::Undefined,
    DamageKind::Fall,
    DamageKind::Barbed,
    DamageKind::Existence,
    DamageKind::Medicine,
    DamageKind::Bullet,
    DamageKind::Melee,
    DamageKind::Blunt,
    DamageKind::GrenadeFragment,
    DamageKind::Explosion,
    DamageKind::LightBleeding,
    DamageKind::HeavyBleeding,
    DamageKind::Dehydration,
    DamageKind::Exhaustion,
    DamageKind::Stimulator,
    DamageKind::Poison,
];

/// A phrase the rule said: its frame, its bank's phrase and place, its
/// clip, and the tags it was drawn with.
#[derive(Debug, Clone, Copy)]
pub struct Phrase {
    pub frame: i64,
    pub trigger: i32,
    pub bank: usize,
    pub clip: usize,
    pub tags: u32,
}

/// A loss of no recorded hit is taken for a bleeding's tick (the
/// character's own damage) where it is one part's alone or the same in
/// every part it took that it did not empty (a bleeding on a destroyed
/// limb, which the game takes from each living part alike); else for a
/// blast's (an enemy's, not a weapon's, its biggest share the part
/// struck). This tool's reading.
fn bleeding(drops: &[(Part, f32, bool)]) -> bool {
    let whole: Vec<f32> = drops.iter().filter(|(_, _, emptied)| !emptied).map(|(_, d, _)| *d).collect();
    let most = whole.iter().copied().fold(0.0, f32::max);
    let least = whole.iter().copied().fold(f32::INFINITY, f32::min);
    drops.len() == 1 || whole.is_empty() || most - least <= 0.01 * most
}

/// A recorded phrase of the bot's: its frame, its bank's phrase, its clip.
pub type Heard = (i64, i32, usize);

/// What the recording does not record of a bot but shows: muffled all
/// along (its speech heard in `SimpleOccluded`), on painkillers all along
/// (its own medicine, the AI's: tried both ways, `compare_all`), and its
/// breath heard while each exhaustion's breath it was heard saying plays
/// (its stamina is not recorded).
#[derive(Debug, Clone, Copy, Default)]
pub struct Given {
    pub muffled: bool,
    pub painkillers: bool,
}

/// Why the rule said a phrase, where the recording cannot tell all of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The hits and the health as recorded.
    Recorded,
    /// A bleeding's sixteenth tick: the count's target goes by the
    /// character's own damaging effects at work, which are not recorded.
    Bleeding,
    /// A death by a blast: the part it reached last decides the cry (the
    /// head's, silence), which is not recorded.
    BlastDeath,
}

/// One bot's speaker run over the recording: what it said, and what it was
/// told that the trace's hits do not account for. Where the recording has
/// the bot say a phrase on a frame, the speaker's draw of that bank's clip
/// is steered to it, and a hurt's chain to what followed in the recording
/// (`Speaker::steer`): what the speaker does after (how long it is busy,
/// its chain) then goes by the game's draws.
#[allow(clippy::too_many_arguments)]
pub fn run(setup: &Setup, all: &[Setup], banks: &[PhraseBank], frames: &BTreeMap<i64, Frame>, hits: &BTreeMap<i64, Vec<BodyHit>>, recorded: &[Heard], given: Given, seed: u64) -> (Vec<(Phrase, Cause)>, Vec<String>) {
    let chained: Vec<i32> = banks.iter().filter(|b| HEALTH.contains(&b.trigger) && b.chain.who == 0).map(|b| b.chain.event).collect();
    let compared = |trigger: i32| HEALTH.contains(&trigger) || chained.contains(&trigger);
    let mut speaker = Speaker::of_ai(seed);
    speaker.set_banks(banks.iter().map(Bank::of).collect());
    if given.muffled {
        speaker.face_shield_seen(true, true);
    }
    let mut said = Vec::new();
    let mut notes = Vec::new();
    let t0 = frames.values().next().map_or(0.0, |f| f.time);
    let time_of = |frame: i64| frames.get(&frame).map_or(f64::NAN, |f| f.time - t0) as f32;
    let most = frames.values().find_map(|f| f.bots.get(&setup.id)?.parts).map(|parts| parts.map(|[_, m]| m));
    // The exhaustion's breaths recorded: heard from each, as long as it plays.
    let breath = banks.iter().find(|b| b.trigger == trigger::BREATH);
    let audible: Vec<(f32, f32)> = recorded
        .iter()
        .filter(|(_, tr, _)| *tr == trigger::BREATH)
        .filter_map(|&(f, _, clip)| {
            let c = breath?.clips.get(clip)?;
            (compare(c.mask, tag::AWARE) && !compare(c.mask, tag::UNAWARE)).then(|| (time_of(f), time_of(f) + c.length))
        })
        .collect();
    let mut was_audible = false;
    let mut last: Option<[f32; 7]> = None;
    let mut alive = true;
    let mut death_kind = DamageKind::LightBleeding;
    let savages: Vec<i64> = all.iter().filter(|s| s.side == "Savage" && s.id != setup.id).map(|s| s.id).collect();
    let situation = |now: [f32; 7], alive: bool, audible: bool| {
        let parts = match most {
            Some(most) => std::array::from_fn(|k| [now[k], most[k]]),
            None => now.map(|c| [c, c.max(1.0)]),
        };
        Situation { alive, status: Status::of_parts(parts), on_painkillers: given.painkillers, painkiller: given.painkillers, audible }
    };
    let take = |speaker: &mut Speaker, frame: i64, cause: Cause, said: &mut Vec<(Phrase, Cause)>| {
        for s in speaker.take_said() {
            if let Said::Phrase(p) = s {
                let trigger = speaker.bank(p.bank).map_or(0, |b| b.trigger);
                said.push((Phrase { frame, trigger, bank: p.bank, clip: p.clip, tags: speaker.last_tags() }, cause));
            }
        }
    };
    for (&number, frame) in frames {
        let t = (frame.time - t0) as f32;
        let bot = frame.bots.get(&setup.id);
        let heard = audible.iter().any(|&(from, to)| t >= from && t < to);
        // The recording's draws of this frame.
        for &(_, trigger, clip) in recorded.iter().filter(|r| r.0 == number) {
            speaker.steer(Some((trigger, clip)), None);
            let Some(bank) = banks.iter().find(|b| b.trigger == trigger) else { continue };
            if bank.chain.who != 0 || [trigger::NONE, trigger::PHRASE_NONE].contains(&bank.chain.event) {
                continue;
            }
            let end = t + bank.clips.get(clip).map_or(0.0, |c| c.length) + 0.1;
            let chained = recorded.iter().any(|&(f, tr, _)| tr == bank.chain.event && (time_of(f) - end).abs() <= 0.2);
            speaker.steer(None, Some((trigger, chained)));
        }
        // Its group's tag: a Scav with another alive within 50 m.
        if setup.side == "Savage" {
            let near = bot.is_some_and(|b| savages.iter().any(|id| frame.bots.get(id).is_some_and(|o| o.alive && gap(o.root, b.root) < GROUP_REACH)));
            speaker.group = if near { tag::COOP } else { tag::SOLO };
        }
        for hit in hits.get(&number).into_iter().flatten().filter(|h| h.profile == setup.profile) {
            if !alive {
                continue;
            }
            speaker.struck(hit.part);
            let s = situation(hit.after, hit.alive, heard);
            if hit.before.iter().zip(&hit.after).any(|(a, b)| (a - b).abs() >= 0.01) {
                speaker.breath(t, &s);
                take(&mut speaker, number, Cause::Recorded, &mut said);
            }
            speaker.damaged(hit.kind, hit.damage, t, &s);
            take(&mut speaker, number, Cause::Recorded, &mut said);
            if !hit.alive {
                speaker.died(hit.kind, t, &s);
                take(&mut speaker, number, Cause::Recorded, &mut said);
                alive = false;
            }
            speaker.hit(hit.part, hit.damage, hit.kind, t, &s);
            last = Some(hit.after);
        }
        let Some(bot) = bot else { continue };
        let mut blasted = false;
        if let (true, Some(parts)) = (alive, bot.parts) {
            let now = parts.map(|[c, _]| c);
            if let Some(was) = last {
                let drops: Vec<(Part, f32)> = Part::ALL.iter().map(|p| (*p, was[p.index()] - now[p.index()])).filter(|(_, d)| *d >= 0.01).collect();
                let total: f32 = drops.iter().map(|(_, d)| d).sum();
                let s = situation(now, bot.alive, heard);
                if !drops.is_empty() {
                    let emptied: Vec<(Part, f32, bool)> = drops.iter().map(|&(p, d)| (p, d, now[p.index()] <= 0.0)).collect();
                    let kind = if bleeding(&emptied) { DamageKind::LightBleeding } else { DamageKind::Explosion };
                    let cause = if kind == DamageKind::Explosion { Cause::Recorded } else { Cause::Bleeding };
                    if kind == DamageKind::Explosion {
                        notes.push(format!("frame {number}: {total:.2} lost with no hit recorded ({}), taken for a blast's", drops.iter().map(|(p, d)| format!("{} {d:.2}", p.name())).collect::<Vec<_>>().join(", ")));
                        let biggest = drops.iter().copied().fold((Part::Chest, 0.0), |a, b| if b.1 > a.1 { b } else { a }).0;
                        speaker.struck(biggest);
                        blasted = true;
                    }
                    death_kind = kind;
                    speaker.breath(t, &s);
                    take(&mut speaker, number, cause, &mut said);
                    for (_, d) in &drops {
                        speaker.damaged(kind, *d, t, &s);
                        take(&mut speaker, number, cause, &mut said);
                    }
                } else if was.iter().zip(&now).any(|(a, b)| b - a >= 0.01) {
                    speaker.breath(t, &s);
                    take(&mut speaker, number, Cause::Recorded, &mut said);
                }
            }
            last = Some(now);
        }
        if alive && !bot.alive {
            notes.push(format!("frame {number}: dead with no hit recorded, taken for a {}'s death", death_kind.name()));
            let s = situation(last.unwrap_or([0.0; 7]), false, heard);
            speaker.died(death_kind, t, &s);
            take(&mut speaker, number, if blasted { Cause::BlastDeath } else { Cause::Recorded }, &mut said);
            alive = false;
        }
        let s = situation(last.unwrap_or([0.0; 7]), alive, heard);
        // What its AI said (contact, mutter, enemy down...), as recorded: not
        // built, but it keeps the speaker busy at its bank's importance.
        for &(_, trigger, _) in recorded.iter().filter(|r| r.0 == number && !compared(r.1)) {
            if alive {
                speaker.say(trigger, false, t, &s);
                take(&mut speaker, number, Cause::Recorded, &mut said);
            }
        }
        if alive && heard != was_audible {
            speaker.audible_changed(t, &s);
            take(&mut speaker, number, Cause::Recorded, &mut said);
        }
        was_audible = heard;
        speaker.update(t, &s);
        take(&mut speaker, number, Cause::Recorded, &mut said);
        speaker.unsteer();
    }
    (said, notes)
}
