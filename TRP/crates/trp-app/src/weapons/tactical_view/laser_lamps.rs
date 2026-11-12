//! The laser's uniform and screen, the tactical keys and the lamps spawned.

use super::*;

/// Which laser: the weapon's (its device and laser), a head device's (its
/// item and laser) or a weapon's hung on the body (its item, its device and
/// laser).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum LaserKey {
    Weapon(usize, usize),
    Head(Id, usize),
    Slung(Id, usize, usize),
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
pub(super) fn tactical_keys(
    bindings: Res<crate::platform::keys::Bindings>,
    options: Res<crate::Options>,
    mut options_state: ResMut<crate::screens::options_view::OptionsState>,
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
    // A change of the hands begun (`Player.HandsChangingEvent`), which lets
    // Hold mode's lights go as it begins.
    let changing = std::mem::replace(&mut state.changes, held.changes) != held.changes;

    // (Under the edit build screen, as under the inventory, no key is
    // heard.)
    let live = alive && !controller.inventory_open && !controller.edit_build && options.keys_live();
    // The keys, Hold mode's lights let go (as the hands change, at a death,
    // under a screen over the battle, with the window's focus lost) and the
    // head light's coroutine, as the simulation runs them
    // (`trp_sim::tactical::Devices`). The player's tactical sound is played
    // whole, as the switches on the head are (`PROVISIONAL`: the game's is
    // placed 1.2 m over the feet, the Character group, rolloff 5).
    let keys = if live { tac::Keys::of(|name| bindings.fired(name)) } else { tac::Keys::default() };
    let around = tac::Around { held: held.item, let_go: changing || !alive || controller.inventory_open || !focused, live, dt: time.delta_secs() };
    let (firearm, motion) = match hands.as_deref_mut() {
        Some(rig) => (rig.firearm.as_mut(), Some(&mut rig.hands)),
        None => (None, None),
    };
    // What is carried is marked changed only where a device was switched
    // (a let-go, a weapon's request taken, the head light's): reached for
    // on every frame, it was marked so on each, the inventory's page was
    // drawn afresh on each and no box knew the mouse was on it.
    let events = state.devices.frame(mode, keys, around, &mut carried.bypass_change_detection().equipment.item, firearm, motion, &mut controller.movement);
    if events.iter().any(|event| matches!(event, tac::Event::LetGo | tac::Event::HeadTaken { .. } | tac::Event::Weapon { refused_by: None, .. })) {
        carried.set_changed();
    }
    for event in events {
        match event {
            tac::Event::Sound => controller.toggle_sounds.push(trp_data::audio::sound::TACTICAL_SOUND),
            tac::Event::LetGo => info!("Hold mode's lights let go: forced off"),
            tac::Event::ModeFlipped(new) => {
                options_state.set_tactical_input(new, &bindings.controls);
                state.mode_switched = Some(new);
                info!("the tactical key's mode: {}", new.word());
            }
            tac::Event::Weapon { key, states, refused_by: None } => info!("the tactical devices' {key:?}: {states:?}"),
            tac::Event::Weapon { key, refused_by: Some(operation), .. } => info!("the tactical devices' {key:?}: refused by the hands ({operation})"),
            tac::Event::HipPenalty { penalty, combo } => info!("the hip penalty: {penalty:.4} (a combo unit {})", if combo { "on" } else { "off" }),
            tac::Event::HeadTaken { states, gesture } => info!("the head light: {states:?}; its gesture {}", if gesture { "plays" } else { "refused" }),
            tac::Event::HeadRefused(why) => info!("the head light: refused, {why}"),
            // The models switched at the coroutine's 0.8 s.
            tac::Event::HeadSwitched => {
                for device in head_devices(&carried) {
                    state.head_shown.insert(device.id, device.light);
                }
                info!("the head light switched: its sound");
            }
            tac::Event::HeadUnlocked => info!("the head light's lock lifted"),
        }
    }
    // A change of what is worn on the head: its devices shown as they
    // stand, at once and silently (`OnHelmetViewChange`).
    let found: Vec<(Id, String)> = head_devices(&carried).iter().map(|d| (d.id, d.kind.template.clone())).collect();
    if found != state.head_found {
        state.head_shown = head_devices(&carried).iter().map(|d| (d.id, d.light)).collect();
        state.head_found = found;
    }
}

/// The cookie's picture, made once by its name.
pub(super) fn cookie(state: &mut Tactical, light: &DeviceLight, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let texture = light.cookie.as_ref()?;
    state.cookies.entry(texture.name.clone()).or_insert_with(|| crate::effects::particles_view::picture(texture, images)).clone()
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
    let (color, intensity) = crate::rendering::lighting::engine_light(light.color, light.drawn_intensity, light.range, gain);
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

pub(super) fn spawn_lamp(commands: &mut Commands, state: &mut Tactical, light: &DeviceLight, images: &mut Assets<Image>) -> Entity {
    let gain = state.exposure_gain;
    spawn_lamp_for(commands, state, light, images, gain)
}

/// The same for a camera at another exposure (`exposure_gain`'s figure for
/// it).
pub(super) fn spawn_lamp_for(commands: &mut Commands, state: &mut Tactical, light: &DeviceLight, images: &mut Assets<Image>, gain: f32) -> Entity {
    let (spot, point) = lamp_of(light, gain);
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

/// A device's model as its state shows it (`UpdateBeams`): `mode_000` while
/// off, its mode's object while on; `object` finds an object of the
/// device's numbering on the model drawn.
pub(super) fn show_modes(device: &Device, now: Light, object: impl Fn(usize) -> Option<Entity>, set: &mut impl FnMut(Entity, bool)) {
    if let Some(off) = device.off_node.and_then(&object) {
        set(off, !now.on);
    }
    for (mode, &node) in device.mode_nodes.iter().enumerate() {
        if let Some(entity) = object(node) {
            set(entity, now.on && now.mode as usize == mode);
        }
    }
}

/// Whether a device's light is lit as its device stands: its mode's
/// object shown, the light on in its prefab, and, an infrared one, seen
/// only through night vision (none here: `IkLight` gives it nothing).
pub(super) fn lit(light: &DeviceLight, state: Light) -> bool {
    state.on && light.mode == Some(state.mode as usize) && light.enabled && !light.infrared
}
