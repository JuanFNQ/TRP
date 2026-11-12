//! The weapon's parts glowing with its heat.
//!
//! The weapon's parts glowing with its heat (`docs/spec/malfunctions.md`,
//! "The barrel's glow"; the `OVERHEATING` switch): each part the game's
//! prefab gives a `HotObject` whose material draws the glow
//! (`trp_data::heat`), drawn again over itself with its own material and
//! the glow put into what it emits (`heat_glow.wgsl`), its heat's term
//! `((T - 30) * 0.1)^2` from the weapon's heat as its logic has it
//! (`T = 10 * heat / 220 + 30`, `HotObject.ConvertHeat2Celsio`: so
//! `(heat / 220)^2`), given anew only when the heat has moved by a
//! hundredth (`WeaponPrefab.ManualUpdate`). A part with `VisibleHeatAlpha`
//! nought (a handguard, a gas block) only warms in thermal vision: no glow
//! is drawn for it.
//!
//! Each weapon's model takes its heat from its owner (`HeatOwner`, the
//! game's `WeaponPrefab`, every one enabled registered in
//! `WeaponOverHeatSystem`), each with its own last heat given: the rig in
//! hand its logic's; a weapon on the ground (`loose`) or slung on the body
//! (`slung_view`) its item's, cooled from its last shot by the raid's clock
//! as the logic in hand cools it (`WeaponPrefab.CurrentOverheat`); any
//! other rig none.

use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::light::NotShadowCaster;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use trp_sim::items::inventory::Id;
use trp_sim::weapons::malfunction::{self, Settings, WeaponFigures};

const HEAT_SHADER: Handle<Shader> = uuid_handle!("6d2f8e31-4b7a-4c95-9e08-1a3c5b7d9f24");

/// The globals' `HeatVisibleFactor`, which the install's `Overheat` leaves
/// out: the class's own 1 (`GlobalConfiguration`).
const HEAT_VISIBLE_FACTOR: f32 = 1.0;

/// How far the heat moves before the parts are given it again
/// (`WeaponPrefab.ManualUpdate`: not below 0.01).
const HEAT_STEP: f32 = 0.01;

/// `Heat` in `heat_glow.wgsl`.
#[derive(ShaderType, Debug, Clone, Copy, Default, Reflect)]
pub struct HeatUniform {
    pub centre_term: Vec4,
    pub size_visible: Vec4,
    pub color_1: Vec4,
    pub color_2: Vec4,
    pub spec: Vec4,
}

/// The glow laid on a part's own material.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct HeatGlow {
    #[uniform(100)]
    pub heat: HeatUniform,
}

impl MaterialExtension for HeatGlow {
    fn fragment_shader() -> ShaderRef {
        HEAT_SHADER.into()
    }
}

/// A part's own material with its glow.
pub type HeatMaterial = ExtendedMaterial<StandardMaterial, HeatGlow>;

/// A weapon's model whose parts glow and shimmer with its heat (the game's
/// `WeaponPrefab`): where its heat comes from, and the heat it was last
/// given (`_previousOverheat`, which begins below any, so that its first
/// frame gives it). On a rig's `Weapon_root`, or on a model's root (its
/// first object, the prefab's own).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct HeatOwner {
    /// The item whose heat it shows (a weapon on the ground, or slung);
    /// none for a rig of the hands, which shows the weapon in hand's
    /// logic's while it is the rig in hand, else none.
    item: Option<Id>,
    /// That item's figures, read once.
    figures: Option<WeaponFigures>,
    /// Its parts shimmer only while it is shown: one's own slung weapons,
    /// whose renderers the game turns off in first person but whose
    /// shimmer its code would still emit there (read from code, not seen:
    /// not built).
    pub(crate) seen_only: bool,
    given: f32,
    /// Its item as last found among what is carried and lies about
    /// (`give_heat`): the carried things' change number then, and the
    /// item's last shot's heat and time (none: the item is gone). The item
    /// is looked for again only when that number has moved; its heat now is
    /// worked out from this by the clock on every frame.
    found: Option<(u32, Option<malfunction::State>)>,
}

impl HeatOwner {
    /// A rig of the hands.
    pub fn hands() -> Self {
        Self { item: None, figures: None, seen_only: false, given: -1.0, found: None }
    }

    /// A model of an item not in hand.
    pub fn item(id: Id, seen_only: bool) -> Self {
        Self { item: Some(id), figures: None, seen_only, given: -1.0, found: None }
    }

    /// The heat last given.
    pub fn given(&self) -> f32 {
        self.given
    }

    /// Whether its item is to be looked for: never yet, the carried things
    /// have changed since it was (`stamp`, their change number: an item's
    /// last shot is written to it, and a thing is taken up, put down or
    /// gone, only with a change of theirs), or it was found before its
    /// figures could be read (`readable`: the database is there now).
    fn looks_again(&self, stamp: u32, readable: bool) -> bool {
        match self.found {
            Some((at, state)) => at != stamp || (state.is_some() && self.figures.is_none() && readable),
            None => true,
        }
    }

    /// Its item's last shot as last found; none where the item is gone or
    /// was never looked for.
    fn last_shot(&self) -> Option<malfunction::State> {
        self.found.and_then(|(_, state)| state)
    }
}

/// An item not in hand by its number, wherever a weapon's model shows one:
/// on the ground, on the character, or hung on a dummy.
fn item_of(carried: &crate::screens::inventory_view::Carried, id: Id) -> Option<&trp_sim::items::inventory::Item> {
    carried.ground.iter().map(|(item, _)| item).find(|item| item.id == id).or_else(|| carried.equipment.item.find(id)).or_else(|| carried.find_on_bodies(id))
}

/// The heat given after a frame's: the new one once it is a hundredth or
/// more from the last given, else the last (`WeaponPrefab.ManualUpdate`).
pub fn stepped(given: f32, heat: f32) -> f32 {
    if (heat - given).abs() < HEAT_STEP {
        given
    } else {
        heat
    }
}

/// The glow's term of the heat given: `((T - 30) * 0.1)^2` with `T = 10 *
/// heat / 220 + 30`, so `(heat / 220)^2`; none below nought (the first
/// frame's -1).
pub fn glow_term(given: f32) -> f32 {
    (given.max(0.0) / 220.0).powi(2)
}

/// The heat a weapon not in hand shows: its item's, from its last shot's
/// heat and time by the raid's clock now, as the logic in hand has it
/// (`WeaponPrefab.CurrentOverheat`, `Weapon.GetCurrentOverheat(GameHelper.
/// PastTime)`); none with `OVERHEATING` off, as the logic's.
pub fn item_heat(state: &malfunction::State, figures: &WeaponFigures, settings: &Settings, now: f64, overheating: bool) -> f32 {
    let figures = WeaponFigures { allow_overheat: figures.allow_overheat && overheating, ..*figures };
    malfunction::current_heat(state, now, &figures, settings)
}

/// A part's glow, on a twin of the part's mesh: the model it is of (its
/// owner, `HeatOwner`), the part's material, and its figures (in Bevy's
/// axes).
#[derive(Component, Clone)]
pub struct HotGlow {
    pub owner: Entity,
    pub material: Handle<StandardMaterial>,
    pub centre: Vec3,
    pub size: Vec3,
    pub visible: f32,
    pub colors: [Vec4; 2],
    pub spec: Vec4,
}

/// A part's shimmer while hot (`HotObject` with `UseHeatHaze`;
/// `particles_view` emits it): the model it is of (its owner,
/// `HeatOwner`), its figures, and its state once woken; the heat it was
/// last given, where it was on the frame it last went by, and that frame's
/// number.
#[derive(Component)]
pub struct HotShimmer {
    pub owner: Entity,
    pub haze: trp_data::weapons::heat::HotHaze,
    pub state: Option<trp_sim::weapons::muzzle::BarrelHaze>,
    pub given: f32,
    pub previous: Option<[f32; 3]>,
    pub synced: u64,
}

pub struct HeatViewPlugin;

impl Plugin for HeatViewPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, HEAT_SHADER, "../shaders/heat_glow.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<HeatMaterial>::default()).add_systems(Update, (dress_glows, follow_layers, give_heat, glow_heat).chain());
    }
}

/// Twins for a model's parts that glow, each a child of the entity that
/// draws its slot's mesh (`drawn`: the node, the slot and the entity), out
/// of sight until the weapon is hot, its heat its owner's (`HeatOwner`, put
/// on `owner` by the caller). A part drawn on bones is left without (none
/// of the install's weapons sampled has one: `PROVISIONAL`). And each part
/// that shimmers given its state (`HotShimmer`).
pub fn spawn_glows(commands: &mut Commands, owner: Entity, hot: &[trp_data::weapons::heat::HotPart], drawn: &[(usize, usize, Entity)], parts: &[Vec<(Handle<Mesh>, Handle<StandardMaterial>)>], skinned: &dyn Fn(usize) -> bool, layer: usize) {
    let vec4 = |c: [f32; 4]| Vec4::from_array(c);
    // The shimmer, on an entity drawn for its node (where its renderer is),
    // for a part that gives its heat to any material.
    for part in hot.iter().filter(|part| part.materials > 0) {
        let Some(haze) = &part.haze else { continue };
        let Some(&(_, _, entity)) = drawn.iter().find(|(node, _, _)| *node == part.node) else { continue };
        commands.entity(entity).insert(HotShimmer { owner, haze: haze.clone(), state: None, given: -1.0, previous: None, synced: 0 });
    }
    for part in hot.iter().filter(|part| part.visible > 0.0 && !skinned(part.node)) {
        for slot in &part.slots {
            let Some(&(_, _, entity)) = drawn.iter().find(|(node, at, _)| *node == part.node && *at == slot.slot) else { continue };
            let Some((mesh, material)) = parts.get(part.node).and_then(|own| own.get(slot.slot)) else { continue };
            let glow = HotGlow {
                owner,
                material: material.clone(),
                centre: crate::platform::convert::position(part.center),
                size: Vec3::from_array(part.size),
                visible: part.visible,
                colors: [vec4(slot.colors[0]), vec4(slot.colors[1])],
                spec: vec4(slot.spec),
            };
            let twin = commands.spawn((Mesh3d(mesh.clone()), Transform::default(), Visibility::Hidden, NotShadowCaster, NoFrustumCulling, RenderLayers::layer(layer), glow)).id();
            commands.entity(entity).add_child(twin);
        }
    }
}

/// Each new twin its own material, once its part's is there: the part's
/// drawn over it (an opaque or cut-out part's laid on whole), with the
/// glow.
fn dress_glows(mut commands: Commands, glows: Query<(Entity, &HotGlow), (Without<MeshMaterial3d<HeatMaterial>>, crate::platform::park::EvenParked)>, standard: Res<Assets<StandardMaterial>>, mut materials: ResMut<Assets<HeatMaterial>>) {
    for (entity, glow) in &glows {
        let Some(own) = standard.get(&glow.material) else { continue };
        let (alpha_mode, cutoff) = match own.alpha_mode {
            AlphaMode::Opaque => (AlphaMode::Blend, 0.0),
            AlphaMode::Mask(cutoff) => (AlphaMode::Blend, cutoff),
            other => (other, 0.0),
        };
        let heat = HeatUniform {
            centre_term: glow.centre.extend(0.0),
            size_visible: glow.size.max(Vec3::splat(1e-6)).extend(glow.visible),
            color_1: glow.colors[0],
            color_2: glow.colors[1],
            spec: Vec4::new(glow.spec.x, glow.spec.y, HEAT_VISIBLE_FACTOR, cutoff),
        };
        let base = StandardMaterial { alpha_mode, ..own.clone() };
        commands.entity(entity).insert(MeshMaterial3d(materials.add(HeatMaterial { base, extension: HeatGlow { heat } })));
    }
}

/// A twin drawn where its part is: its layers kept as the part's.
fn follow_layers(mut twins: Query<(&ChildOf, &mut RenderLayers), (With<HotGlow>, crate::platform::park::EvenParked)>, layers: Query<&RenderLayers, (Without<HotGlow>, crate::platform::park::EvenParked)>) {
    for (parent, mut own) in &mut twins {
        if let Ok(theirs) = layers.get(parent.parent()) {
            if *own != *theirs {
                *own = theirs.clone();
            }
        }
    }
}

/// Each owner given its heat now, once it has moved a hundredth from the
/// last given: the rig in hand's logic's (its `Weapon_root` the
/// `WeaponRig`'s), any other rig's none; a model of an item its item's
/// (found on the ground or on the character; gone, none). The item is
/// looked for when what is carried or lies about has changed
/// (`HeatOwner::looks_again`), not on every frame; its heat is worked out
/// from its last shot by the clock on every frame, so it cools as before.
#[allow(clippy::too_many_arguments)]
fn give_heat(
    rig: Option<Res<crate::weapons::weapon::WeaponRig>>,
    hands: Option<Res<crate::weapons::weapon::HandsRig>>,
    carried: Option<Res<crate::screens::inventory_view::Carried>>,
    controller: Option<Res<crate::player::controller::Controller>>,
    game: Option<Res<crate::GameData>>,
    features: Option<Res<crate::screens::options_view::Features>>,
    mut settings: Local<Option<Settings>>,
    mut owners: Query<(Entity, &mut HeatOwner), crate::platform::park::EvenParked>,
) {
    let in_hand = rig.as_deref().and_then(|rig| rig.weapon_root);
    let held = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).map_or(0.0, trp_sim::Firearm::heat);
    let now = controller.as_deref().map_or(0.0, crate::player::controller::Controller::raid_clock);
    let overheating = features.as_deref().is_none_or(|features| features.wear().overheating);
    for (entity, mut owner) in &mut owners {
        let source = owner.item;
        let heat = match (source, carried.as_ref()) {
            (None, _) if Some(entity) == in_hand => held,
            (None, _) | (Some(_), None) => 0.0,
            (Some(id), Some(carried)) => {
                let stamp = carried.last_changed().get();
                if owner.looks_again(stamp, game.is_some()) {
                    let item = item_of(carried, id);
                    let figures = match (item, game.as_deref(), owner.figures) {
                        (Some(item), Some(game), None) => Some(crate::weapons::wear::figures_of(&game.db, item)),
                        (.., kept) => kept,
                    };
                    (owner.found, owner.figures) = (Some((stamp, item.map(|item| item.malfunction))), figures);
                }
                match (owner.last_shot(), owner.figures, game.as_deref()) {
                    (Some(state), Some(figures), Some(game)) => {
                        let settings = settings.get_or_insert_with(|| crate::weapons::wear::settings_of(&game.db.config));
                        item_heat(&state, &figures, settings, now, overheating)
                    }
                    _ => 0.0,
                }
            }
        };
        let next = stepped(owner.given, heat);
        if next != owner.given {
            owner.given = next;
        }
    }
}

/// Each twin given its owner's heat's term, shown while it is above nought.
fn glow_heat(owners: Query<&HeatOwner, crate::platform::park::EvenParked>, mut glows: Query<(&HotGlow, &MeshMaterial3d<HeatMaterial>, &mut Visibility), crate::platform::park::EvenParked>, mut materials: ResMut<Assets<HeatMaterial>>) {
    for (glow, material, mut shown) in &mut glows {
        let term = owners.get(glow.owner).map_or(0.0, |owner| glow_term(owner.given));
        let on = term > 0.0;
        let wanted = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *shown != wanted {
            *shown = wanted;
        }
        // (Changed only when it moved: a material touched is sent again.)
        let stale = on && materials.get(&material.0).is_some_and(|m| (m.extension.heat.centre_term.w - term).abs() > 1e-9);
        if stale {
            if let Some(material) = materials.get_mut(&material.0) {
                material.extension.heat.centre_term.w = term;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A weapon that overheats, with the class's own figures otherwise.
    fn hot_figures() -> WeaponFigures {
        WeaponFigures { allow_overheat: true, ..WeaponFigures::default() }
    }

    #[test]
    fn a_weapon_not_in_hand_cools_as_its_logic_would() {
        let settings = Settings::default();
        let figures = hot_figures();
        // Fired to 185 at 10 s of the raid, then dropped or slung.
        let state = malfunction::State { last_shot_heat: 185.0, last_shot_time: 10.0, ..malfunction::State::default() };
        assert_eq!(item_heat(&state, &figures, &settings, 10.0, true), 185.0);
        let mut last = 185.0;
        for now in [11.0, 15.0, 30.0, 45.0] {
            let heat = item_heat(&state, &figures, &settings, now, true);
            assert_eq!(heat, malfunction::current_heat(&state, now, &figures, &settings), "at {now} s");
            assert!(heat < last, "at {now} s: {heat} after {last}");
            last = heat;
        }
        // Cold in the end.
        assert_eq!(item_heat(&state, &figures, &settings, 10_000.0, true), 0.0);
        // `OVERHEATING` off, or a weapon that does not overheat: none.
        assert_eq!(item_heat(&state, &figures, &settings, 11.0, false), 0.0);
        assert_eq!(item_heat(&state, &WeaponFigures::default(), &settings, 11.0, true), 0.0);
    }

    /// A rifle fired to 185 at 10 s of the raid, lying on the ground beside
    /// the inventory tests' character.
    fn with_a_hot_rifle_down() -> (crate::screens::inventory_view::Carried, Id) {
        use trp_sim::items::inventory::{Item, Kind};
        let (mut carried, _) = crate::screens::inventory_view::tests::carried();
        let mut rifle = Item::new(std::sync::Arc::new(Kind { template: "rifle".into(), ..Kind::default() }));
        rifle.malfunction = malfunction::State { last_shot_heat: 185.0, last_shot_time: 10.0, ..malfunction::State::default() };
        let id = rifle.id;
        carried.ground.push((rifle, Vec3::ZERO));
        (carried, id)
    }

    #[test]
    fn an_item_is_looked_for_only_when_what_is_carried_has_changed() {
        let (mut carried, id) = with_a_hot_rifle_down();
        let state = |carried: &crate::screens::inventory_view::Carried| item_of(carried, id).map(|item| item.malfunction);
        assert_eq!(state(&carried).map(|state| state.last_shot_heat), Some(185.0));
        let mut owner = HeatOwner::item(id, false);
        assert!(owner.looks_again(7, true), "never looked for");
        (owner.found, owner.figures) = (Some((7, state(&carried))), Some(hot_figures()));
        assert!(!owner.looks_again(7, true), "nothing carried has changed");
        // Not looked for, it still cools: its heat is its last shot's by
        // the clock now.
        let settings = Settings::default();
        let heat = |owner: &HeatOwner, now: f64| item_heat(&owner.last_shot().expect("found"), &hot_figures(), &settings, now, true);
        assert_eq!(heat(&owner, 10.0), 185.0);
        assert!(heat(&owner, 11.0) < 185.0);
        assert!(heat(&owner, 30.0) < heat(&owner, 11.0));
        // Something carried changed (a thing moved, a shot written to its
        // item): looked for again.
        assert!(owner.looks_again(8, true));
        // Taken up and gone from where a model's item is looked for: none.
        carried.ground.clear();
        owner.found = Some((8, state(&carried)));
        assert_eq!(owner.last_shot(), None);
        assert!(!owner.looks_again(8, true), "gone is not looked for again either");
        // Found before the database could be read: again once it can.
        let early = HeatOwner { found: Some((7, Some(malfunction::State::default()))), ..HeatOwner::item(id, false) };
        assert!(!early.looks_again(7, false));
        assert!(early.looks_again(7, true));
    }

    #[test]
    fn the_system_finds_an_item_when_the_carried_things_change() {
        let (carried, id) = with_a_hot_rifle_down();
        let mut world = World::new();
        world.insert_resource(carried);
        let owner = world.spawn(HeatOwner::item(id, false)).id();
        let found = |world: &World| world.get::<HeatOwner>(owner).expect("the owner").found.expect("looked for");
        world.run_system_once(give_heat).expect("it runs");
        let first = found(&world);
        assert_eq!(first.1.map(|state| state.last_shot_heat), Some(185.0));
        // Nothing carried changed: what was found stands.
        world.run_system_once(give_heat).expect("it runs");
        assert_eq!(found(&world), first);
        // Another shot's heat written to the item, as the hands write it.
        world.resource_mut::<crate::screens::inventory_view::Carried>().ground[0].0.malfunction.last_shot_heat = 200.0;
        world.run_system_once(give_heat).expect("it runs");
        let second = found(&world);
        assert_ne!(second.0, first.0, "the carried things' change number has moved");
        assert_eq!(second.1.map(|state| state.last_shot_heat), Some(200.0));
        // Taken up: gone, and its glow with it.
        world.resource_mut::<crate::screens::inventory_view::Carried>().ground.clear();
        world.run_system_once(give_heat).expect("it runs");
        assert_eq!(found(&world).1, None);
        assert_eq!(world.get::<HeatOwner>(owner).expect("the owner").given(), 0.0);
    }

    #[test]
    fn the_heat_is_given_again_only_a_hundredth_on() {
        // The first frame gives it, nought too.
        assert_eq!(stepped(-1.0, 0.0), 0.0);
        assert_eq!(stepped(-1.0, 185.0), 185.0);
        assert_eq!(stepped(185.0, 184.995), 185.0);
        assert_eq!(stepped(185.0, 184.98), 184.98);
        assert_eq!(stepped(0.004, 0.0), 0.004);
    }

    #[test]
    fn the_glow_is_off_at_nought() {
        assert_eq!(glow_term(-1.0), 0.0);
        assert_eq!(glow_term(0.0), 0.0);
        assert_eq!(glow_term(220.0), 1.0);
        assert!((glow_term(185.0) - (185.0f32 / 220.0).powi(2)).abs() < 1e-7);
    }

    #[test]
    fn each_glow_shows_its_own_owner_s_heat() {
        let mut world = World::new();
        let mut materials = Assets::<HeatMaterial>::default();
        let material = |materials: &mut Assets<HeatMaterial>| materials.add(HeatMaterial { base: StandardMaterial::default(), extension: HeatGlow { heat: HeatUniform::default() } });
        let (on_ground, slung, cold_one, gone) = (material(&mut materials), material(&mut materials), material(&mut materials), material(&mut materials));
        world.insert_resource(materials);
        let owner = |given: f32| HeatOwner { given, ..HeatOwner::item(1, false) };
        let hot = world.spawn(owner(185.0)).id();
        let warm = world.spawn(owner(60.0)).id();
        let cold = world.spawn(owner(0.0)).id();
        let nobody = world.spawn_empty().id();
        world.despawn(nobody);
        let glow = |owner: Entity| HotGlow { owner, material: Handle::default(), centre: Vec3::ZERO, size: Vec3::ONE, visible: 1.0, colors: [Vec4::ONE; 2], spec: Vec4::ONE };
        let twins: Vec<Entity> = [(hot, &on_ground), (warm, &slung), (cold, &cold_one), (nobody, &gone)].into_iter().map(|(owner, material)| world.spawn((glow(owner), MeshMaterial3d(material.clone()), Visibility::Hidden)).id()).collect();
        world.run_system_once(glow_heat).expect("it runs");
        let shown: Vec<Visibility> = twins.iter().map(|twin| *world.get::<Visibility>(*twin).expect("a twin")).collect();
        assert_eq!(shown, [Visibility::Inherited, Visibility::Inherited, Visibility::Hidden, Visibility::Hidden]);
        let materials = world.resource::<Assets<HeatMaterial>>();
        let term = |handle: &Handle<HeatMaterial>| materials.get(handle).expect("a material").extension.heat.centre_term.w;
        assert!((term(&on_ground) - glow_term(185.0)).abs() < 1e-7);
        assert!((term(&slung) - glow_term(60.0)).abs() < 1e-7);
        assert_eq!(term(&cold_one), 0.0);
    }
}
