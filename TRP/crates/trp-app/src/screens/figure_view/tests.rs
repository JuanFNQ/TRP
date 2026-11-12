//! Tests of the inventory's figure.

use super::*;
use bevy::ecs::world::CommandQueue;

/// The install and its database, read once for these tests; none where
/// no install is configured (the test then passes with a note).
fn installed() -> Option<&'static (trp_data::Install, trp_data::Database)> {
    static READ: std::sync::OnceLock<Option<(trp_data::Install, trp_data::Database)>> = std::sync::OnceLock::new();
    let read = READ
        .get_or_init(|| {
            let install = trp_data::Install::locate().ok()?;
            let db = trp_data::Database::load(&install).expect("database");
            Some((install, db))
        })
        .as_ref();
    if read.is_none() {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
    }
    read
}

/// With `OVERALL FIGURE` the OVERALL tab's figure is built beside the
/// gear page's on a skeleton of its own (the menu player's bones, in
/// the figure's order), on a layer of its own, sharing no bone with the
/// gear page's, which stays as it was: a piece put on it later is on its
/// layer; and its hands, which its hung weapons leave off, are the
/// character's until its first showing and then as made at each.
#[test]
fn the_overall_figure_has_a_skeleton_of_its_own() {
    let Some((install, _)) = installed() else { return };
    let mut assets = trp_data::unity::Assets::new(install.clone()).expect("manifest");
    let data = trp_sim::characters::figure::FigureData::load(&mut assets, &[]).expect("the menu figure");
    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let piece = {
        let mut commands = Commands::new(&mut queue, &world);
        let paths = data.paths().to_vec();
        let pose: trp_sim::locomotion::body::Pose = vec![([0.0; 3], [0.0, 0.0, 0.0, 1.0]); paths.len()];
        let gear = Builder::new(&mut commands, &paths, &pose);
        let mut overall = Builder::overall(&mut commands, &data);
        let head = overall.bone("Base HumanHead").expect("its head");
        let piece = overall.rigid(&mut commands, head, &Handle::default(), &Handle::default());
        gear.finish(&mut commands, Some((overall, data.clone())));
        piece
    };
    queue.apply(&mut world);
    let mut figure = world.resource_mut::<Figure>();
    let figure = &mut *figure;
    let overall = figure.overall.as_mut().expect("the OVERALL tab's own figure");
    assert_ne!(figure.skeleton.root, overall.skeleton.root);
    assert!(figure.skeleton.bones.iter().all(|bone| !overall.bones.contains(bone)), "no bone of the gear page's figure is the OVERALL figure's");
    assert_eq!(overall.bones.len(), data.paths().len(), "every bone of the menu figure, in its order");
    assert!(overall.ribcage.is_some(), "the ribcage the weapon hangs on");
    assert_eq!((figure.skeleton.layer(), overall.skeleton.layer()), (RenderLayers::layer(LAYER), RenderLayers::layer(OVERALL_LAYER)));
    assert!(overall.running.is_none() && overall.weapon.is_none(), "made at its first showing, not before");
    let id: trp_sim::items::inventory::Id = 7;
    assert_eq!(overall.in_hand(Some(id)), Some(id), "before its first showing, the hands' now");
    (overall.showings, overall.held_item) = (1, None);
    assert_eq!(overall.in_hand(Some(id)), None, "after, as made at the showing");
    assert_eq!(world.get::<RenderLayers>(piece), Some(&RenderLayers::layer(OVERALL_LAYER)), "a piece put on it is on its layer");
}

/// What the figure holds is the item in hand, read by its rig's key: a
/// firearm by its build (kind 0 for a rifle, its chambered round
/// counted, its stock as folded), nothing for the empty hands; and the
/// OVERALL figure's hung weapons leave it off (`slung_view`'s
/// `the_overall_figure_never_hangs_what_it_holds`).
#[test]
fn the_figure_holds_the_item_in_hand() {
    let Some((install, db)) = installed() else { return };
    let mut kit = crate::dummy_kit(install, db, "usec").expect("a kit");
    let akm = kit.in_slot("FirstPrimaryWeapon").expect("its AKM").clone();
    let held = held_now(&kit, Some(akm.id), db).expect("the AKM held");
    assert_eq!((held.key.as_str(), held.prefab.as_deref(), held.kind), (trp_sim::items::inventory::rig_key(&akm).as_str(), None, trp_sim::characters::figure::kind::TWO_HANDED));
    assert_eq!(held.chamber, u32::from(trp_sim::items::inventory::chambered(&akm).is_some()));
    assert!(!held.folded);
    assert_eq!(held_now(&kit, None, db), None, "the empty hands hold nothing");
    assert!(held.model && held.item == akm.id);
    // Its stock folded, where it folds.
    if let Some(part) = kit.find_mut(akm.id).and_then(|gun| trp_sim::items::inventory::folding_part(gun).map(|part| part.id)) {
        kit.find_mut(part).expect("the folding part").folded = true;
        assert!(held_now(&kit, Some(akm.id), db).expect("held").folded);
    }
    // The game's rule for the rest (`CreateWeapon`): a grenade, a knife
    // and the rangefinder by their own prefabs (a weapon's prefab each),
    // told 2, 3 and 0; medicine and food by none, told 0 and holding
    // nothing.
    let alone = |template: &str| trp_sim::items::inventory::Item::new(std::sync::Arc::new(trp_sim::items::inventory::Kind::of(db, template).expect("the item")));
    use trp_sim::characters::figure::kind;
    for (template, what, kind, model) in [
        ("5710c24ad2720bc3458b45a3", "an F-1", kind::THROWN, true),
        ("54491bb74bdc2d09088b4567", "the ER bayonet", kind::KNIFE, true),
        ("61605e13ffa6e502ac5e7eef", "the Ranger 1500 rangefinder", kind::TWO_HANDED, true),
        ("544fb45d4bdc2dee738b4568", "a Salewa", kind::TWO_HANDED, false),
        ("57347d7224597744596b4e72", "a can of beef stew", kind::TWO_HANDED, false),
    ] {
        let item = alone(template);
        let held = held_now(&item, Some(item.id), db).unwrap_or_else(|| panic!("{what} held"));
        assert_eq!((held.kind, held.model, held.prefab.is_some()), (kind, model, model), "{what}");
        assert_eq!(held.chamber, 0, "{what}");
    }
}

/// The game's eight lights are made from the one set read with the
/// OVERALL panel (`OverallPanel::lights`), by the maker the edit build
/// screen's lights are made by: six points and two directional, each on
/// the OVERALL tab's lights' layer alone, the points about the figure
/// where it stands.
#[test]
fn the_eight_lights_light_the_overall_figure_alone() {
    let Some((install, _)) = installed() else { return };
    let panel = trp_data::interface::screen::OverallPanel::load(install, "level44").expect("the OVERALL panel");
    assert_eq!(panel.lights.len(), 8);
    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let made = {
        let mut commands = Commands::new(&mut queue, &world);
        overall_lights(&mut commands, &panel.lights)
    };
    queue.apply(&mut world);
    assert_eq!(made.len(), 8);
    let points: Vec<Vec3> = made.iter().filter(|&&light| world.get::<PointLight>(light).is_some()).map(|&light| world.get::<Transform>(light).expect("its place").translation).collect();
    assert_eq!((points.len(), made.iter().filter(|&&light| world.get::<DirectionalLight>(light).is_some()).count()), (6, 2));
    assert!(made.iter().all(|&light| world.get::<RenderLayers>(light) == Some(&RenderLayers::layer(OVERALL_LIGHTS))), "on the OVERALL tab's lights' layer alone");
    // Within 1.5 m of the figure's middle a metre up (the view's (0, 0, 5)).
    let middle = PLACE + Vec3::Y;
    for point in points {
        assert!(point.distance(middle) < 1.5, "{point} from {middle}");
    }
}
