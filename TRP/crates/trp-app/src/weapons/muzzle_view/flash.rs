//! Fired muzzles, their flash, shots and light.

use super::*;

/// The rig in hand's muzzles and the shots each has made since `seen` (the
/// shots seen so far, by muzzle, brought up to now): the weapon's own by
/// its shots, its underbarrel launcher's own by the launcher's
/// (`InitiateShot`: a launcher's shot plays its container's effects, the
/// weapon's not; underbarrel-launcher spec, "The grenade").
pub fn fired_muzzles<'a>(rig: Option<&'a crate::weapons::weapon::WeaponRig>, controller: &crate::player::controller::Controller, seen: &mut [u32; 2]) -> Vec<(&'a crate::weapons::weapon::MuzzleParts, u32)> {
    let counts = [controller.shots_fired, controller.launcher_shots_fired];
    let shots: [u32; 2] = std::array::from_fn(|k| counts[k].wrapping_sub(seen[k]));
    *seen = counts;
    let Some(rig) = rig else { return Vec::new() };
    [rig.muzzle.as_ref(), rig.launcher_muzzle.as_ref()].into_iter().zip(shots).filter_map(|(parts, shots)| Some((parts?, shots))).collect()
}

/// The flash's frame: the rig in hand's jets taken in the first time it is
/// seen (their picture read as stored, as the game's gamma-space project
/// reads it, its cells tiled), and each shot since the last frame given its
/// draws and its time.
pub fn muzzle_flash(
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    controller: Res<crate::player::controller::Controller>,
    time: Res<Time>,
    mut views: ResMut<MuzzleViews>,
    mut meshes: ResMut<JetMeshes>,
    mut images: ResMut<Assets<Image>>,
) {
    let views = &mut *views;
    let fired = fired_muzzles(rig.as_deref(), &controller, &mut views.shots_seen);
    // The weapon put away: its trails cleared (`MuzzleEffectsClear`, as its
    // manager is switched off).
    let in_hand: Vec<Entity> = fired.iter().map(|(parts, _)| parts.parent).collect();
    if views.in_hand != in_hand {
        for before in std::mem::take(&mut views.in_hand) {
            if let Some(flash) = views.flashes.get_mut(&before) {
                flash.smokes.iter_mut().for_each(Smoke::clear);
                flash.smoke_clock = 0.0;
                flash.smoke_last.iter_mut().for_each(|last| *last = None);
            }
        }
        views.in_hand = in_hand;
    }
    for (parts, shots) in fired {
        muzzle_shots(views, parts, shots, &time, &mut meshes, &mut images);
    }
}

/// One muzzle's flash taken in the first time it is seen, and its shots
/// since the last frame given their draws and their time.
fn muzzle_shots(views: &mut MuzzleViews, parts: &crate::weapons::weapon::MuzzleParts, shots: u32, time: &Time, meshes: &mut JetMeshes, images: &mut Assets<Image>) {
    if !views.flashes.contains_key(&parts.parent) {
        let m = &parts.data.material;
        // A muzzle with no jets (most suppressed weapons') has no mesh: its
        // gun smoke is all there is to it.
        if !parts.vertices.is_empty() && !parts.indices.is_empty() {
            let Some(picture) = tiled(&m.texture, images) else { return };
            meshes.0.insert(parts.parent, JetMesh { vertices: parts.vertices.clone(), indices: parts.indices.clone(), picture });
        }
        let figures = JetUniform {
            tint: Vec4::from(m.tint),
            cell: Vec4::new(0.0, 0.0, m.inv_window, m.randomize_phase),
            shot: Vec4::new(1.0, 0.0, 0.0, 0.0),
            z_hdr: Vec4::new(m.z_shift, m.hdr, 0.0, 0.0),
            ..default()
        };
        info!("the muzzle flash: {} jets, {} quads, {} gun smokes", parts.data.jets.len(), parts.vertices.len() / 4, parts.data.smokes.len());
        let smoke_pictures = parts.data.smokes.iter().map(|smoke| tiled(&smoke.material.texture, images)).collect();
        let count = parts.data.smokes.len();
        views.flashes.insert(
            parts.parent,
            Flash { data: parts.data.clone(), flash: MuzzleFlash::default(), figures, smokes: vec![Smoke::default(); count], smoke_pictures, smoke_clock: 0.0, smoke_last: vec![None; count], light: MuzzleLight::default(), lamp: None },
        );
    }
    let Some(flash) = views.flashes.get_mut(&parts.parent) else { return };
    let now = time.elapsed_secs();
    let lit = MuzzleLight::lit_by(&flash.data);
    for _ in 0..shots {
        let draws = flash.flash.shot(&flash.data, now, &mut views.rng);
        flash.figures.random_vals = Vec4::from(draws.random_vals);
        flash.figures.rot_mx = Vec4::from(draws.rot_mx);
        flash.figures.cell.x = draws.cell_shift[0];
        flash.figures.cell.y = draws.cell_shift[1];
        for (smoke, data) in flash.smokes.iter_mut().zip(&flash.data.smokes) {
            smoke.shot(data, &mut views.rng);
        }
        // The light's reach last, as `Shot` draws it after the rest.
        if lit {
            flash.light.shot(&mut views.rng);
        }
    }
}

/// The muzzle's light (firing spec, "The muzzle's light"): the rig in
/// hand's made the first time it is lit, beside its fireport (5 cm along the
/// fireport's parent's x, `UpdateLights`), and set each frame as the shot's
/// time gives it (`ManualLateUpdate`: drawn only while above 0.001); every
/// other rig's put out. A weapon whose jets have no chance has none.
pub fn muzzle_light(
    mut commands: Commands,
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    time: Res<Time>,
    mut views: ResMut<MuzzleViews>,
    nodes: Query<(&Transform, &ChildOf), Without<MuzzleLamp>>,
    mut lamps: Query<(&mut PointLight, &mut Transform, &mut Visibility), With<MuzzleLamp>>,
) {
    let views = &mut *views;
    let gain = views.exposure_gain;
    let now = time.elapsed_secs();
    // Each muzzle's beside its own fireport: the launcher's beside its.
    let launcher = rig.as_ref().and_then(|rig| Some((rig.launcher_muzzle.as_ref()?.parent, rig.launcher_fireport?)));
    let weapon_port = rig.as_ref().and_then(|rig| rig.fireport);
    for (&parent, flash) in views.flashes.iter_mut() {
        let fireport = match launcher {
            Some((own, port)) if own == parent => Some(port),
            _ => weapon_port,
        };
        let intensity = if views.in_hand.contains(&parent) && MuzzleLight::lit_by(&flash.data) { MuzzleLight::intensity(&flash.flash, &flash.data, now) } else { 0.0 };
        if intensity <= 0.0 {
            if let Some((_, _, mut visibility)) = flash.lamp.and_then(|lamp| lamps.get_mut(lamp).ok()) {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
            continue;
        }
        let Some((port, port_parent)) = fireport.and_then(|port| nodes.get(port).ok()) else { continue };
        let at = port.translation + convert::position([trp_sim::weapons::muzzle::LIGHT_SHIFT, 0.0, 0.0]);
        let (color, power) = crate::rendering::lighting::engine_light(trp_sim::weapons::muzzle::LIGHT_COLOR, intensity, flash.light.range, gain);
        // The engine's light made with its defaults: a point light, no
        // shadows (`MuzzleLight.Start`).
        let lamp = PointLight { color, intensity: power, range: flash.light.range, radius: 0.0, shadows_enabled: false, ..default() };
        match flash.lamp.and_then(|entity| lamps.get_mut(entity).ok()) {
            Some((mut light, mut transform, mut visibility)) => {
                *light = lamp;
                transform.translation = at;
                *visibility = Visibility::Inherited;
            }
            None => {
                let entity = commands.spawn((lamp, Transform::from_translation(at), Visibility::Inherited, MuzzleLamp, Name::new("Muzzle Light"), ChildOf(port_parent.parent()))).id();
                flash.lamp = Some(entity);
            }
        }
    }
}

/// The near plane a camera's effects take.
pub(crate) fn near_of(projection: &Projection) -> f32 {
    match projection {
        Projection::Perspective(p) => p.near,
        _ => 0.03,
    }
}
