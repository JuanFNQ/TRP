//! The effects read away from the frame.
//!
//! Nothing on the first frame shows them: the muzzles' fumes, sparks and
//! heat haze (`particles_view`), the tracers (`tracer_view`), the grenades'
//! explosions, the hits' older emitters and the muzzles' blasts
//! (`explosion_view`), a grenade's scorch (`decal_view`). They are read on
//! one worker with a reader of its own and the run's database, in the order
//! they are likely first wanted (a shot's first), their pictures made with
//! handles reserved from the engine's store (`background::PendingImages`),
//! and put in on the frame they come (`receive_effects`). The warm-up waits
//! for them (`EffectsComing`), so that the first shot or blast draws as
//! when they were read at the start, and their pictures go up to the card
//! under the curtain. The bullets' hits' entries are those the start read
//! for the bullets (`material_entries`, read once where it was twice). The
//! bullets' holes are read on a thread of their own (`decal_view`).

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};

/// Which effects are read: the options' switches and the hard settings
/// (`PARTICLES` with the shot effects, `TRACERS` with `BALLISTICS`, the
/// scorch with the deferred decals), and the hits' entries' names.
#[derive(Resource, Clone)]
pub struct EffectsWanted {
    pub particles: bool,
    pub tracers: bool,
    pub scorches: bool,
    pub impact_names: Vec<String>,
}

pub struct EffectsReadsPlugin(pub EffectsWanted);

impl Plugin for EffectsReadsPlugin {
    fn build(&self, app: &mut App) {
        let wanted = &self.0;
        if !(wanted.particles || wanted.tracers || wanted.scorches) {
            return;
        }
        app.insert_resource(self.0.clone()).add_systems(Startup, start_reading).add_systems(First, receive_effects);
    }
}

/// The effects being read (`start_reading`); the warm-up waits while it
/// is here.
#[derive(Resource)]
pub struct EffectsComing(Task<Read>);

/// What the worker read: each effect's resource where it could be read,
/// and their pictures.
pub struct Read {
    fumes: Option<crate::effects::particles_view::Fumes>,
    sparks: Option<crate::effects::particles_view::Sparks>,
    hazes: Option<crate::effects::particles_view::Hazes>,
    tracers: Option<crate::weapons::tracer_view::Tracers>,
    explosions: Option<crate::effects::explosion_view::Explosions>,
    scorches: Option<crate::effects::decal_view::Scorches>,
    images: crate::assets::background::PendingImages,
    took: f32,
}

fn start_reading(mut commands: Commands, game: Res<crate::GameData>, images: Res<Assets<Image>>, wanted: Res<EffectsWanted>) {
    let (install, db, hard, wanted) = (game.unity.install.clone(), game.db.clone(), game.hard.clone(), wanted.clone());
    let mut pending = crate::assets::background::PendingImages::new(&images);
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let began = std::time::Instant::now();
        let (mut fumes, mut sparks, mut hazes, mut tracers, mut explosions, mut scorches) = (None, None, None, None, None, None);
        match trp_data::unity::Assets::new(install) {
            Ok(mut unity) => {
                if wanted.particles {
                    fumes = crate::effects::particles_view::prepare_fumes(&mut unity, &mut pending);
                    sparks = crate::effects::particles_view::prepare_sparks(&mut unity, &mut pending);
                    hazes = crate::effects::particles_view::prepare_hazes(&mut unity, &hard, &mut pending);
                }
                if wanted.tracers {
                    tracers = crate::weapons::tracer_view::prepare_tracers(&mut unity, &mut pending);
                }
                if wanted.particles {
                    explosions = crate::effects::explosion_view::prepare_explosions(&mut unity, &db, &wanted.impact_names, &mut pending);
                }
                if wanted.scorches {
                    scorches = crate::effects::decal_view::prepare_scorches(&mut unity, &db, &mut pending);
                }
            }
            Err(e) => warn!("the effects are not drawn: their reader could not be made: {e:#}"),
        }
        Read { fumes, sparks, hazes, tracers, explosions, scorches, images: pending, took: began.elapsed().as_secs_f32() }
    });
    commands.insert_resource(EffectsComing(task));
}

/// The effects read: their pictures into the store, each into the world.
fn receive_effects(mut commands: Commands, coming: Option<ResMut<EffectsComing>>, mut images: ResMut<Assets<Image>>) {
    let Some(mut coming) = coming else { return };
    let Some(read) = crate::assets::background::take(&mut coming.0, false, "the effects") else { return };
    commands.remove_resource::<EffectsComing>();
    let Read { fumes, sparks, hazes, tracers, explosions, scorches, images: pending, took } = read;
    let bytes = pending.put(&mut images);
    if let Some(fumes) = fumes {
        commands.insert_resource(fumes);
    }
    if let Some(sparks) = sparks {
        commands.insert_resource(sparks);
    }
    if let Some(hazes) = hazes {
        commands.insert_resource(hazes);
    }
    if let Some(tracers) = tracers {
        commands.insert_resource(tracers);
    }
    if let Some(explosions) = explosions {
        commands.insert_resource(explosions);
    }
    if let Some(scorches) = scorches {
        commands.insert_resource(scorches);
    }
    info!("the effects: read away from the frame in {:.0} ms, their pictures ({:.1} MB) put in", took * 1000.0, bytes as f32 / (1 << 20) as f32);
}
