//! A helmet's cut of what is heard.
//! After `trp_sim::hearing`; sound spec, "Hearing under headgear": the template chosen by what the
//! character wears on its head, applied once the game's mixer is read (its first fade from the
//! mixer's start snapshot) and again whenever the head's things change; its fades stepped each
//! frame on the game's clock; and each group of the world's sounds (`ui_sound::Group`) set to its
//! level as heard, a contusion's tinnitus (`tinnitus`) dipping the dry way alone. With `HEADGEAR
//! HEARING` off no template is ever applied: every group at the tinnitus' dip or 0 dB.

use bevy::prelude::*;
use trp_sim::sound::hearing::{choose, worn_key, Group, Headphones};

use crate::screens::inventory_view::Carried;
use crate::audio::ui_sound::{set_level, Mixer};

/// The headphones as the game keeps them, made once the mixer is read;
/// what was worn on the head when a template was last applied; the
/// tinnitus' dip while it rings.
#[derive(Resource)]
pub struct Hearing {
    on: bool,
    settings: trp_data::audio::sound::HeadphonesSettings,
    headphones: Option<Headphones>,
    worn: Option<u64>,
    dip: Option<f32>,
    ringing: bool,
}

impl Hearing {
    /// A frame of a contusion's tinnitus: its dip (dB, `MainChannelLevel`),
    /// held until the next.
    pub fn ring(&mut self, dip: f32) {
        self.dip = Some(dip);
    }

    /// The tinnitus over.
    pub fn quiet(&mut self) {
        self.dip = None;
    }
}

/// The hearing, with the headphones' fades from the client settings
/// (`AudioSettings.HeadphonesSettings`), on or off by its switch; its frame
/// after the tinnitus'.
pub fn start(app: &mut App, on: bool) {
    let settings = trp_data::audio::sound::headphones_settings(&app.world().resource::<crate::GameData>().db.client);
    if on {
        info!("headgear hearing: a template fades over {:.2} s, in by {:?}, out by {:?}", settings.fade_duration, settings.fade_in, settings.fade_out);
    }
    app.insert_resource(Hearing { on, settings, headphones: None, worn: None, dip: None, ringing: false });
    app.add_systems(Update, hear.after(crate::audio::tinnitus::ring));
}

/// The frame's hearing: the template applied where the head's things have
/// changed (`PROVISIONAL`: the game applies it again on any thing moved to
/// or from a slot, which tells only while a fade runs), the fades stepped,
/// the tinnitus' dip laid over the guns' and main groups' dry faders, and
/// every group's level set.
pub fn hear(mut hearing: ResMut<Hearing>, carried: Res<Carried>, mixer: Option<Res<Mixer>>, time: Res<Time>) {
    let Some(mixer) = mixer else { return };
    let hearing = &mut *hearing;
    // (A muffled voice seen from outside is not under `Main`: no dip.)
    let as_before = |dip: Option<f32>| {
        for group in Group::ALL {
            set_level(&mixer.0.level(group), if group == Group::SimpleOccluded { 0.0 } else { dip.unwrap_or(0.0) });
        }
    };
    if !hearing.on {
        as_before(hearing.dip);
        return;
    }
    if hearing.headphones.is_none() {
        // Until the mixer is read, as before.
        let Some(game) = mixer.0.game() else {
            as_before(hearing.dip);
            return;
        };
        hearing.headphones = Some(Headphones::new(hearing.settings, game.headphones_start.unwrap_or_else(Headphones::default_start)));
    }
    let Some(headphones) = hearing.headphones.as_mut() else { return };
    let worn = worn_key(&carried.equipment);
    if hearing.worn != Some(worn) {
        hearing.worn = Some(worn);
        let choice = choose(&carried.equipment);
        headphones.apply(choice.template());
        info!("headgear hearing: {choice:?}, the template {}", choice.template().name);
    }
    headphones.step(time.delta_secs());
    match hearing.dip {
        Some(dip) => headphones.ring(dip),
        None if hearing.ringing => headphones.quiet(),
        None => {}
    }
    hearing.ringing = hearing.dip.is_some();
    for group in Group::ALL {
        set_level(&mixer.0.level(group), headphones.level(group));
    }
}
