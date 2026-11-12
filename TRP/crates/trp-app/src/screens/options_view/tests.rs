//! Tests of the options tab and its file.

use super::*;
use trp_sim::base::bindings::{AxisGroup, KeyGroup};

fn source(keys: &[&str], wheel: Option<bool>) -> Source {
    Source { keys: keys.iter().map(|key| code(key).unwrap()).collect(), wheel }
}

#[test]
fn what_is_changed_is_kept_and_read_back_over_the_install_s() {
    let group = |name: &str, press: PressType, keys: &[&str]| KeyGroup { name: name.to_owned(), variants: [source(keys, None), Source::default()], press };
    let install = Controls {
        keys: vec![group("ReloadWeapon", PressType::Release, &["R"]), group("Prone", PressType::Press, &["X"]), group("NextWalkPose", PressType::Press, &["C"])],
        axes: vec![AxisGroup { name: "MoveX".into(), pairs: [[source(&["D"], None), source(&["A"], None)], Default::default()] }],
        double_click: 0.3,
    };
    let mut changed = install.clone();
    changed.keys[0] = group("ReloadWeapon", PressType::Press, &["T", "LeftAlt"]);
    changed.keys[2].variants[1] = source(&["Z"], Some(false));
    changed.axes[0].pairs[1] = [source(&["RightArrow"], None), source(&["LeftArrow"], None)];
    changed.double_click = 0.45;
    let graphics = Graphics { vsync: false, field_of_view: 68.0, mode: Mode::Borderless, resolution: [2560, 1440], texture_quality: 0, mip_streaming: false, shadow_quality: 3, ssao: trp_sim::lighting::hbao::SsaoMode::HighestQuality, clouds_quality: trp_sim::lighting::clouds::CloudsQuality::High, render_thread: false, game_fps: 90, fps_limit_off: true, msaa: 2, dust_resolution: crate::effects::offscreen_dust::DustResolution::Quarter, renderer: crate::platform::backend::Renderer::Dx12, lod_bias: 3.5 };
    let mouse = Mouse { sensitivity: 0.42, aiming: 0.31, optic: 2.5, invert: [false, true] };
    let path = std::env::temp_dir().join(format!("trp-options-test-{}.json", std::process::id()));
    // Some switches off (WORLD INTERACTIONS among them, which takes
    // LOOTING and BENCHES with it), one off as it comes turned on.
    let mut kept = Switches::default();
    for switch in [Switch::Clothing, Switch::SmoothAnimations, Switch::ItemIcons, Switch::CharacterBody, Switch::TarkovUi, Switch::Stamina, Switch::WorldInteractions] {
        assert!(kept.set(switch, false));
    }
    assert!(kept.set(Switch::TarkovLighting, true));
    use crate::panels::battle_view::{BattleModes, Shows};
    let game = BattleModes { quick: Shows::Never, stamina: Shows::Always, health: Shows::Autohide, green_to_red: true, tactical: trp_sim::weapons::tactical::InputMode::Hold };
    use crate::audio::ui_sound::Levels;
    let sound = Levels { overall: 7, interface: 4 };
    // Skills, a fraction of a level among them (what a host's level up
    // left toward the next).
    use trp_sim::condition::skills::{Skill, SkillLevels};
    let mut skills = SkillLevels::default();
    skills.set_progress(Skill::CovertMovement, 4000.5);
    skills.set_level(Skill::Strength, 51);
    // Two families begun, in that order, one with points into a level.
    let mut mastering = trp_sim::condition::mastering::MasteringLevels::default();
    mastering.set_points("M4", 1700.5);
    mastering.begin("AKM");
    // Three of the HUD page's notices off, and the unused tabs hidden.
    let mut notices = Notices::default();
    notices.set(Notices::SEARCHING, false);
    notices.set(Notices::EXAMINING, false);
    notices.set(Notices::MALFUNCTIONS, false);
    save(&path, &changed, &graphics, &mouse, &kept, (&game, &notices, true, 12_345), &sound, &skills, &mastering).unwrap();
    // Read over an install that has since gained a binding: the new
    // one stays, the rest are as kept.
    let mut read = install.clone();
    read.keys.push(group("Jump", PressType::Release, &["Space"]));
    let mut display = Graphics { vsync: true, field_of_view: 50.0, mode: Mode::Windowed, resolution: [1600, 900], texture_quality: 2, mip_streaming: true, shadow_quality: 1, ssao: trp_sim::lighting::hbao::SsaoMode::Off, clouds_quality: trp_sim::lighting::clouds::CloudsQuality::Low, render_thread: true, game_fps: 120, fps_limit_off: false, msaa: 4, dust_resolution: crate::effects::offscreen_dust::DustResolution::Half, renderer: crate::platform::backend::Renderer::Vulkan, lod_bias: 2.0 };
    let mut pointer = Mouse { sensitivity: 0.7, aiming: 0.5, optic: 1.0, invert: [false; 2] };
    // A switch is as recommended until a file says it is not.
    let mut switched = Switches::default();
    assert!(switched.on(Switch::Clothing) && switched.on(Switch::TarkovUi) && !switched.on(Switch::TarkovLighting));
    // The HUD page's settings over the install's.
    let mut hud = BattleModes { quick: Shows::Always, stamina: Shows::Autohide, health: Shows::Never, green_to_red: false, tactical: trp_sim::weapons::tactical::InputMode::Press };
    let mut shown = Notices::default();
    let mut hidden = false;
    let mut experience = 0;
    let mut volumes = Levels { overall: 1, interface: 1 };
    let mut levels = SkillLevels::default();
    let mut families = trp_sim::condition::mastering::MasteringLevels::default();
    assert_eq!(load(&path, &mut read, &mut display, &mut pointer, &mut switched, (&mut hud, &mut shown, &mut hidden, &mut experience), &mut volumes, &mut levels, &mut families), [true, true]);
    assert_eq!(experience, 12_345, "the character's experience is kept");
    assert_eq!(shown, notices, "the notices turned off are kept off");
    assert!(shown.on(Notices::LOOTING) && !shown.on(Notices::SEARCHING) && !shown.on(Notices::EXAMINING) && !shown.on(Notices::MALFUNCTIONS));
    assert!(hidden, "the unused tabs are kept hidden");
    assert_eq!(levels, skills, "the skills are kept, the fraction of a level too");
    assert_eq!(families, mastering, "the families are kept in the order begun, with their points");
    assert_eq!((levels.level(Skill::CovertMovement), levels.level(Skill::Strength), levels.level(Skill::Endurance)), (40, 51, 0));
    assert_eq!(hud, game, "the battle screen's settings are kept");
    assert_eq!(volumes, sound, "the SOUNDS page's volumes are kept");
    assert!(!switched.on(Switch::TarkovUi), "TARKOV UI, on as it comes, is kept off");
    assert!(switched.on(Switch::TarkovLighting), "TARKOV LIGHTING, off as it comes, is kept on");
    assert!(!switched.on(Switch::Looting) && !switched.on(Switch::Benches), "what needs WORLD INTERACTIONS is kept off with it");
    assert_eq!(switched, kept, "every switch is kept");
    // A file kept before there was such a switch leaves it as it is,
    // and the older lists' file is not read.
    let older = path.with_extension("older.json");
    std::fs::write(&older, r#"{ "graphics": { "vsync": true }, "features": { "tarkov_ui": false, "health": false } }"#).unwrap();
    let mut untold = Switches::default();
    let mut install_hud = BattleModes::default();
    let mut install_volumes = Levels { overall: 1, interface: 1 };
    let mut untrained = SkillLevels::default();
    let mut none_begun = trp_sim::condition::mastering::MasteringLevels::default();
    let mut all_shown = Notices::default();
    let mut all_tabs = false;
    let mut new_character = 0;
    assert_eq!(load(&older, &mut read.clone(), &mut display.clone(), &mut pointer.clone(), &mut untold, (&mut install_hud, &mut all_shown, &mut all_tabs, &mut new_character), &mut install_volumes, &mut untrained, &mut none_begun), [false, true]);
    assert_eq!(new_character, 0, "a file without it leaves a new character's");
    assert_eq!(all_shown, Notices::default(), "a file without them shows every notice");
    assert!(!all_tabs, "a file without it shows every tab");
    assert!(none_begun.begun().is_empty(), "a file without them begins no family");
    assert!(untrained.untrained(), "a file without them leaves every skill at 0");
    assert_eq!(install_volumes, Levels { overall: 1, interface: 1 }, "a file without them leaves the install's volumes");
    assert_eq!(untold, Switches::default(), "the older list's switches are not read");
    assert_eq!(install_hud, BattleModes::default(), "a file without them leaves the install's");
    // A file kept by hand with a need off: what needs it is off.
    std::fs::write(&older, r#"{ "switches": { "movement": false, "weight": true } }"#).unwrap();
    let mut by_hand = Switches::default();
    load(&older, &mut read.clone(), &mut display.clone(), &mut pointer.clone(), &mut by_hand, (&mut install_hud, &mut all_shown, &mut all_tabs, &mut new_character), &mut install_volumes, &mut untrained, &mut none_begun);
    assert!(!by_hand.on(Switch::Movement) && !by_hand.on(Switch::Weight));
    // The notices as once kept, under their old name, read.
    std::fs::write(&older, r#"{ "xp_notices": { "looting": false } }"#).unwrap();
    let mut kept_before = Notices::default();
    load(&older, &mut read.clone(), &mut display.clone(), &mut pointer.clone(), &mut Switches::default(), (&mut install_hud, &mut kept_before, &mut all_tabs, &mut new_character), &mut install_volumes, &mut untrained, &mut none_begun);
    assert!(!kept_before.on(Notices::LOOTING) && kept_before.on(Notices::MALFUNCTIONS), "the older name's notices are read");
    let _ = std::fs::remove_file(&older);
    let _ = std::fs::remove_file(&path);
    assert_eq!(read.keys[..3], changed.keys[..]);
    assert_eq!(read.keys[3], group("Jump", PressType::Release, &["Space"]));
    assert_eq!(read.axes, changed.axes);
    assert_eq!(read.double_click, 0.45);
    assert_eq!(display, graphics);
    assert_eq!(pointer, mouse);
    // No file: nothing read, nothing changed.
    assert_eq!(load(&path, &mut read, &mut display, &mut pointer, &mut switched, (&mut hud, &mut shown, &mut hidden, &mut experience), &mut volumes, &mut levels, &mut families), [false, false]);
    assert_eq!(pointer, mouse);
    assert!(!switched.on(Switch::Clothing));
}

#[test]
fn a_switch_needs_what_it_names_and_goes_off_with_it() {
    let mut switches = Switches::default();
    assert!(switches.on(Switch::Looting) && switches.on(Switch::Benches) && switches.on(Switch::Clothing));
    // Off: everything that needs it goes with it; what it needs stays.
    assert!(switches.set(Switch::Inventory, false));
    for gone in [Switch::Looting, Switch::RaidLimits, Switch::Clothing, Switch::CompatibleHighlight, Switch::Benches] {
        assert!(!switches.on(gone), "{}", gone.key());
        assert!(!switches.allowed(gone), "{} is held off", gone.key());
    }
    assert!(switches.on(Switch::WorldInteractions) && switches.on(Switch::Attachments) && switches.on(Switch::Targets));
    // On: refused while a need is off.
    assert!(!switches.set(Switch::Looting, true));
    assert!(!switches.on(Switch::Looting));
    // The need back on brings nothing back by itself.
    assert!(switches.set(Switch::Inventory, true));
    assert!(!switches.on(Switch::Looting) && switches.allowed(Switch::Looting));
    assert!(switches.set(Switch::Looting, true));
    // A switch with two needs wants both.
    assert!(switches.set(Switch::Attachments, false) && !switches.on(Switch::Benches));
    assert!(!switches.set(Switch::Benches, true));
    // Off is always taken.
    assert!(switches.set(Switch::Health, false) && !switches.on(Switch::Health));
    // One that is not built is never taken on, whatever is on, and one a
    // file has on is put off.
    let mut unbuilt = Switches::default();
    assert!(!Switch::LedgeClimbing.built() && !unbuilt.allowed(Switch::LedgeClimbing));
    assert!(!unbuilt.set(Switch::LedgeClimbing, true) && !unbuilt.on(Switch::LedgeClimbing));
    unbuilt.0[Switch::LedgeClimbing as usize] = true;
    unbuilt.settle();
    assert!(!unbuilt.on(Switch::LedgeClimbing));
    // The switches as they come agree with their needs, and every
    // need is a switch with no needs of its own.
    let mut recommended = Switches::default();
    recommended.settle();
    assert_eq!(recommended, Switches::default());
    for switch in Switch::ALL {
        assert!(switch.requires().iter().all(|need| need.requires().is_empty() && *need != switch), "{}", switch.key());
    }
}

#[test]
fn the_switches_are_listed_once_and_set_every_field() {
    // A switch's place is its number in the switches.
    for (at, switch) in Switch::ALL.into_iter().enumerate() {
        assert_eq!(switch as usize, at);
    }
    // On exactly one section of the FEATURES page, on the HUD page's,
    // or kept off the pages, under its own key.
    for switch in Switch::ALL {
        let places = SECTIONS.iter().filter(|(_, list)| list.contains(&switch)).count() + HUD_SWITCHES.iter().filter(|hud| **hud == switch).count() + BACKEND.iter().filter(|kept| **kept == switch).count();
        assert_eq!(places, 1, "{}", switch.key());
    }
    // Off the pages: every character switch but SIMPLE XP GAIN, every
    // weapon, inventory, world and presentation switch, and TARKOV UI.
    assert_eq!(SECTIONS.iter().flat_map(|(_, list)| list.iter()).count() + HUD_SWITCHES.len(), 7);
    // FEATURES' sections in the user's order (THE RANGE drawn after them),
    // the character's name under CHARACTER's switches.
    assert_eq!(SECTIONS.map(|(name, _)| name), ["EXPERIMENTAL", "CHARACTER"]);
    assert_eq!(SECTIONS[CHARACTER_SECTION].0, "CHARACTER");
    let mut keys: Vec<&str> = Switch::ALL.map(Switch::key).to_vec();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), Switch::ALL.len());
    // Every switch on, every field on; every switch off, every field
    // off but the ambient occlusion's, which the GRAPHICS page's SSAO
    // switches with its own OFF.
    assert_eq!(Switches([true; 37]).features(), Features::all_on());
    let none = Switches([false; 37]).features();
    for part in Part::ALL {
        assert_eq!(part.get(&none), part == Part::AmbientOcclusion, "{}", part.key());
    }
    // Each switch alone sets fields of its own: no field is two
    // switches'.
    let mut owner: Vec<Option<Switch>> = vec![None; Part::ALL.len()];
    for switch in Switch::ALL {
        let mut one = Switches([false; 37]);
        one.0[switch as usize] = true;
        let set = one.features();
        for (at, part) in Part::ALL.into_iter().enumerate().filter(|(_, part)| *part != Part::AmbientOcclusion && part.get(&set)) {
            assert!(owner[at].replace(switch).is_none(), "{} is set by two switches", part.key());
        }
    }
    // As the app comes: Tarkov's own on, its departures off.
    let features = Features::default();
    assert!(features.tarkov_ui && features.ballistics && features.workbench && features.smooth_animations && features.ambient_occlusion && features.hide_unfinished);
    assert!(!features.customs_light && !features.raid_limits && !features.compatible_highlight && !features.grounded_feet);
}

#[test]
fn a_group_field_off_holds_its_parts_off_and_keeps_their_own() {
    let mut kept = Features::default();
    kept.sounds = false;
    kept.shot_sounds = false;
    kept.visual_effects = false;
    kept.stamina = false;
    let held = kept.effective();
    // The group's parts are off whatever their own fields say, and
    // each part's own is kept for when the group is on.
    assert!(!held.movement_sounds && !held.hands_sounds && !held.shot_sounds && !held.headgear_hearing, "the sounds' parts");
    assert!(kept.movement_sounds && kept.hands_sounds && !kept.shot_sounds, "their own are kept");
    assert!(!held.muzzle_flash && !held.particles && !held.tracers && !held.spent_cases && !held.magazine_rounds && !held.face_shield_mask && !held.face_shield_cracks, "the effects' seven");
    assert!(!held.stamina_drain && !held.exhaustion, "the stamina's two");
    assert!(!held.systems().stamina && !held.systems().exhaustion);
    assert!(held.health && held.ballistics && held.movement && held.tarkov_ui, "the rest as they are");
    // With every group on, each part is its own field.
    let mut own = Features::default();
    own.shot_sounds = false;
    assert_eq!(own.effective(), own);
    assert!(Part::ALL.iter().all(|part| part.group().is_none_or(|group| group.group().is_none())), "a group is no part of another");
}

/// `OBJECT LOD QUALITY` (`lod_view`): the game's five choices written as
/// its menu writes them, a bias kept by the file read back clamped as
/// the game clamps it; and `DISTANCE DETAIL`, on as the app comes, a
/// field of `CHARACTER BODY` alone.
#[test]
fn the_levels_of_detail_s_option_and_switch() {
    assert_eq!(trp_data::setup::install::LOD_BIAS_CHOICES.map(lod_bias_word), ["2", "2.5", "3", "3.5", "4"]);
    let path = std::env::temp_dir().join(format!("trp-lod-test-{}.json", std::process::id()));
    let mut graphics = Graphics { vsync: true, field_of_view: 50.0, mode: Mode::Windowed, resolution: [1600, 900], texture_quality: 2, mip_streaming: true, shadow_quality: 1, ssao: trp_sim::lighting::hbao::SsaoMode::Off, clouds_quality: trp_sim::lighting::clouds::CloudsQuality::Low, render_thread: true, game_fps: 120, fps_limit_off: false, msaa: 4, dust_resolution: crate::effects::offscreen_dust::DustResolution::Half, renderer: crate::platform::backend::Renderer::Vulkan, lod_bias: 2.0 };
    for (written, read) in [("3", 3.0), ("9", 4.0), ("1", 2.0)] {
        std::fs::write(&path, format!(r#"{{ "graphics": {{ "lod_bias": {written} }} }}"#)).unwrap();
        let (mut keys, mut pointer) = (Controls { keys: Vec::new(), axes: Vec::new(), double_click: 0.3 }, Mouse { sensitivity: 0.7, aiming: 0.5, optic: 1.0, invert: [false; 2] });
        let mut switched = Switches::default();
        let (mut hud, mut shown, mut hidden, mut experience) = (crate::panels::battle_view::BattleModes::default(), Notices::default(), false, 0);
        let mut volumes = crate::audio::ui_sound::Levels { overall: 1, interface: 1 };
        let (mut levels, mut families) = (trp_sim::condition::skills::SkillLevels::default(), trp_sim::condition::mastering::MasteringLevels::default());
        load(&path, &mut keys, &mut graphics, &mut pointer, &mut switched, (&mut hud, &mut shown, &mut hidden, &mut experience), &mut volumes, &mut levels, &mut families);
        assert_eq!(graphics.lod_bias, read, "{written}");
    }
    let _ = std::fs::remove_file(&path);
    assert!(Features::default().distance_detail);
    let mut body = Switches([false; 37]);
    body.0[Switch::CharacterBody as usize] = true;
    assert!(body.features().distance_detail);
    assert!(!Switches([false; 37]).features().distance_detail);
    assert_eq!(Part::DistanceDetail.key(), "distance_detail");
    // The world's things' own field, WORLD INTERACTIONS'.
    assert!(Features::default().world_distance_detail);
    let mut world = Switches([false; 37]);
    world.0[Switch::WorldInteractions as usize] = true;
    assert!(world.features().world_distance_detail && !world.features().distance_detail);
    assert!(!body.features().world_distance_detail);
    assert_eq!(Part::WorldDistanceDetail.key(), "world_distance_detail");
    // The dummies' posing economy, TARGETS' field, on.
    assert!(Features::default().posing_economy);
    let mut targets = Switches([false; 37]);
    targets.0[Switch::Targets as usize] = true;
    assert!(targets.features().posing_economy);
    assert!(!body.features().posing_economy);
    assert_eq!(Part::PosingEconomy.key(), "posing_economy");
}

#[test]
fn a_bar_s_figure_is_written_as_the_game_writes_it() {
    assert_eq!(Slide::Sensitivity.shown(0.7), "0.700");
    assert_eq!(Slide::DoubleClick.shown(0.3), "0.30");
    assert_eq!(Slide::Fov.shown(74.0), "74");
    assert_eq!(Slide::GameFps.shown(120.0), "120");
    assert_eq!(Slide::Volume(0).shown(10.0), "10  (0 dB)");
    for kind in [Slide::Fov, Slide::GameFps, Slide::Sensitivity, Slide::Aiming, Slide::DoubleClick, Slide::Optic, Slide::Volume(1)] {
        let [low, high] = kind.range();
        assert!(low < high && kind.step() > 0.0, "{kind:?}");
    }
}

#[test]
fn the_bindings_fall_into_their_categories_in_the_game_s_order() {
    let group = |name: &str| KeyGroup { name: name.to_owned(), variants: [source(&["R"], None), Source::default()], press: PressType::Press };
    let controls = Controls {
        keys: vec![group("Jump"), group("WeaponMounting"), group("Shoot"), group("PrimaryWeaponFirst"), group("ToggleGoggles"), group("Nidnod"), group("Slot4"), group("MakeScreenshot")],
        axes: vec![AxisGroup { name: "MoveX".into(), pairs: Default::default() }],
        double_click: 0.3,
    };
    let categories: Vec<usize> = entries(&controls).into_iter().map(|(_, category)| category).collect();
    assert_eq!(categories, [1, 1, 1, 2, 2, 3, 4, 5, 6, 7]);
    assert_eq!(CATEGORIES[MOUSE_CATEGORY], "MOUSE");
    assert_eq!(GROUP_STARTS.len() + 3, CATEGORIES.len());
}


/// The pages in the column's order (no GAME page), SKILLS last.
#[test]
fn the_pages_are_in_the_users_order() {
    assert_eq!(PAGES, ["GRAPHICS", "SOUNDS", "HUD", "CONTROLS", "FEATURES", "SKILLS"]);
    let named = [GRAPHICS_PAGE, SOUNDS_PAGE, HUD_PAGE, CONTROLS_PAGE, FEATURES_PAGE, SKILLS_PAGE].map(|page| PAGES[page]);
    assert_eq!(named, PAGES, "each page's index names it");
}

/// The HUD page's notices: eight, each with its key in the file; the
/// warning of a round the chamber does not take has no switch among them.
#[test]
fn the_notices_have_no_switch_for_the_incompatible_ammo() {
    assert_eq!(Notices::ALL.len(), Notices::KEYS.len());
    assert!(Notices::ALL.iter().all(|(_, name, _)| !name.contains("AMMO")) && !Notices::KEYS.contains(&"incompatible_ammo"));
    let mut places: Vec<usize> = Notices::ALL.map(|(which, ..)| which).to_vec();
    places.sort_unstable();
    assert_eq!(places, (0..Notices::ALL.len()).collect::<Vec<_>>(), "each notice has a place of its own");
}
