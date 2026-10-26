//! A thing's figures as the inspect window lists them
//! (`trp_sim::item_attributes`; inspect-window spec), from the install.
//! These pass trivially, with a note, where no install is configured.

use trp_data::inventory::Loadout;
use trp_data::Database;
use trp_sim::inventory::{Item, Kind};
use trp_sim::item_attributes::{figures, Show};

const AKM: &str = "59d6088586f774275f37482f";
/// 7.62x39 PS.
const PS: &str = "5656d7c34bdc2d9d198b4587";

fn db() -> Option<&'static Database> {
    let Ok(_install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    Some(crate::fixture::database().expect("database"))
}

fn stock(db: &Database, template: &str) -> Item {
    let list = Loadout::of_preset(db, template).expect("the stock preset");
    Item::of_list(db, &list).expect("the item").0
}

fn prop(db: &Database, template: &str, name: &str) -> f32 {
    db.items[template].props.get(name).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32
}

/// The AKM: its durability row, its six bars in the game's order, then its
/// compact rows; its ergonomics the template's and its parts' summed.
#[test]
fn a_weapon_s_figures() {
    let Some(db) = db() else { return };
    let rifle = stock(&db, AKM);
    let shown = figures(&rifle, &db, &|_| None);
    let ids: Vec<&str> = shown.rows.iter().map(|row| row.id).collect();
    assert_eq!(ids, ["Ergonomics", "CenterOfImpact", "SightingRange", "RecoilUp", "RecoilBack", "Velocity", "WeaponFireType", "AmmoCaliber", "FireRate", "EffectiveDist"]);
    assert!(shown.durability.is_some_and(|durability| durability.now > 0.0 && durability.now <= durability.peak));
    fn parts(item: &Item, out: &mut Vec<String>) {
        for part in item.slots.iter().flatten() {
            if part.kind.class_names.iter().any(|name| name == "Mod") {
                out.push(part.kind.template.clone());
            }
            parts(part, out);
        }
    }
    let mut mods = Vec::new();
    parts(&rifle, &mut mods);
    let ergonomics = prop(&db, AKM, "Ergonomics") + mods.iter().map(|template| prop(&db, template, "Ergonomics")).sum::<f32>();
    assert_eq!(shown.rows[0].value.as_deref(), Some(format!("{}", ergonomics.clamp(0.0, 100.0)).as_str()));
    assert!(matches!(shown.rows[0].show, Show::Bar { range: [0.0, 100.0], .. }));
    // Recoil up: the template's force and the parts' recoil, rounded.
    let recoil = mods.iter().map(|template| prop(&db, template, "Recoil")).sum::<f32>() / 100.0;
    let up = prop(&db, AKM, "RecoilForceUp");
    assert_eq!(shown.rows[3].value.as_deref(), Some(format!("{}", (up + up * recoil).round() as i64).as_str()));
    assert!(shown.rows[3].less_is_good);
    // Its velocity is the PS round's (the magazine's), with " m/s".
    assert!(shown.rows[5].value.as_deref().is_some_and(|value| value.ends_with(" m/s")), "{:?}", shown.rows[5].value);
}

/// A round: its damage, its penetration as a dropdown, its calibre, its
/// speed, each where it has one.
#[test]
fn a_round_s_figures() {
    let Some(db) = db() else { return };
    let round = Item::new(std::sync::Arc::new(Kind::of(&db, PS).expect("PS")));
    let shown = figures(&round, &db, &|_| None);
    let ids: Vec<&str> = shown.rows.iter().map(|row| row.id).collect();
    assert_eq!(&ids[..4], ["MaxAmmoDamage", "AmmoPenetrationPower", "Caliber", "BulletSpeed"]);
    assert_eq!(shown.rows[0].value.as_deref(), Some(format!("{}", prop(&db, PS, "Damage")).as_str()));
    assert_eq!(shown.rows[1].show, Show::Dropdown);
    assert!(shown.durability.is_none());
}

/// Body armour with plates: ARMOR POINTS first among its compact rows (its
/// armours' durability summed), its zones and its class at the end, a
/// dropdown each where it holds more than one.
#[test]
fn an_armour_s_figures() {
    let Some(db) = db() else { return };
    let armour = db
        .items
        .values()
        .filter(|template| db.is_a(template, "Armor") && db.default_preset(&template.id).is_ok())
        .filter_map(|template| Loadout::of_preset(&db, &template.id).ok().and_then(|list| Item::of_list(&db, &list).ok()).map(|(item, _)| item))
        .find(|item| item.slots.iter().flatten().count() > 1)
        .expect("a body armour with plates");
    let shown = figures(&armour, &db, &|_| None);
    let compact: Vec<&str> = shown.rows.iter().filter(|row| row.show == Show::Compact).map(|row| row.id).collect();
    assert_eq!(compact.first(), Some(&"ArmorPoints"), "{compact:?}");
    let ids: Vec<&str> = shown.rows.iter().map(|row| row.id).collect();
    assert_eq!(ids.last(), Some(&"ArmorClass"), "{ids:?}");
    assert_eq!(ids[ids.len() - 2], "ArmorZone", "{ids:?}");
    assert!(shown.rows.iter().all(|row| !matches!(row.show, Show::Bar { .. })));
}

/// An armoured rig (the Bagariy): ARMOR POINTS, then CONTAINER SIZE (its
/// grids' cells, 25 as the user's capture of the game shows), then its
/// penalties; its zones and class last.
#[test]
fn an_armoured_rig_s_figures() {
    let Some(db) = db() else { return };
    let rig = stock(&db, "628d0618d1ba6e4fa07ce5a4");
    let shown = figures(&rig, &db, &|_| None);
    let ids: Vec<&str> = shown.rows.iter().map(|row| row.id).collect();
    assert_eq!(&ids[..2], ["ArmorPoints", "ContainerSize"], "{ids:?}");
    assert_eq!(shown.rows[1].value.as_deref(), Some("25"));
    assert_eq!(&ids[ids.len() - 2..], ["ArmorZone", "ArmorClass"], "{ids:?}");
}

/// A round's penetration dropdown (`MultiLineInfo(penetrationPower)`): the
/// PS's 35, as the user's capture of the game shows for the SPP's 35: very
/// high through classes 1 to 3, low through 4, very low through 5 and 6.
#[test]
fn a_round_s_penetration_lines() {
    let Some(db) = db() else { return };
    let round = Item::new(std::sync::Arc::new(Kind::of(&db, PS).expect("PS")));
    let shown = figures(&round, &db, &|_| None);
    let penetration = shown.rows.iter().find(|row| row.id == "AmmoPenetrationPower").expect("its row");
    let said: Vec<(Option<u32>, &str)> = penetration.tip.iter().map(|line| (line.class, line.text.as_str())).collect();
    assert_eq!(
        said,
        [
            (Some(1), "UI/ArmorPenetration/VeryHigh"),
            (Some(2), "UI/ArmorPenetration/VeryHigh"),
            (Some(3), "UI/ArmorPenetration/VeryHigh"),
            (Some(4), "UI/ArmorPenetration/Low"),
            (Some(5), "UI/ArmorPenetration/VeryLow"),
            (Some(6), "UI/ArmorPenetration/VeryLow"),
        ]
    );
}

/// Medicine as the game's inspect window lists it (the user's captures of
/// 2026-10-26, `local\GrizzlyInspect.png`, `Calok Inspecty.png`,
/// `Stimulant Inspect.png`, `Stimulant Inspect2.png`): every row's name and
/// value, in its order, in the install's English words.
#[test]
fn medicine_s_figures_are_the_captures() {
    let Some(db) = db() else { return };
    let install = crate::fixture::install().expect("install");
    let texts = trp_data::inventory::item_texts(&install, "en").expect("texts");
    let rows = |template: &str, resource: Option<f32>| -> Vec<(String, String)> {
        let mut item = Item::new(std::sync::Arc::new(Kind::of(db, template).expect("the item")));
        item.resource = resource;
        figures(&item, db, &|key| texts.say(key)).rows.into_iter().map(|row| (row.name, row.value.unwrap_or_default())).collect()
    };
    let said = |list: &[(&str, &str)]| -> Vec<(String, String)> { list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() };
    // The Grizzly, used down to 1,080.
    assert_eq!(
        rows("590c657e86f77412b013051d", Some(1080.0)),
        said(&[("HP RESOURCE", "1080/1800 HP"), ("USE TIME", "5sec"), ("REMOVES CONCUSSION", ""), ("REMOVES FRACTURE", "50 HP"), ("STOPS HEAVY BLEEDINGS", "130 HP"), ("STOPS LIGHT BLEEDINGS", "40 HP"), ("REMOVES RADIATION EXPOSURE", "")])
    );
    // The CALOK-B.
    assert_eq!(rows("5e8488fa988a8701445df1e4", None), said(&[("HP RESOURCE", "3/3 HP"), ("USE TIME", "3sec"), ("STOPS HEAVY BLEEDINGS", "")]));
    // The SJ1: the side effects red, -0.25 written -0.2 (halves to even).
    let (blue, red) = ("<color=#54C1FFFF>", "<color=#C40000FF>");
    let up = |dur: u32, by: &str| format!("Dur. {dur}sec ({blue}{by}</color>)");
    let side = |del: u32, dur: u32, by: &str| format!("Del. {del}sec / Dur. {dur}sec ({red}{by}</color>)");
    assert_eq!(
        rows("5c0e531286f7747fa54205c2", None),
        said(&[
            ("CANNOT BE DISCARDED IN RAID", "> 2"),
            ("USE TIME", "2sec"),
            ("SKILL \"ENDURANCE\"", &up(180, "+20")),
            ("SKILL \"STRENGTH\"", &up(180, "+20")),
            ("SKILL \"STRESS RESISTANCE\"", &up(180, "+20")),
            ("ENERGY RECOVERY", &side(100, 200, "-0.2")),
            ("HYDRATION RECOVERY", &side(100, 200, "-0.3")),
        ])
    );
    // The Propital.
    assert_eq!(
        rows("5c0e530286f7747fa1419862", None),
        said(&[
            ("CANNOT BE DISCARDED IN RAID", "> 2"),
            ("USE TIME", "2sec"),
            ("REMOVES CONCUSSION", "Dur. 240sec"),
            ("REMOVES PAIN", "Dur. 240sec"),
            ("HEALTH REGENERATION", &up(300, "+1")),
            ("SKILL \"METABOLISM\"", &up(300, "+20")),
            ("SKILL \"HEALTH\"", &up(300, "+20")),
            ("SKILL \"VITALITY\"", &up(300, "+20")),
            ("HANDS TREMOR", "Del. 270sec / Dur. 30sec"),
            ("TUNNEL EFFECT", "Del. 270sec / Dur. 30sec"),
        ])
    );
    // A water bottle: its use time and its hydration, coloured by its sign.
    let water = figures(&Item::new(std::sync::Arc::new(Kind::of(db, "5448fee04bdc2dbc018b4567").expect("water"))), db, &|key| texts.say(key));
    let hydration = water.rows.iter().find(|row| row.name == "HYDRATION").expect("its hydration");
    assert_eq!((hydration.value.as_deref(), hydration.colored), (Some("60"), Some(60.0)));
}
