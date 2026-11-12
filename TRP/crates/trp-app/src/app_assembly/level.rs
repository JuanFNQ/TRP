//! The range laid out: a level file, its workbenches, the capsule, the doors, the start, and the camera's box.

use super::*;

pub(super) struct RangeSet {
    pub(super) range: world::range::Range,
    pub(super) camera_box: Option<trp_data::character::player::CameraBoxData>,
}

/// Lays out the range, places the start and what `--lay` asked for, and reads the camera's box; `None` where `--export-level` ends the run.
pub(super) fn lay_out(options: &Options, options_state: &screens::options_view::OptionsState, mut unity: &mut UnityAssets, hard: &HardSettings, movement: &mut trp_sim::Movement, ground: &mut [(trp_sim::items::inventory::Item, Vec3)], laid_from: usize) -> anyhow::Result<Option<RangeSet>> {
    // A level file (`level`), `--level`'s, to try an edit of the range in
    // Blender for the run.
    if let Some(path) = options.level.clone() {
        match world::level::Level::read(&path) {
            Ok(read) => {
                println!(
                    "the range from {}: {} boxes, {} props placed, {} starts, {} lamps, {}",
                    path.display(),
                    read.boxes.len(),
                    read.props.len(),
                    read.starts.len(),
                    read.lamps.len(),
                    if read.indoors.is_some() { "an indoors" } else { "no indoors" }
                );
                world::level::set(read);
            }
            Err(e) => eprintln!("the level file {} could not be read, the range's own instead: {e:#}", path.display()),
        }
    }
    // The test range stops the character as far off as the game's capsule
    // would. The workbench stands on it with the options' `WORKBENCH`.
    world::range::set_workbench(options_state.in_force.workbench);
    world::range::set_armor_bench(options_state.in_force.armor_bench);
    let mut range = world::range::Range::default();
    // `--export-level`: the range as it stands, written, and the run ends.
    if let Some(path) = &options.export_level {
        world::level::write(&range.layout(), path)?;
        println!("the range written to {}", path.display());
        return Ok(None);
    }
    match trp_data::character::player::CapsuleData::load(&mut unity) {
        Ok(capsule) => {
            // The player's controller is the game's own, which keeps no skin
            // outside the capsule (movement spec, "The character's own
            // collision"): recorded, a character stops the radius from a wall.
            range.radius = capsule.radius;
        }
        Err(e) => eprintln!("the character's capsule could not be read: {e:#}"),
    }
    // The doors, containers and trunk turn by the install's curve and wait
    // its delay.
    match trp_sim::held::things::ThingSettings::new(&hard) {
        Ok(settings) => range.thing_settings = Some(settings),
        Err(e) => eprintln!("the doors and containers will not move: {e:#}"),
    }
    // The right-hand door is locked, its key in the crate: to be unlocked
    // with it, or kicked (which opens a door only from the side it swings
    // away from: here, from the yard).
    range.lock_door(1, DOOR_KEY);
    // A capture can start somewhere else in the range than its middle.
    if let Some(name) = &options.start {
        match range.place(name) {
            Some(at) => movement.position = at,
            None => eprintln!("ignoring unknown place {name}: platform, shelf, stairs, doors, yard, car, wooden-crate, supply-crate, scav, drawer, table, high-shelf or floor"),
        }
        if let (false, Some([yaw, pitch])) = (options.view_given, range.start_view(name)) {
            movement.pitch = pitch;
            (movement.yaw, movement.body_yaw) = (yaw, yaw);
            movement.note_rotation();
        }
    }
    // What `--lay` asked for, on the floor 0.8 m ahead of the start (or
    // `--lay-ahead`'s), each 0.4 m right of the one before.
    for (index, (_, at)) in ground[laid_from..].iter_mut().enumerate() {
        *at = screens::inventory_view::beside(movement.position, movement.yaw, [options.lay_ahead, 0.4 * index as f32]);
    }

    // The camera is kept out of the world by a box of its own.
    let camera_box = match trp_data::character::player::CameraBoxData::load(&mut unity) {
        Ok(shape) => Some(shape),
        Err(e) => {
            eprintln!("the camera's collider could not be read: {e:#}");
            None
        }
    };
    Ok(Some(RangeSet { range, camera_box }))
}
