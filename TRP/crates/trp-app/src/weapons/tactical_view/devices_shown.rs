//! Devices shown on the weapon, slung, on the figure and on the dummies.

use super::*;

/// Each device's model as its state shows it, at once (`UpdateBeams`):
/// `mode_000` while off, its mode's object while on; and its lights made
/// for the rig in hand and the head, each shown as its device stands.
#[allow(clippy::too_many_arguments)]
pub(super) fn show_devices(
    mut commands: Commands,
    mut state: ResMut<Tactical>,
    rig: Option<Res<WeaponRig>>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    mut game: ResMut<crate::GameData>,
    mut images: ResMut<Assets<Image>>,
    heads: Option<Res<crate::player::body_view::HeadModels>>,
    mut shown: Query<&mut Visibility, crate::platform::park::EvenParked>,
) {
    let state = &mut *state;
    // (The body's models on the head are parked while hidden: their
    // objects are kept as they would be.)
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
            show_modes(device, states_of(i), |node| rig.bones.get(node).copied(), &mut set);
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
            let read = trp_data::weapons::tactical::read_alone(unity, db, template).map_err(|e| warn!("the head device {template} could not be read; it shows no light: {e:#}")).ok().map(Arc::new);
            state.head_data.insert(template.clone(), read);
        }
        if state.head_lamps.iter().any(|(owner, _)| owner == id) {
            continue;
        }
        let Some(Some(device)) = state.head_data.get(template).cloned() else { continue };
        for (l, light) in device.lights.iter().enumerate() {
            let entity = spawn_lamp(&mut commands, state, light, &mut images);
            state.head_lamps.push((*id, Lamp { entity, device: 0, light: l, node: None, right: false, turned: [0.0, 0.0, 0.0, 1.0] }));
            // Where it is seen from outside (`place_lamps`).
            match heads.as_deref().and_then(|heads| heads.device_on_body(template)).and_then(|on_body| on_body.object(light.node)) {
                Some((_, name)) => info!("the head device {template}: its light {l} seen from outside on its object {name}"),
                None => info!("the head device {template}: its light {l} has no object on the body; from outside it goes by the camera"),
            }
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
    // The head's devices' models on the body's model of what is worn, seen
    // from outside (`TacticalComboVisualController` on each model): as
    // their lights show.
    if let Some(heads) = heads.as_deref() {
        for (id, template) in &found {
            let (Some(Some(device)), Some(on_body)) = (state.head_data.get(template), heads.device_on_body(template)) else { continue };
            let now = state.head_shown.get(id).copied().unwrap_or_default();
            show_modes(device, now, |node| on_body.object(node).map(|(entity, _)| entity), &mut set);
        }
    }
}

/// The devices of the weapons hung on the body (`slung_view`): each one's
/// model as its state shows it, as the weapon in hand's; its lights made
/// once for each model hung and shown as its device stands. A weapon put
/// away keeps its devices' state on its item (`upd.Light`: the keys alone
/// change it in Press mode), and its model hung on the body keeps its
/// lights: for its own character the game switches that model's renderers
/// off and nothing else (`PlayerBody.SlotView.CreateAndParent`), so a light
/// left on lights the ground from the body, in first person too.
pub(super) fn show_slung_devices(
    mut commands: Commands,
    mut state: ResMut<Tactical>,
    slung: Option<Res<crate::player::slung_view::Slung>>,
    carried: Res<Carried>,
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
    let mut hung_now = Vec::new();
    for hung in slung.iter().flat_map(|slung| slung.with_devices()) {
        hung_now.push(hung.root);
        let weapon = if state.on { hung.item_in(&carried) } else { None };
        let states = states_of(weapon, hung.devices.len());
        for (device, &now) in hung.devices.iter().zip(&states) {
            show_modes(device, now, |node| hung.objects.get(node).copied(), &mut set);
        }
        if !state.on {
            continue;
        }
        if !state.slung_lamps.iter().any(|(made_for, _)| *made_for == hung.root) {
            let mut lamps = Vec::new();
            for (d, device) in hung.devices.iter().enumerate() {
                for (l, light) in device.lights.iter().enumerate() {
                    let entity = spawn_lamp(&mut commands, state, light, &mut images);
                    lamps.push(Lamp { entity, device: d, light: l, node: hung.objects.get(light.node).copied(), right: false, turned: [0.0, 0.0, 0.0, 1.0] });
                }
            }
            state.slung_lamps.push((hung.root, lamps));
        }
        if let Some((_, lamps)) = state.slung_lamps.iter().find(|(made_for, _)| *made_for == hung.root) {
            for lamp in lamps {
                set(lamp.entity, lit(&hung.devices[lamp.device].lights[lamp.light], states[lamp.device]));
            }
        }
    }
    // A model taken down (its weapon drawn, moved or rebuilt): its lights go.
    state.slung_lamps.retain(|(root, lamps)| {
        let keep = hung_now.contains(root);
        if !keep {
            for lamp in lamps {
                commands.entity(lamp.entity).despawn();
            }
        }
        keep
    });
}

/// The devices of the weapon in the OVERALL tab's own figure's hands
/// (`figure_view`, `OVERALL FIGURE`): its model's as its item's state shows
/// them, as a hung weapon's (the game makes the menu figure's weapon by the
/// same factory, `ObjectsFactory.CreateItem`, which puts a
/// `TacticalComboVisualController` on each device and shows its state:
/// `UpdateBeams`, the item's `LightComponent`); its lights made once for
/// each model made at a showing and lit as its device stands, on the
/// figure's own layers (`figure_view::lamp_layers`: seen by its camera
/// alone), for its camera's exposure (EV100 0, the game's light's: a gain
/// of 1). Off (`TACTICAL DEVICES`), its devices show their off object and
/// light nothing.
pub(super) fn show_figure_devices(
    mut commands: Commands,
    mut state: ResMut<Tactical>,
    figure: Option<Res<crate::screens::figure_view::Figure>>,
    carried: Res<Carried>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Query<&mut Visibility>,
) {
    let state = &mut *state;
    let held = figure.as_deref().and_then(|figure| figure.overall.as_ref()).and_then(|overall| overall.held_weapon()).filter(|(tree, devices, _)| !tree.is_empty() && !devices.is_empty());
    // A model taken down or made anew: its lights go.
    let root = held.map(|(tree, ..)| tree[0]);
    if state.figure_lamps.as_ref().is_some_and(|(made_for, _)| Some(*made_for) != root) {
        for lamp in state.figure_lamps.take().map(|(_, lamps)| lamps).unwrap_or_default() {
            commands.entity(lamp.entity).despawn();
        }
    }
    let Some((tree, devices, item)) = held else { return };
    let mut set = |entity: Entity, visible: bool| {
        if let Ok(mut visibility) = shown.get_mut(entity) {
            let want = if visible { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
    };
    let weapon = if state.on { item.and_then(|id| carried.equipment.item.find(id)) } else { None };
    let states = states_of(weapon, devices.len());
    for (device, &now) in devices.iter().zip(&states) {
        show_modes(device, now, |node| tree.get(node).copied(), &mut set);
    }
    if !state.on {
        return;
    }
    if state.figure_lamps.is_none() {
        let mut lamps = Vec::new();
        for (d, device) in devices.iter().enumerate() {
            for (l, light) in device.lights.iter().enumerate() {
                let entity = spawn_lamp_for(&mut commands, state, light, &mut images, 1.0);
                commands.entity(entity).insert(crate::screens::figure_view::lamp_layers());
                lamps.push(Lamp { entity, device: d, light: l, node: tree.get(light.node).copied(), right: false, turned: [0.0, 0.0, 0.0, 1.0] });
            }
        }
        state.figure_lamps = Some((tree[0], lamps));
    }
    if let Some((_, lamps)) = &state.figure_lamps {
        for lamp in lamps {
            set(lamp.entity, lit(&devices[lamp.device].lights[lamp.light], states[lamp.device]));
        }
    }
}

/// The dummies' weapons' devices (`dummy::DummyLights`, `dummy::build`):
/// each one's model as its state shows it, as the weapon in hand's
/// (`UpdateBeams`), in its first mode while on; its lights made once for
/// each dummy's model and shown as its device stands. A light another
/// carries is drawn at its distance fade's figure times its fade by the
/// camera's distance (`CullingLightObject`), 1 within 45 m: the range's
/// dummies stand within it. Off (`TACTICAL DEVICES`), their devices show
/// their off object and light nothing.
pub(super) fn show_dummy_devices(
    mut commands: Commands,
    mut state: ResMut<Tactical>,
    dummies: Option<Res<crate::world::dummy::Dummies>>,
    source: Option<Res<crate::world::dummy::DummySource>>,
    lights: Option<Res<crate::world::dummy::DummyLights>>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Query<&mut Visibility>,
) {
    let state = &mut *state;
    let (Some(dummies), Some(maker)) = (dummies, source.as_deref().and_then(|source| source.maker.as_ref())) else { return };
    let devices = &maker.weapon.devices;
    if devices.is_empty() {
        return;
    }
    let mut set = |entity: Entity, visible: bool| {
        if let Ok(mut visibility) = shown.get_mut(entity) {
            let want = if visible { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != want {
                *visibility = want;
            }
        }
    };
    let lit_now = Light { on: state.on && lights.is_some_and(|lights| lights.on), mode: 0 };
    for (tree, shown) in dummies.weapon_trees() {
        let Some(&key) = tree.first() else { continue };
        // A dead body's weapon gone lights nothing (`dummy::clear_dummy_weapons`).
        let now = if shown { lit_now } else { Light::default() };
        for device in devices {
            if let Some(&off) = device.off_node.and_then(|node| tree.get(node)) {
                set(off, !now.on);
            }
            for (mode, &node) in device.mode_nodes.iter().enumerate() {
                if let Some(&entity) = tree.get(node) {
                    set(entity, now.on && now.mode as usize == mode);
                }
            }
        }
        if !state.on {
            continue;
        }
        if !state.dummy_lamps.iter().any(|(made_for, _)| *made_for == key) {
            let mut lamps = Vec::new();
            for (d, device) in devices.iter().enumerate() {
                for (l, light) in device.lights.iter().enumerate() {
                    let entity = spawn_lamp(&mut commands, state, light, &mut images);
                    lamps.push(Lamp { entity, device: d, light: l, node: tree.get(light.node).copied(), right: false, turned: [0.0, 0.0, 0.0, 1.0] });
                }
            }
            state.dummy_lamps.push((key, lamps));
        }
        if let Some((_, lamps)) = state.dummy_lamps.iter().find(|(made_for, _)| *made_for == key) {
            for lamp in lamps {
                set(lamp.entity, lit(&devices[lamp.device].lights[lamp.light], now));
            }
        }
    }
}
