//! The watch on the first-person hands' wrist, showing the computer's own time.
//! After the body animator spec, "The watch on the wrist". The worn hands' entry names its watch: its three hands, hung on
//! the left forearm's last bone where the entry puts them
//! (`PlayerBody.SetWatches`, `PluggableBone.Plug`), turned each frame to
//! the computer's local time (`Watch.Update`), and shown while the hands
//! are (`PlayerBody.UpdatePlayerRenders`: in first person alone). The
//! options' `CHARACTER BODY` (`Features::wrist_watch`): off, nothing of it
//! is read or drawn.

use std::collections::HashMap;

use bevy::prelude::*;
use trp_data::formats::curve::Curve;
use trp_data::unity::anim::path_hash;
use trp_data::character::watch::WatchPlace;

use crate::player::clothing::Clothing;
use crate::assets::render_data::Converted;
use crate::weapons::weapon::{FirstPersonHands, WeaponRig};
use crate::{platform::convert, GameData, Options};

/// The watch: each hands bundle's, the one hung and its clock.
#[derive(Resource)]
pub struct WristWatch {
    /// Each hands bundle's watch (`trp_data::watch::places`).
    places: HashMap<String, WatchPlace>,
    /// The hands bundle the watch hung is for; none before the first.
    hung_for: Option<String>,
    drawn: Option<Drawn>,
    /// The time of day as the engine's own clock began (`Time<Real>`), in
    /// seconds from midnight: the command line's (`--clock`), else the
    /// computer's local time, taken on the first frame it is wanted.
    start: Option<f64>,
    clock: Option<f64>,
}

/// A watch hung: its first object, its hands' (hour, minute, second) and
/// the second hand's tick.
struct Drawn {
    root: Entity,
    hands: [Entity; 3],
    tick: Curve,
}

impl WristWatch {
    /// The install's watches; the clock as the command line sets it.
    pub fn new(install: &trp_data::Install, clock: Option<f64>) -> anyhow::Result<Self> {
        Ok(Self { places: trp_data::character::watch::places(install)?, hung_for: None, drawn: None, start: None, clock })
    }
}

/// A time of day written `HH:MM` or `HH:MM:SS` (the seconds may have a
/// fraction), in seconds from midnight.
pub fn clock_of(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let hours: u32 = parts[0].parse().ok().filter(|h| *h < 24)?;
    let minutes: u32 = parts[1].parse().ok().filter(|m| *m < 60)?;
    let seconds: f64 = parts.get(2).map_or(Some(0.0), |s| s.parse().ok().filter(|s: &f64| (0.0..60.0).contains(s)))?;
    Some(hours as f64 * 3600.0 + minutes as f64 * 60.0 + seconds)
}

/// The computer's local time of day, in seconds from midnight
/// (`GetLocalTime`; the game's `DateTime.Now`).
#[cfg(windows)]
fn local_time_of_day() -> f64 {
    /// `SYSTEMTIME`.
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut time = SystemTime::default();
    // SAFETY: a structure of the layout the call fills.
    unsafe { GetLocalTime(&mut time) };
    time.hour as f64 * 3600.0 + time.minute as f64 * 60.0 + time.second as f64 + time.milliseconds as f64 / 1000.0
}

/// Elsewhere the zone is not read: the time of day is UTC's.
#[cfg(not(windows))]
fn local_time_of_day() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |since| since.as_secs_f64() % 86_400.0)
}

/// Hangs the watch of the hands worn, once the arms' bones are there, and
/// again when a suit puts on other hands: none where the hands' entry
/// names none. The bone is the hands' skeleton's (`PluggableBone.
/// BonePath`), which in first person is the weapon rig's arm, as the arms'
/// own mesh has it (`clothing::joint_of`); every rig shares the first's
/// arm bones, so it stays the same bone for the run.
#[allow(clippy::too_many_arguments)]
pub fn hang_watch(
    mut commands: Commands,
    mut watch: ResMut<WristWatch>,
    weapon: Option<Res<WeaponRig>>,
    clothing: Option<Res<Clothing>>,
    options: Res<Options>,
    mut game: ResMut<GameData>,
    mut converted: ResMut<Converted>,
    (mut meshes, mut images, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>, ResMut<Assets<StandardMaterial>>),
) {
    let Some(weapon) = weapon else { return };
    let hands = clothing.as_ref().and_then(|clothing| clothing.hands.clone()).unwrap_or_else(|| options.arms.clone());
    if watch.hung_for.as_deref() == Some(hands.as_str()) {
        return;
    }
    if let Some(drawn) = watch.drawn.take() {
        commands.entity(drawn.root).try_despawn();
    }
    let place = watch.places.get(&hands).cloned();
    watch.hung_for = Some(hands.clone());
    let Some(place) = place else {
        info!("the hands {hands} have no watch");
        return;
    };
    let model = match trp_data::character::watch::load(&mut game.unity, &place.prefab) {
        Ok(model) => model,
        Err(e) => {
            warn!("the watch of the hands {hands}: {e:#}");
            return;
        }
    };
    // Its path below the ribcage, as the rig names its arms' bones.
    const RIBCAGE: &str = "Base HumanRibcage/";
    let on_rig = model.bone_path.find(RIBCAGE).map_or(model.bone_path.as_str(), |at| &model.bone_path[at + RIBCAGE.len()..]);
    let Some(&bone) = weapon.by_hash.get(&path_hash(on_rig)) else {
        warn!("the watch's bone {} is not on the arms", model.bone_path);
        return;
    };
    let unity = &game.unity;
    let mut entities: Vec<Entity> = Vec::with_capacity(model.hierarchy.nodes.len());
    for (index, node) in model.hierarchy.nodes.iter().enumerate() {
        // Its first object where the entry puts it on the bone, its own
        // scale kept (`PluggableBone.Plug`).
        let transform = match index {
            0 => Transform { translation: convert::position(place.position), rotation: convert::rotation(trp_sim::base::math::quat_euler(place.rotation)), scale: Vec3::from(node.scale) },
            _ => convert::transform(node.translation, node.rotation, node.scale),
        };
        let visibility = if node.active { Visibility::Inherited } else { Visibility::Hidden };
        let entity = commands.spawn((Name::new(node.name.clone()), transform, visibility)).id();
        commands.entity(node.parent.map_or(bone, |parent| entities[parent])).add_child(entity);
        entities.push(entity);
        let Some(renderer) = &node.renderer else { continue };
        let entry = match converted.mesh(unity, renderer.mesh, model.hierarchy.mirrored(index), &mut meshes) {
            Ok(entry) => entry,
            Err(e) => {
                warn!("the watch's {}: {e:#}", node.name);
                continue;
            }
        };
        crate::assets::render_data::unskin_all(&mut meshes, &entry.parts);
        // Its renderers cast their shadows (`m_CastShadows` 1), which the
        // game leaves so; the hands' own skin casts none.
        for (slot, mesh) in entry.parts.iter().enumerate() {
            let material = converted.material(unity, renderer.materials.get(slot).copied().flatten(), &mut images, &mut materials);
            let part = commands.spawn((Name::new("Watch, drawn"), Mesh3d(mesh.clone()), MeshMaterial3d(material), Transform::default())).id();
            commands.entity(entity).add_child(part);
        }
    }
    info!("the watch {} on the hands {hands}, on {}", place.prefab, model.bone_path);
    watch.drawn = Some(Drawn { root: entities[0], hands: model.hands.map(|at| entities[at]), tick: model.tick });
}

/// Shows the watch while the hands are, and turns its hands to the time:
/// the time of day as the engine's clock began, and that clock's whole
/// milliseconds since (the wall's in play, the frames' with `--fixed-dt`).
pub fn turn_watch(
    mut watch: ResMut<WristWatch>,
    real: Res<Time<Real>>,
    hands: Query<&Visibility, With<FirstPersonHands>>,
    mut visibilities: Query<&mut Visibility, Without<FirstPersonHands>>,
    mut transforms: Query<&mut Transform>,
) {
    let elapsed = real.elapsed();
    let watch = &mut *watch;
    let Some(drawn) = &watch.drawn else { return };
    let shown = hands.iter().next().copied().unwrap_or(Visibility::Hidden);
    if let Ok(mut visibility) = visibilities.get_mut(drawn.root) {
        if *visibility != shown {
            *visibility = shown;
        }
    }
    if shown == Visibility::Hidden {
        return;
    }
    let start = *watch.start.get_or_insert_with(|| watch.clock.unwrap_or_else(|| local_time_of_day() - elapsed.as_secs_f64()));
    let seconds = trp_sim::held::watch::time_of_day(start, elapsed.as_millis() as u64);
    for (entity, degrees) in drawn.hands.iter().zip(trp_sim::held::watch::hands(seconds, &drawn.tick)) {
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            transform.rotation = convert::rotation(trp_sim::base::math::quat_euler([0.0, 0.0, degrees]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_is_read_from_the_command_line() {
        assert_eq!(clock_of("10:08"), Some(36_480.0));
        assert_eq!(clock_of("23:59:30.5"), Some(86_370.5));
        assert_eq!(clock_of("24:00"), None);
        assert_eq!(clock_of("10"), None);
        assert_eq!(clock_of("10:60"), None);
    }
}
