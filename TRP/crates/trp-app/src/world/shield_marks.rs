//! A face shield's marks on the range.
//! After the body hits spec, "A face shield's marks"; `trp_sim::armor::Mark`, `MarkLimit`: each
//! mark the armour found, on a dummy's shield or on the character's own, made by the one who shot
//! or threw, here always the player (every bullet, fragment and blast on the range is the
//! player's), while it is found among the living and within its limit of four marks a fixed step
//! (`Player.FaceshieldMarkOperation`). A mark is a hit more on the shield's item
//! (`trp_sim::inventory::mark`), which the camera hears and draws as cracks (`visor_view`). With
//! `FACE SHIELD CRACKS` off there is no `ShieldMarks` and nothing is marked.

use bevy::prelude::*;
use trp_sim::condition::armor::{Mark, MarkLimit};

use crate::player::controller::Controller;
use crate::screens::inventory_view::Carried;

/// The player's limit, the marks on its own shields still to make (a
/// blast's on its own armour: `blast_view`), the game's fixed step
/// (`Time.fixedDeltaTime`, the install's), and whether it was alive as the
/// last frame's marks were made.
#[derive(Resource)]
pub struct ShieldMarks {
    limit: MarkLimit,
    pub own: Vec<Mark>,
    fixed_dt: f32,
    was_alive: bool,
}

impl ShieldMarks {
    pub fn new(fixed_dt: f32) -> Self {
        Self { limit: MarkLimit::default(), own: Vec::new(), fixed_dt, was_alive: true }
    }
}

/// The frame's marks made, after every hit and blast of the frame and
/// before the camera hears the shields (`visor_view::visor_frame`): the
/// dummies' first, then the character's own (`PROVISIONAL`: within a frame
/// the game makes them in the order its characters are hit, which matters
/// only past the limit). The player is found among the living while alive,
/// and in the frame of its death (the game takes a dead one out of its
/// list a frame after: grenade spec, "The blast"); a mark not made by the
/// limit is lost. Without the switch the armour's marks are let go.
pub fn mark_shields(marks: Option<ResMut<ShieldMarks>>, dummies: Option<ResMut<crate::world::dummy::Dummies>>, mut carried: ResMut<Carried>, controller: Res<Controller>, time: Res<Time>) {
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    let on_dummies = match dummies {
        Some(mut dummies) if !dummies.marks.is_empty() => std::mem::take(&mut dummies.marks),
        _ => Vec::new(),
    };
    let Some(mut marks) = marks else { return };
    if on_dummies.is_empty() && marks.own.is_empty() {
        if marks.was_alive != alive {
            marks.was_alive = alive;
        }
        return;
    }
    let own = std::mem::take(&mut marks.own);
    let found = alive || marks.was_alive;
    marks.was_alive = alive;
    if !found {
        return;
    }
    let (now, fixed_dt) = (time.elapsed_secs(), marks.fixed_dt);
    let queued = on_dummies.into_iter().map(|(which, mark)| (Some(which), mark)).chain(own.into_iter().map(|mark| (None, mark)));
    for (body, mark) in queued {
        let Some(item) = mark.item else { continue };
        if !marks.limit.allow(now, fixed_dt) {
            info!("a face shield's mark not made: four already in this fixed step");
            continue;
        }
        let root = match body {
            Some(which) => carried.body_mut(which),
            None => Some(&mut carried.equipment.item),
        };
        let Some(root) = root else { continue };
        if trp_sim::items::inventory::mark(root, item) {
            let hits = root.find(item).map_or(0, |shield| shield.hits);
            info!("a face shield marked ({}): {hits} hits", body.map_or("the character's own".to_owned(), |which| format!("dummy {which}'s")));
        }
    }
}
