//! Tests of the SKILLS tab.

use super::*;

/// .NET's formats of a `Single`, as the buffs' words have them.
#[test]
fn figures_are_written_as_dotnet_writes_them() {
    assert_eq!(dotnet_figure(0.48, "0.#%"), "48%");
    assert_eq!(dotnet_figure(0.006 * 41.0, "0.#%"), "24.6%");
    assert_eq!(dotnet_figure(0.7, "0.#%"), "70%");
    assert_eq!(dotnet_figure(0.0125, "0.##%"), "1.25%");
    assert_eq!(dotnet_figure(24.0, "0.#"), "24");
    assert_eq!(dotnet_figure(1.25, "0.#"), "1.3");
    assert_eq!(dotnet_figure(0.5, "F1"), "0.5");
    assert_eq!(dotnet_figure(2.0, "F1"), "2.0");
    assert_eq!(dotnet_figure(1234.5, "N2"), "1,234.50");
    assert_eq!(fill_hole("by <color=#54c1ff>{0:0.#%}</color>", BuffValue::Figure(0.25)), "by <color=#54c1ff>25%</color>");
    assert_eq!(thousands(1000), "1 000");
    assert_eq!(thousands(3384), "3 384");
    assert_eq!(thousands(12), "12");
}

/// The filter keeps a class's skills (the game's class order: Special
/// has none in the list); the sort by level keeps the game's order
/// among equals.
#[test]
fn the_list_is_filtered_and_sorted_as_the_game_s() {
    let mut levels = SkillLevels::default();
    levels.set_level(Skill::MagDrills, 12);
    levels.set_level(Skill::Strength, 30);
    assert_eq!(listed(&levels, 0, 0).len(), 35, "the live game's page: the list less the eight locked");
    assert!(listed(&levels, 4, 0).iter().all(|skill| skill.class() == Class::Practical && !skill.locked()));
    assert!(listed(&levels, 2, 0).iter().all(|skill| skill.class() == Class::Combat));
    assert!(listed(&levels, 3, 0).is_empty());
    let sorted = listed(&levels, 0, 1);
    assert_eq!(&sorted[..3], &[Skill::Strength, Skill::MagDrills, Skill::Endurance]);
}

/// A level's words: its points into it of what it takes, maxed at elite.
#[test]
fn a_level_s_points_are_of_what_it_takes() {
    assert_eq!(level_exp(0), 10);
    assert_eq!(level_exp(8), 90);
    assert_eq!(level_exp(9), 100);
    let said = Words::default();
    let mut levels = SkillLevels::default();
    levels.set_progress(Skill::Endurance, 345.0);
    let at = level_words(&said, &levels, Skill::Endurance);
    assert_eq!((at.level, at.elite), (3, false));
    assert_eq!(at.said, "Level: 3");
    assert_eq!(at.progress, "<color=#c5c3b2><b>18</b></color>/40");
    assert!((at.fill - 0.45).abs() < 1e-5);
    levels.set_level(Skill::Endurance, 51);
    let at = level_words(&said, &levels, Skill::Endurance);
    assert!(at.elite && at.fill == 1.0 && at.base == 0.0);
    assert_eq!(at.progress, "<color=#c5c3b2>maxed</color>");
}

/// The levelling's rate by points earned, as the install's figures
/// make it; at none (here, always) the fresh one, its arrow up and its
/// line, none at elite.
#[test]
fn a_skill_starts_a_raid_fresh() {
    let config = serde_json::json!({ "SkillFreshEffectiveness": 1.3, "SkillFreshPoints": 1, "SkillPointsBeforeFatigue": 1, "SkillFatiguePerPoint": 0.6, "SkillMinEffectiveness": 0.0001 });
    assert_eq!(effectiveness(&config, 0), Some(1.3));
    assert_eq!(effectiveness(&config, 1), Some(1.0));
    assert!((effectiveness(&config, 3).unwrap() - 0.36).abs() < 1e-6);
    assert_eq!(effectiveness(&serde_json::json!({}), 0), None);
    assert_eq!(arrows_of(1.3, false), (true, false));
    assert_eq!(arrows_of(1.3, true), (false, false));
    let said = Words::default();
    // The game's 129 (`courses10`'s `health-skills`): the single 1.3
    // times 100 at double precision.
    assert_eq!(rate_line(&said, 1.3, false), "\n\n<color=#54C1FFFF>Leveling speed increased: 129%</color>");
    assert_eq!(rate_line(&said, 1.3, true), "");
    assert_eq!(percent_of(0.36), 36);
    assert_eq!(percent_of(1.0), 100);
}

/// The views made one a frame, the first on the frame the list shows
/// (`courses10`'s `inventory-tabs`: 1 on the click's frame, 2 and 3 on
/// the next two, all forty-two frames later; here 35, thirty-four
/// later); a sort chosen meanwhile ends the making, a filter reaches
/// only the views made; the other view shown with a sort chosen has none.
#[test]
fn the_views_are_made_one_a_frame() {
    let levels = SkillLevels::default();
    let mut fill = Fill::default();
    fill.step();
    assert_eq!(fill.listed(&levels, 0, 0), vec![Skill::Endurance]);
    fill.step();
    fill.step();
    assert_eq!(fill.listed(&levels, 0, 0).len(), 3);
    for _ in 0..32 {
        fill.step();
    }
    assert_eq!(fill.listed(&levels, 0, 0).len(), 35);
    assert!(fill.done());
    // A sort mid-way: the making ends.
    let mut fill = Fill::default();
    (0..5).for_each(|_| fill.step());
    fill.chosen(1, 1);
    (0..40).for_each(|_| fill.step());
    assert_eq!(fill.listed(&levels, 0, 1).len(), 5);
    // A filter mid-way: the views made after show whatever their class.
    let mut fill = Fill::default();
    (0..3).for_each(|_| fill.step());
    fill.chosen(0, 2);
    (0..40).for_each(|_| fill.step());
    let shown = fill.listed(&levels, 2, 0);
    assert_eq!(shown.len(), 35 - 3, "the first three (Physical) filtered out, the rest kept");
    assert_eq!(shown[0], Skill::Health);
    // Chosen again once all are made: every view filtered.
    fill.chosen(0, 2);
    assert!(fill.listed(&levels, 2, 0).iter().all(|skill| skill.class() == Class::Combat));
    // The other view shown with a sort chosen: none made.
    let mut fill = Fill::default();
    fill.switched([0, 1]);
    (0..40).for_each(|_| fill.step());
    assert!(fill.listed(&levels, 0, 1).is_empty());
}

/// The skills the app dims: those it has nothing of, all of them with
/// SKILLS off; never one that acts (the look's reach, repairs, wear and
/// malfunctions, the armour's penalty among them).
#[test]
fn a_skill_is_dimmed_where_it_changes_nothing_here() {
    assert!(dimmed_why(Skill::Endurance, true).is_none());
    assert!(dimmed_why(Skill::Endurance, false).is_some());
    assert!(dimmed_why(Skill::Search, true).is_none());
    assert!(dimmed_why(Skill::Attention, true).is_none());
    assert!(dimmed_why(Skill::Crafting, true).is_some());
    assert!(dimmed_why(Skill::Assault, true).is_none());
    for acts in [Skill::Perception, Skill::Intellect, Skill::Charisma, Skill::TroubleShooting, Skill::WeaponTreatment, Skill::LightVests, Skill::HeavyVests] {
        assert!(dimmed_why(acts, true).is_none(), "{acts:?}");
    }
}
