//! Tactical devices in the app (tactical-devices spec): the game's keys for
//! them in either activation mode, put to the weapon's firearm and the
//! head's devices (`trp_sim::tactical`); each device's model shown as its
//! state has it (`TacticalComboVisualController`); its lights as the
//! engine's, placed where the game's are each frame (the weapon's on its
//! model, the head's by the camera: `HeadLightsValidator`); and what the
//! HUD's panel of the activation mode is told (`sight_panels`). The lasers
//! are `laser_view`'s, the light shafts and flares `shaft_view`'s.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::query::QueryItem;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, SpotLightTexture};
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::{Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::transform::TransformSystems;
use trp_data::tactical::{DeviceLaser, Volume};
use trp_data::tactical::{Device, DeviceLight};
use trp_sim::inventory::{Id, Item, Light};
use trp_sim::tactical::{self as tac, InputMode, WeaponKey};

use crate::controller::Controller;
use crate::inventory_view::{Carried, HandsHeld};
use crate::weapon::{HandsRig, WeaponRig};

/// The devices' systems; `on` is the options' `TACTICAL DEVICES` as the run
/// began. Off, the keys do nothing and no light is made, but every device's
/// model still shows its off object, as an item made with its device off.
pub struct TacticalViewPlugin {
    pub on: bool,
    /// The range lit by its own light (`CUSTOMS LIGHT` off): its camera at
    /// Bevy's own exposure.
    pub own_light: bool,
}

/// What a light worked out for the game's light's exposure (EV100 0,
/// `lighting::EXPOSURE_EV100`) is multiplied by for the camera it is drawn
/// by: 1 with Customs' light; with the range's own, whose camera keeps
/// Bevy's own exposure (EV100 9.7), 2 to the difference, so that a lit
/// surface takes what the game's would.
fn exposure_gain(own_light: bool) -> f32 {
    if own_light {
        2f32.powf(bevy::camera::Exposure::EV100_BLENDER - crate::lighting::EXPOSURE_EV100)
    } else {
        1.0
    }
}

impl Plugin for TacticalViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tactical { on: self.on, exposure_gain: exposure_gain(self.own_light), ..default() })
            .add_systems(Update, tactical_keys.after(crate::keys::hear).before(crate::controller::step))
            .add_systems(Update, show_devices.after(tactical_keys))
            .add_systems(PostUpdate, (place_lamps, aim_lasers).chain().after(TransformSystems::Propagate).before(crate::optic_view::squeeze_drawn))
            .add_plugins(ExtractComponentPlugin::<LaserScreen>::default());
    }
}

/// A device's light as the engine draws it, which the frame places.
#[derive(Component)]
struct TacticalLamp;

/// A light of a device as the engine draws it: its entity, and which
/// device's which light it is (the weapon's by the rig's node, the head's
/// by the side of the camera it was found on).
struct Lamp {
    entity: Entity,
    device: usize,
    light: usize,
    /// The weapon's: the light's object on the rig.
    node: Option<Entity>,
    /// The head's: on the camera's right (`HeadLightsValidator.IsOnRight`),
    /// and its turn as last placed (the game moves it along its turn of the
    /// frame before).
    right: bool,
    turned: [f32; 4],
}

/// The devices' state in the app.
#[derive(Resource, Default)]
pub struct Tactical {
    pub on: bool,
    /// The head light's coroutine and lock.
    head: tac::HeadLight,
    /// The head's devices as their models and lights show them: switched
    /// only at the coroutine's 0.8 s (their items' state is written at the
    /// press), at once when what is worn changes.
    head_shown: HashMap<Id, Light>,
    /// The head's devices as last found (item and template), to tell a
    /// change of what is worn.
    head_found: Vec<(Id, String)>,
    /// Each head device's figures, read from its own prefab when first
    /// worn, by template (none where it could not be read).
    head_data: HashMap<String, Option<Arc<Device>>>,
    /// Each head device's side of the camera, as first found.
    head_side: HashMap<Id, bool>,
    /// The thing in hand on the frame before (Hold mode lets the lights of
    /// the weapon put away go).
    held: Option<Id>,
    /// The activation mode switched this frame by T+B, for the HUD's panel.
    pub mode_switched: Option<InputMode>,
    /// The lights of the rig in hand (its root), and the head's.
    weapon_lamps: Option<((Entity, usize), Vec<Lamp>)>,
    head_lamps: Vec<(Id, Lamp)>,
    /// The lights' cookies, by the picture's name.
    cookies: HashMap<String, Option<Handle<Image>>>,
    /// The lights' share for the camera's exposure (`exposure_gain`).
    exposure_gain: f32,
    /// The lasers' pictures and noises, by their names.
    pictures: HashMap<String, Option<Handle<Image>>>,
    /// Each laser's dot's light (`LaserBeam`'s own: "laserBeamLight"), by the
    /// laser: the weapon's by device and laser, the head's by item.
    dot_lights: HashMap<LaserKey, Entity>,
    /// The head's lasers' turns as last placed (as its lights').
    head_laser_turns: HashMap<LaserKey, [f32; 4]>,
}

/// Which laser: the weapon's (its device and laser) or a head device's (its
/// item and laser).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum LaserKey {
    Weapon(usize, usize),
    Head(Id, usize),
}

/// What the lasers' shader is given for one laser (`laser.wgsl`): the
/// camera's figures and the beam's and the dot's.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct LaserUniform {
    pub view_from_world: Mat4,
    pub clip_from_view: Mat4,
    pub camera: Vec4,
    pub origin: Vec4,
    pub axis: Vec4,
    pub beam_color: Vec4,
    pub beam: Vec4,
    pub clock: Vec4,
    pub dot_center: Vec4,
    pub dot_x: Vec4,
    pub dot_y: Vec4,
    pub dot_color: Vec4,
    pub dot: Vec4,
    pub dot_view: Vec4,
}

/// One laser to draw: its figures and its pictures (the beam's soft spot
/// and 3D noise, the dot's picture and noise).
#[derive(Clone)]
pub struct LaserDraw {
    pub uniform: LaserUniform,
    pub main: Handle<Image>,
    pub noise: Handle<Image>,
    pub dot_main: Handle<Image>,
    pub dot_noise: Handle<Image>,
}

/// On a camera while a laser shows: the lasers (`muzzle_view` draws them).
#[derive(Component, Clone, Default)]
pub struct LaserScreen(pub Vec<LaserDraw>);

impl ExtractComponent for LaserScreen {
    type QueryData = &'static LaserScreen;
    type QueryFilter = ();
    type Out = LaserScreen;

    /// Only while a laser shows.
    fn extract_component(lasers: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        (!lasers.0.is_empty()).then(|| lasers.clone())
    }
}

/// What the head's devices are: every device on what is worn on the head.
fn head_devices(carried: &Carried) -> Vec<&Item> {
    tac::head_devices(tac::HEAD_SLOTS.iter().filter_map(|slot| carried.equipment.worn(slot)))
}

/// The game's keys for the devices, before the controller's step (the
/// hands' `ModSwitch` is taken by this frame's animator), and the head
/// light's coroutine moved on.
#[allow(clippy::too_many_arguments)]
fn tactical_keys(
    bindings: Res<crate::keys::Bindings>,
    options: Res<crate::Options>,
    mut options_state: ResMut<crate::options_view::OptionsState>,
    mut controller: ResMut<Controller>,
    mut carried: ResMut<Carried>,
    held: Res<HandsHeld>,
    mut hands: Option<ResMut<HandsRig>>,
    mut state: ResMut<Tactical>,
    time: Res<Time>,
    windows: Query<&Window>,
) {
    let state = &mut *state;
    state.mode_switched = None;
    if !state.on {
        return;
    }
    let controller = &mut *controller;
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    let mode = options_state.game.tactical;
    let focused = windows.iter().any(|window| window.focused);

    // Hold mode's lights go off, forced and with the player's sound, as the
    // hands change, at a death, under a screen over the battle (the
    // inventory) and with the window's focus lost
    // (`GamePlayerOwner.ReleaseTactical`). The player's tactical sound is
    // played whole, as the switches on the head are (`PROVISIONAL`: the
    // game's is placed 1.2 m over the feet, the Character group, rolloff 5).
    if mode == InputMode::Hold && (held.item != state.held || !alive || controller.inventory_open || !focused) {
        if let Some(previous) = state.held {
            let request = carried.equipment.item.find(previous).and_then(|weapon| tac::weapon_request(weapon, WeaponKey::Up));
            if let Some(request) = request {
                tac::apply(&mut carried.equipment.item, &request.states);
                controller.toggle_sounds.push(trp_data::sound::TACTICAL_SOUND);
                info!("Hold mode's lights let go: forced off");
            }
        }
    }
    state.held = held.item;

    // The head light's coroutine: the models and the sound at 0.8 s, the
    // lock lifted 0.6 s later.
    let step = state.head.step(time.delta_secs());
    if step.switched {
        for device in head_devices(&carried) {
            state.head_shown.insert(device.id, device.light);
        }
        controller.toggle_sounds.push(trp_data::sound::TACTICAL_SOUND);
        info!("the head light switched: its sound");
    }
    if step.unlocked {
        info!("the head light's lock lifted");
    }
    // A change of what is worn on the head: its devices shown as they
    // stand, at once and silently (`OnHelmetViewChange`).
    let found: Vec<(Id, String)> = head_devices(&carried).iter().map(|d| (d.id, d.kind.template.clone())).collect();
    if found != state.head_found {
        state.head_shown = head_devices(&carried).iter().map(|d| (d.id, d.light)).collect();
        state.head_found = found;
    }

    let live = alive && !controller.inventory_open && options.keys_live();
    if !live {
        return;
    }
    // T+B: the other activation mode, kept at once.
    if bindings.fired("ToggleTacticalInteraction") {
        let new = mode.flipped();
        options_state.set_tactical_input(new, &bindings.controls);
        state.mode_switched = Some(new);
        info!("the tactical key's mode: {}", new.word());
    }
    // The weapon's keys.
    let mut keys = Vec::new();
    match mode {
        InputMode::Press if bindings.fired("Tactical") => keys.push(WeaponKey::Toggle),
        InputMode::Hold if bindings.fired("Tactical") => keys.push(WeaponKey::Down),
        InputMode::Hold if bindings.fired("ReleaseTactical") => keys.push(WeaponKey::Up),
        _ => {}
    }
    if bindings.fired("NextTacticalDevice") {
        keys.push(WeaponKey::Next);
    }
    if let (Some(id), Some(rig)) = (held.item, hands.as_deref_mut()) {
        for key in keys {
            let Some(request) = carried.equipment.item.find(id).and_then(|weapon| tac::weapon_request(weapon, key)) else { continue };
            let Some(firearm) = rig.firearm.as_mut() else { continue };
            if !request.forced && !firearm.can_change_lights() {
                info!("the tactical devices' {key:?}: refused by the hands ({})", firearm.operation_said());
                continue;
            }
            tac::apply(&mut carried.equipment.item, &request.states);
            if request.animated {
                firearm.mod_switch();
            }
            if request.sound {
                controller.toggle_sounds.push(trp_data::sound::TACTICAL_SOUND);
            }
            info!("the tactical devices' {key:?}: {:?}", request.states.iter().map(|(_, l)| (l.on, l.mode)).collect::<Vec<_>>());
        }
        // The hip penalty as the controller works it out when it is made
        // and as a request is taken (`UpdateHipInaccuracy`).
        let combo = carried.equipment.item.find(id).is_some_and(tac::combo_on);
        if rig.firearm.is_some() && rig.hands.combo_on() != combo {
            rig.hands.update_hip_inaccuracy(combo);
            info!("the hip penalty: {:.4} (a combo unit {})", rig.hands.hip_penalty(), if combo { "on" } else { "off" });
        }
    }
    // The head light's keys (`Player.SwitchHeadLights`): refused while its
    // lock is on, while the hands may not act, or with no device.
    let (toggle, switch) = if bindings.fired("SwitchHeadLight") {
        (false, true)
    } else {
        (bindings.fired("ToggleHeadLight"), false)
    };
    if toggle || switch {
        let found = head_devices(&carried);
        if state.head.locked() {
            info!("the head light: refused, its lock is on");
        } else if !controller.movement.state_suits_hands() {
            info!("the head light: refused, the hands may not act");
        } else if found.is_empty() {
            info!("the head light: refused, nothing on the head carries a light");
        } else {
            let states = tac::head_request(&found, toggle, switch);
            tac::apply(&mut carried.equipment.item, &states);
            state.head.press();
            let plays = controller.movement.set_interaction(trp_sim::Interaction::HelmetRailGear);
            info!("the head light: {:?}; its gesture {}", states.iter().map(|(_, l)| (l.on, l.mode)).collect::<Vec<_>>(), if plays { "plays" } else { "refused" });
        }
    }
}

/// The cookie's picture, made once by its name.
fn cookie(state: &mut Tactical, light: &DeviceLight, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let texture = light.cookie.as_ref()?;
    state.cookies.entry(texture.name.clone()).or_insert_with(|| crate::particles_view::picture(texture, images)).clone()
}

/// A device's light as the engine's: its colour, its intensity and its
/// reach as the game gives them (the game's own shading: times 4π, which
/// Bevy divides by), its cone, its shadows (`PROVISIONAL`: Bevy's own
/// filtering and biases for the engine's soft medium ones), its cookie. A
/// light only the game's shading can draw as the game does (`shading`):
/// otherwise matched a fifth of its reach out, as the effects' lights are,
/// for a camera at the game's light's exposure, times `gain` for one at
/// another (`exposure_gain`).
fn lamp_of(light: &DeviceLight, gain: f32) -> (Option<SpotLight>, Option<PointLight>) {
    let pi = std::f32::consts::PI;
    let (color, intensity) = if crate::shading::on() {
        (Color::linear_rgb(light.color[0], light.color[1], light.color[2]), 4.0 * pi * light.drawn_intensity)
    } else {
        let reach = 0.2 * light.range;
        let at = light.drawn_intensity / (1.0 + 25.0 * 0.2 * 0.2);
        let l = crate::lighting::linear;
        (Color::linear_rgb(l(light.color[0]), l(light.color[1]), l(light.color[2])), 4.0 * pi * pi * crate::lighting::EXPOSURE_SCALE * reach * reach * l(at) * gain)
    };
    let shadows = light.shadows > 0;
    if light.spot {
        let spot = SpotLight {
            color,
            intensity,
            range: light.range,
            radius: 0.0,
            shadows_enabled: shadows,
            shadow_map_near_z: light.shadow_near.max(0.01),
            inner_angle: (light.inner_spot_angle / 2.0).to_radians().min((light.spot_angle / 2.0).to_radians()),
            outer_angle: (light.spot_angle / 2.0).to_radians(),
            ..default()
        };
        (Some(spot), None)
    } else {
        (None, Some(PointLight { color, intensity, range: light.range, radius: 0.0, shadows_enabled: shadows, ..default() }))
    }
}

fn spawn_lamp(commands: &mut Commands, state: &mut Tactical, light: &DeviceLight, images: &mut Assets<Image>) -> Entity {
    let (spot, point) = lamp_of(light, state.exposure_gain);
    let mut entity = commands.spawn((Transform::default(), Visibility::Hidden, NotShadowCaster, TacticalLamp, Name::new("tactical light")));
    if let Some(spot) = spot {
        entity.insert(spot);
        if let Some(image) = cookie(state, light, images) {
            entity.insert(SpotLightTexture { image });
        }
    }
    if let Some(point) = point {
        entity.insert(point);
    }
    entity.id()
}

/// Whether a device's light is lit as its device stands: its mode's
/// object shown, the light on in its prefab, and, an infrared one, seen
/// only through night vision (none here: `IkLight` gives it nothing).
fn lit(light: &DeviceLight, state: Light) -> bool {
    state.on && light.mode == Some(state.mode as usize) && light.enabled && !light.infrared
}

/// Each device's model as its state shows it, at once (`UpdateBeams`):
/// `mode_000` while off, its mode's object while on; and its lights made
/// for the rig in hand and the head, each shown as its device stands.
#[allow(clippy::too_many_arguments)]
fn show_devices(
    mut commands: Commands,
    mut state: ResMut<Tactical>,
    rig: Option<Res<WeaponRig>>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    mut game: ResMut<crate::GameData>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Query<&mut Visibility>,
) {
    let state = &mut *state;
    let mut set = |entity: Entity, visible: bool| {
        if let Ok(mut visibility) = shown.get_mut(entity) {
            let want = if visible { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
    };
    // The weapon in hand: its devices' states, matched to its rig's in the
    // order the game walks both.
    let weapon = held.item.and_then(|id| carried.equipment.item.find(id));
    let states: Vec<Light> = match weapon {
        Some(weapon) if state.on => tac::devices(weapon).iter().map(|d| d.light).collect(),
        _ => Vec::new(),
    };
    if let Some(rig) = rig.as_deref() {

        let states_of = |i: usize| if states.len() == rig.devices.len() { states[i] } else { Light::default() };
        for (i, device) in rig.devices.iter().enumerate() {
            let now = states_of(i);
            if let Some(off) = device.off_node.and_then(|node| rig.bones.get(node).copied()) {
                set(off, !now.on);
            }
            for (mode, &node) in device.mode_nodes.iter().enumerate() {
                if let Some(&entity) = rig.bones.get(node) {
                    set(entity, now.on && now.mode as usize == mode);
                }
            }
        }
        if state.on {
            // The rig in hand's lights, made anew for another rig.
            // A rig is told apart by its root and its devices' list (the
            // first rig's root stays as its build's devices come).
            let key = (rig.root, Arc::as_ptr(&rig.devices) as usize);
            if state.weapon_lamps.as_ref().is_none_or(|(made_for, _)| *made_for != key) {
                if let Some((_, old)) = state.weapon_lamps.take() {
                    for lamp in old {
                        commands.entity(lamp.entity).despawn();
                    }
                }
                let mut lamps = Vec::new();
                for (d, device) in rig.devices.iter().enumerate() {
                    for (l, light) in device.lights.iter().enumerate() {
                        let entity = spawn_lamp(&mut commands, state, light, &mut images);
                        lamps.push(Lamp { entity, device: d, light: l, node: rig.bones.get(light.node).copied(), right: false, turned: [0.0, 0.0, 0.0, 1.0] });
                    }
                }
                state.weapon_lamps = Some((key, lamps));
            }
            if let Some((_, lamps)) = &state.weapon_lamps {
                for lamp in lamps {
                    let light = &rig.devices[lamp.device].lights[lamp.light];
                    set(lamp.entity, lit(light, states_of(lamp.device)));
                }
            }
        }
    }
    if !state.on {
        return;
    }
    // The head's devices: each one's figures read as first worn, its
    // lights made, each shown as the head's shown state has it.
    let found: Vec<(Id, String)> = state.head_found.clone();
    for (id, template) in &found {
        if !state.head_data.contains_key(template) {
            let crate::GameData { unity, db, .. } = &mut *game;
            let read = trp_data::tactical::read_alone(unity, db, template).map_err(|e| warn!("the head device {template} could not be read; it shows no light: {e:#}")).ok().map(Arc::new);
            state.head_data.insert(template.clone(), read);
        }
        if state.head_lamps.iter().any(|(owner, _)| owner == id) {
            continue;
        }
        let Some(Some(device)) = state.head_data.get(template).cloned() else { continue };
        for (l, light) in device.lights.iter().enumerate() {
            let entity = spawn_lamp(&mut commands, state, light, &mut images);
            state.head_lamps.push((*id, Lamp { entity, device: 0, light: l, node: None, right: false, turned: [0.0, 0.0, 0.0, 1.0] }));
        }
    }
    // Those of a device no longer worn go.
    let worn: Vec<Id> = found.iter().map(|(id, _)| *id).collect();
    state.head_lamps.retain(|(id, lamp)| {
        let keep = worn.contains(id);
        if !keep {
            commands.entity(lamp.entity).despawn();
        }
        keep
    });
    for (id, lamp) in &state.head_lamps {
        let template = found.iter().find(|(owner, _)| owner == id).map(|(_, t)| t.as_str()).unwrap_or_default();
        let Some(Some(device)) = state.head_data.get(template) else { continue };
        let now = state.head_shown.get(id).copied().unwrap_or_default();
        set(lamp.entity, lit(&device.lights[lamp.light], now));
    }
}

use trp_sim::tactical::{head_pose, turn};

/// The lights placed for the frame, after the frame's transforms and
/// before the weapon is squeezed for the drawing: the weapon's on their
/// objects (the place as drawn, the turn as Unity keeps it: the product of
/// the turns above it, whatever the scales); the head's by the camera
/// (`HeadLightsValidator.Update`): at the camera, moved 0.2 m to its side,
/// up and ahead along its own axes as turned the frame before, then turned
/// as the camera, 10° down and 3° toward the middle, and by the field of
/// view below 50.
#[allow(clippy::type_complexity)]
fn place_lamps(
    mut state: ResMut<Tactical>,
    nodes: Query<(&Transform, Option<&ChildOf>), Without<TacticalLamp>>,
    globals: Query<&GlobalTransform, Without<TacticalLamp>>,
    cameras: Query<(&GlobalTransform, &Projection), (With<crate::MainCamera>, Without<TacticalLamp>)>,
    parts: Query<(&crate::body_view::HeadPart, &GlobalTransform), Without<TacticalLamp>>,
    carried: Res<Carried>,
    mut lamps: Query<(&mut Transform, &mut GlobalTransform), With<TacticalLamp>>,
) {
    let state = &mut *state;
    if !state.on {
        return;
    }
    let mut put = |entity: Entity, at: Vec3, turn: Quat| {
        if let Ok((mut transform, mut global)) = lamps.get_mut(entity) {
            *transform = Transform { translation: at, rotation: turn, scale: Vec3::ONE };
            *global = GlobalTransform::from(*transform);
        }
    };
    if let Some((_, weapon_lamps)) = &state.weapon_lamps {
        for lamp in weapon_lamps {
            let Some(node) = lamp.node else { continue };
            let Ok(global) = globals.get(node) else { continue };
            put(lamp.entity, global.translation(), kept_turn(node, &nodes));
        }
    }
    let Ok((camera, projection)) = cameras.single() else { return };
    let fov = match projection {
        Projection::Perspective(p) => p.fov.to_degrees(),
        _ => 75.0,
    };
    let camera_at = camera.translation();
    let camera_turn = crate::convert::rotation_back(camera.to_scale_rotation_translation().1);
    let at_unity = [camera_at.x, camera_at.y, -camera_at.z];
    for (id, lamp) in state.head_lamps.iter_mut() {
        // Its side, by where its part sits as first seen (`IsOnRight`:
        // its offset along the camera's right), left where none is drawn.
        let side = *state.head_side.entry(*id).or_insert_with(|| {
            let template = carried.equipment.item.find(*id).map(|item| item.kind.template.clone()).unwrap_or_default();
            let right = Vec3::from(turn(camera_turn, [1.0, 0.0, 0.0]));
            parts
                .iter()
                .filter(|(part, _)| part.path.last().is_some_and(|(_, of)| *of == template))
                .map(|(_, global)| {
                    let p = global.translation();
                    Vec3::new(p.x - camera_at.x, p.y - camera_at.y, -(p.z - camera_at.z))
                })
                .filter(|offset| offset.length() < 0.6)
                .map(|offset| offset.dot(right) > 0.0)
                .next()
                .unwrap_or(false)
        });
        lamp.right = side;
        let (place, turned) = head_pose(at_unity, camera_turn, fov, side, lamp.turned);
        lamp.turned = turned;
        put(lamp.entity, crate::convert::position(place), crate::convert::rotation(lamp.turned));
    }
}


/// A Unity rotation looking along `forward` with `up` above it
/// (`Quaternion.LookRotation`), in the game's axes.
fn look_rotation(forward: [f32; 3], up: [f32; 3]) -> [f32; 4] {
    let z = Vec3::from(forward).normalize_or(Vec3::Z);
    let mut x = Vec3::from(up).cross(z);
    if x.length_squared() < 1e-12 {
        x = Vec3::X.cross(z).cross(z) * -1.0;
        if x.length_squared() < 1e-12 {
            x = Vec3::X;
        }
    }
    let x = x.normalize();
    let y = z.cross(x);
    Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize().to_array()
}

/// `Quaternion.Lerp`: the shorter way, the result made whole again.
fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    let q = Quat::from_array(std::array::from_fn(|i| a[i] * (1.0 - t) + sign * b[i] * t));
    q.normalize().to_array()
}

/// The turn as Unity keeps it of a node of a rig (the product of the turns
/// above it, whatever the scales), in Bevy's axes.
fn kept_turn(node: Entity, nodes: &Query<(&Transform, Option<&ChildOf>), Without<TacticalLamp>>) -> Quat {
    let mut rotation = Quat::IDENTITY;
    let mut next = Some(node);
    while let Some(e) = next {
        let Ok((transform, parent)) = nodes.get(e) else { break };
        rotation = transform.rotation * rotation;
        next = parent.map(ChildOf::parent);
    }
    rotation
}

/// The weapon in hand's devices' states, in the order the game walks them,
/// as its rig has `count` of them (all off where they do not match).
fn weapon_states(carried: &Carried, held: &HandsHeld, count: usize) -> Vec<Light> {
    let states: Vec<Light> = held.item.and_then(|id| carried.equipment.item.find(id)).map(|weapon| tac::devices(weapon).iter().map(|d| d.light).collect()).unwrap_or_default();
    if states.len() == count {
        states
    } else {
        vec![Light::default(); count]
    }
}

/// A laser's flat picture made once by its name.
fn picture_of(state: &mut Tactical, texture: Option<&Arc<trp_data::unity::extract::TextureData>>, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let texture = texture?;
    state.pictures.entry(texture.name.clone()).or_insert_with(|| crate::particles_view::picture(texture, images)).clone()
}

/// The beam's 3D noise made once by its name: one byte a texel, repeating,
/// sampled smoothly.
fn volume_of(state: &mut Tactical, volume: Option<&Arc<Volume>>, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let volume = volume?;
    state
        .pictures
        .entry(format!("{} (3D)", volume.name))
        .or_insert_with(|| {
            let [w, h, d] = volume.size;
            let texels = (w * h * d) as usize;
            if texels == 0 || volume.data.len() < texels {
                warn!("the laser's noise {} is not one byte a texel; its beam is not drawn", volume.name);
                return None;
            }
            let mut image = Image::new(Extent3d { width: w, height: h, depth_or_array_layers: d }, TextureDimension::D3, volume.data[..texels].to_vec(), TextureFormat::R8Unorm, RenderAssetUsages::RENDER_WORLD);
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                address_mode_w: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                mipmap_filter: ImageFilterMode::Linear,
                ..default()
            });
            Some(images.add(image))
        })
        .clone()
}

/// The first thing a laser's ray meets, in the game's axes: what a bullet
/// meets on the range and the dummies (`PROVISIONAL`: the range's colliders
/// for the game's mask, the terrain, the detailed colliders, the hit
/// colliders and the dead: layers 11, 12, 16 and 23).
fn first_hit(range: &crate::range::Range, dummies: Option<&crate::dummy::Dummies>, from: [f32; 3], to: [f32; 3]) -> Option<trp_sim::ballistics::RayHit> {
    let mut hits = Vec::new();
    range.bullet_hits(from, to, &[], &mut hits);
    if let Some(dummies) = dummies {
        dummies.ray(from, to, &mut hits);
    }
    hits.into_iter().min_by(|a, b| a.distance.total_cmp(&b.distance))
}

/// Each laser shown this frame (`LaserBeam.LateUpdate`): one ray from 0.1 m
/// ahead of its object, 100 m along its way; with a hit at d (k = d/100),
/// the beam d + 0.1 long at a brightness of 1 − k, the dot on the surface
/// sized from 0.017 to 0.2 by k, and its small light 0.1 m off the surface
/// toward the normal less the beam, a quarter of the way turned from
/// looking at the dot to the beam's own turn, its brightness 3.1(1 − k), its
/// cone from 15° to 300° by k (the engine stops at 179°); with none, the
/// beam 100 m long at its full brightness, no dot, its light dark. Each
/// camera that draws the effects is given them (`muzzle_view` draws them);
/// an infrared one shows nothing without night vision (`_Factor` 0).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn aim_lasers(
    mut commands: Commands,
    mut state: ResMut<Tactical>,
    rig: Option<Res<WeaponRig>>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    range: Res<crate::range::Range>,
    dummies: Option<Res<crate::dummy::Dummies>>,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
    nodes: Query<(&Transform, Option<&ChildOf>), Without<TacticalLamp>>,
    globals: Query<&GlobalTransform, Without<TacticalLamp>>,
    mut cameras: Query<(Entity, &Camera, &GlobalTransform, &Projection, Option<&mut LaserScreen>), (With<crate::muzzle_view::JetScreen>, Without<TacticalLamp>)>,
    main: Query<(&GlobalTransform, &Projection), (With<crate::MainCamera>, Without<TacticalLamp>)>,
    mut lamps: Query<(&mut Transform, &mut GlobalTransform, &mut SpotLight, &mut Visibility), With<TacticalLamp>>,
) {
    let state = &mut *state;
    if !state.on {
        return;
    }
    // The lasers shown: which, where it starts and its turn (the game's
    // axes), and its figures.
    let mut seen: Vec<(LaserKey, [f32; 3], [f32; 4], DeviceLaser)> = Vec::new();
    if let Some(rig) = rig.as_deref() {
        let states = weapon_states(&carried, &held, rig.devices.len());
        for (d, device) in rig.devices.iter().enumerate() {
            let now = states[d];
            for (l, laser) in device.lasers.iter().enumerate() {
                if !now.on || laser.mode != Some(now.mode as usize) || laser.infrared {
                    continue;
                }
                let Some(&node) = rig.bones.get(laser.node) else { continue };
                let Ok(global) = globals.get(node) else { continue };
                let at = global.translation();
                seen.push((LaserKey::Weapon(d, l), [at.x, at.y, -at.z], crate::convert::rotation_back(kept_turn(node, &nodes)), laser.clone()));
            }
        }
    }
    if let Ok((camera, projection)) = main.single() {
        let fov = match projection {
            Projection::Perspective(p) => p.fov.to_degrees(),
            _ => 75.0,
        };
        let at = camera.translation();
        let camera_turn = crate::convert::rotation_back(camera.to_scale_rotation_translation().1);
        for (id, template) in state.head_found.clone() {
            let Some(Some(device)) = state.head_data.get(&template).cloned() else { continue };
            let now = state.head_shown.get(&id).copied().unwrap_or_default();
            let side = state.head_side.get(&id).copied().unwrap_or(false);
            for (l, laser) in device.lasers.iter().enumerate() {
                if !now.on || laser.mode != Some(now.mode as usize) || laser.infrared {
                    continue;
                }
                let key = LaserKey::Head(id, l);
                let before = state.head_laser_turns.get(&key).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]);
                let (place, turned) = head_pose([at.x, at.y, -at.z], camera_turn, fov, side, before);
                state.head_laser_turns.insert(key, turned);
                seen.push((key, place, turned, laser.clone()));
            }
        }
    }
    let t = time.elapsed_secs();
    let to_bevy = |v: [f32; 3]| Vec3::new(v[0], v[1], -v[2]);
    let mut draws: Vec<(LaserUniform, [Handle<Image>; 4])> = Vec::new();
    let mut lit = Vec::new();
    for (key, start, turned, laser) in &seen {
        let (Some(beam), Some(point)) = (laser.beam.as_ref(), laser.point.as_ref()) else { continue };
        let f = turn(*turned, [0.0, 0.0, 1.0]);
        let from: [f32; 3] = std::array::from_fn(|i| start[i] + f[i] * laser.ray_start);
        let to: [f32; 3] = std::array::from_fn(|i| from[i] + f[i] * laser.max_distance);
        let hit = first_hit(&range, dummies.as_deref(), from, to);
        let k = hit.as_ref().map_or(1.0, |h| (h.distance / laser.max_distance).clamp(0.0, 1.0));
        let (length, intensity) = match &hit {
            Some(h) => (h.distance + laser.ray_start, (1.0 - k) * laser.intensity_factor),
            None => (laser.max_distance, laser.intensity_factor),
        };
        let mut uniform = LaserUniform {
            origin: to_bevy(*start).extend(laser.beam_size),
            axis: to_bevy(f).extend(length),
            beam_color: Vec4::from(beam.color),
            beam: Vec4::new(intensity, beam.factor, beam.noise_scale, t / 20.0),
            clock: Vec4::new((t / 8.0).sin(), 0.0, 0.0, 0.0),
            dot_color: Vec4::from(point.color),
            dot_view: Vec4::new(point.size_view[0], point.size_view[1], 0.0, 0.0),
            ..default()
        };
        if let Some(h) = &hit {
            let n = h.normal;
            let size = laser.point_size[0] + (laser.point_size[1] - laser.point_size[0]) * k;
            let facing = look_rotation(n, [0.0, 1.0, 0.0]);
            uniform.dot_center = to_bevy(h.point).extend(size);
            uniform.dot_x = to_bevy(turn(facing, [1.0, 0.0, 0.0])).extend(0.0);
            uniform.dot_y = to_bevy(turn(facing, [0.0, 1.0, 0.0])).extend(0.0);
            uniform.dot = Vec4::new(intensity, point.factor, point.min_fov, point.max_dist);
            // The dot's light.
            if laser.use_point_light {
                let away = Vec3::from(n) - Vec3::from(f);
                let away = away.normalize_or(Vec3::from(n));
                let q = Vec3::from(h.point) + away * laser.surface_offset;
                let looking = look_rotation((Vec3::from(h.point) - q).to_array(), [0.0, 1.0, 0.0]);
                let way = nlerp(looking, *turned, 0.25);
                let angle = (laser.angle_close_far[0] + (laser.angle_close_far[1] - laser.angle_close_far[0]) * k).min(179.0);
                let light_intensity = (1.0 - k) * laser.light_intensity;
                let color: [f32; 3] = std::array::from_fn(|i| beam.color[i] * beam.factor);
                lit.push((*key, crate::convert::position(q.to_array()), crate::convert::rotation(way), angle, light_intensity, color, laser.light_range, laser.cookie.clone()));
            }
        }
        let pictures = [picture_of(state, beam.main.as_ref(), &mut images), volume_of(state, beam.noise.as_ref(), &mut images), picture_of(state, point.main.as_ref(), &mut images), picture_of(state, point.noise.as_ref(), &mut images)];
        let [Some(a), Some(b), Some(c), Some(d)] = pictures else { continue };
        draws.push((uniform, [a, b, c, d]));
    }

    // The dots' lights: made once each, placed and lit; the others dark.
    let pi = std::f32::consts::PI;
    let mut kept = Vec::new();
    for (key, at, way, angle, intensity, color, reach, cookie_picture) in lit {
        let entity = match state.dot_lights.get(&key) {
            Some(&entity) => entity,
            None => {
                let mut entity = commands.spawn((SpotLight { shadows_enabled: false, radius: 0.0, ..default() }, Transform::default(), Visibility::Hidden, NotShadowCaster, TacticalLamp, Name::new("laser dot light")));
                if let Some(image) = cookie_picture.as_ref().and_then(|texture| picture_of(state, Some(texture), &mut images)) {
                    entity.insert(SpotLightTexture { image });
                }
                let id = entity.id();
                state.dot_lights.insert(key, id);
                continue;
            }
        };
        kept.push(entity);
        if let Ok((mut transform, mut global, mut spot, mut visibility)) = lamps.get_mut(entity) {
            *transform = Transform { translation: at, rotation: way, scale: Vec3::ONE };
            *global = GlobalTransform::from(*transform);
            let (color, intensity) = if crate::shading::on() {
                (Color::linear_rgb(color[0], color[1], color[2]), 4.0 * pi * intensity)
            } else {
                let l = crate::lighting::linear;
                (Color::linear_rgb(l(color[0]), l(color[1]), l(color[2])), 4.0 * pi * pi * crate::lighting::EXPOSURE_SCALE * reach * reach * l(intensity) * state.exposure_gain)
            };
            (spot.color, spot.intensity, spot.range, spot.outer_angle, spot.inner_angle) = (color, intensity, reach, (angle / 2.0).to_radians(), 0.0);
            let want = if intensity > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
    }
    for (_, &entity) in state.dot_lights.iter() {
        if !kept.contains(&entity) {
            if let Ok((.., mut visibility)) = lamps.get_mut(entity) {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
    // Each camera that draws the effects: the lasers by its own eye.
    for (entity, camera, eye, projection, screen) in &mut cameras {
        let list: Vec<LaserDraw> = if camera.is_active {
            let view_from_world = eye.to_matrix().inverse();
            let clip_from_view = projection.get_clip_from_view();
            let fov = match projection {
                Projection::Perspective(p) => p.fov.to_degrees(),
                _ => 75.0,
            };
            let camera_at = eye.translation().extend(fov);
            draws
                .iter()
                .map(|(u, [main, noise, dot_main, dot_noise])| LaserDraw {
                    uniform: LaserUniform { view_from_world, clip_from_view, camera: camera_at, ..*u },
                    main: main.clone(),
                    noise: noise.clone(),
                    dot_main: dot_main.clone(),
                    dot_noise: dot_noise.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        match screen {
            Some(mut screen) => {
                if !(screen.0.is_empty() && list.is_empty()) {
                    screen.0 = list;
                }
            }
            None => {
                commands.entity(entity).insert(LaserScreen(list));
            }
        }
    }
}


