//! A recording's bots' voices set beside the speaker's rules, for `bin/botsounds --voices`.
//! After the health spec, "Any character's voice", by a bot's speaker
//! (`speaker`, `Speaker::of_ai`). Each recorded bot (the world file's `bots`:
//! its set-up's voice and side, and on every frame whether it lives, its
//! root, its head bone and its parts' points) is given a speaker of its
//! own with its voice's banks, told what the trace's `body_hits` did to it
//! frame by frame in the game's order, and what its health did besides (a
//! change of its parts no hit accounts for: a bleeding's tick, a blast's
//! damage); what the speaker says is set beside the trace's `sounds` of
//! the bot's speech (`ObservedPlayerSpeech`, or muffled `SimpleOccluded`):
//!
//! - which phrase (the bank holding the recorded clip), on which frame;
//! - whether the recorded clip is one the speaker could have drawn (its
//!   tags against those the phrase was said with);
//! - the clip's volume, the source's (`base_volume`, the speech preset's
//!   0.7), its farthest over the clip's falloff (the environment's share,
//!   1 indoors and 0.98 outdoors, less where the occlusion shortens it),
//!   the pitch, the group, and the place beside the bot's head.
//!
//! The recorded phrases of the bot's other banks (contact, mutter, enemy
//! down, fight...) are its AI's, counted apart; the exhaustion's breath is
//! not compared (a bot's stamina is not recorded).

use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;

use anyhow::{Context, Result};
use serde_json::Value;
use trp_data::audio::sound::PhraseBank;

use crate::condition::health::{DamageKind, Part};
use crate::sound::speaker::{compare, tag, trigger, Bank, Said, Situation, Speaker, Status};

mod reading;
mod replaying;
mod comparing;

pub use reading::*;
pub use replaying::*;
pub use comparing::*;
use reading::{gap};
use replaying::{KINDS};

type V3 = [f32; 3];

/// The health's banks; with what they chain to, the phrases this compares.
pub const HEALTH: [i32; 4] = [trigger::AGONY, trigger::BEING_HURT, trigger::DEATH, trigger::BREATH];

/// How far a Scav's group reaches (`VirtualGroup.GetGroupStatus`: 50 m).
const GROUP_REACH: f32 = 50.0;
