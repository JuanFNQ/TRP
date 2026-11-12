//! The skills page and its lines.

use super::*;

/// The SKILLS page: the character's level; each skill of the game's list
/// by its class, its name and its level on a bar (51, elite; what it does
/// is the SKILLS tab's to say, in its tooltip); and the weapon families'
/// mastering under them, each family's level as the game shows it (Level
/// 1 to 3), and its weapons. A level is taken at once; what play earns,
/// or a host game sets, shows here and is kept the same way. The skills
/// the game locks are left out, as the tab leaves them
/// (`skills_view::shown`).
pub(super) fn skills_page(page: &mut ChildSpawnerCommands, look: &Look, state: &OptionsState) {
    use trp_sim::condition::skills::{Class, Skill, ELITE as ELITE_LEVEL};
    if !state.in_force.skills {
        note_row(page, look, "SKILLS is off on the FEATURES page: every skill counts as 0 and every weapon as Level 1 this run.", AMBER);
    } else if !state.switches.on(Switch::Skills) {
        note_row(page, look, "SKILLS is switched off on the FEATURES page: from the next start every skill counts as 0.", AMBER);
    }
    let shut = |at: usize| state.shut(SKILLS_PAGE, at);
    // The character's level, set by its experience: the start of the level chosen.
    let top = state.level_table.top();
    section_head(page, look, 0, shut(0), "CHARACTER", |_| {});
    if !shut(0) {
        let level = state.level_table.level(state.experience);
        let next = state.level_table.experience_at(level + 1);
        let what = if level >= top { format!("{} experience: the top level.", state.experience) } else { format!("{} experience; level {} at {next}.", state.experience, level + 1) };
        let spec = Row { name: "LEVEL", what: &what, top: level >= top, ..default() };
        row(page, look, false, spec, |c| slider(c, look, Slide::Level(top as u16), level as f32, 300.0));
    }
    let classes = [Class::Physical, Class::Mental, Class::Combat, Class::Practical];
    let count = classes.len() + 1;
    for (at, class) in classes.into_iter().enumerate().map(|(at, class)| (at + 1, class)) {
        let name = class.name().to_uppercase();
        // Each class in the game's colour for it, as its skill page marks
        // it (`SkillClassIcon`).
        let (hue, _) = crate::screens::skills_view::class_look(class);
        let elite_count = crate::screens::skills_view::shown().filter(|skill| skill.class() == class && state.skills.level(*skill) >= ELITE_LEVEL).count();
        section_head_as(page, look, at, shut(at), &name, hue, |head| {
            if elite_count > 0 {
                head.spawn((look.strong(format!("{elite_count} ELITE"), 15.0, ELITE), TextLayout::new_with_no_wrap()));
            }
            if at == 1 {
                small_button(head, look, Click::SkillsUntrained, "ALL TO 0");
            }
        });
        if shut(at) {
            continue;
        }
        for (n, skill) in crate::screens::skills_view::shown().filter(|skill| skill.class() == class).enumerate() {
            let name = skill_word(state, skill);
            let level = state.skills.level(skill);
            // A skill the app does nothing with says so at its end: the
            // hideout's two, and the four Tarkov never reads.
            let tag = crate::screens::skills_view::acts_here(skill).is_err().then_some((if matches!(skill, Skill::Crafting | Skill::HideoutManagement) { "HIDEOUT" } else { "UNUSED" }, INK_DIM));
            let spec = Row { name: &name, top: level >= ELITE_LEVEL, tag, edge: Some(hue.with_alpha(0.55)), ..default() };
            row(page, look, n % 2 == 1, spec, |c| slider(c, look, Slide::Skill(skill), level as f32, 300.0));
        }
    }
    let at = count;
    section_head(page, look, at, shut(at), "WEAPON MASTERING", |head| {
        // Every family begun, back to Level 1.
        small_button(head, look, Click::MasteryUntrained, "ALL TO LEVEL 1");
    });
    if shut(at) {
        return;
    }
    for (n, family) in state.families.all().iter().enumerate() {
        let begun = state.mastering.points(&family.name).is_some();
        let weapons: Vec<String> = family.weapons.iter().map(|weapon| state.names.get(&format!("{weapon} ShortName").to_lowercase()).cloned().unwrap_or_else(|| weapon.clone())).collect();
        let what = if begun { weapons.join(", ") } else { format!("{} (not begun)", weapons.join(", ")) };
        let name = family.name.to_uppercase();
        let level = state.mastering.level(family);
        let spec = Row { name: &name, what: &what, top: level >= trp_sim::condition::mastering::TOP_LEVEL, ..default() };
        row(page, look, n % 2 == 1, spec, |c| slider(c, look, Slide::Mastering(n as u16), level as f32 + 1.0, 300.0));
    }
}


/// A skill's name in the install's words, in capitals, or the app's.
fn skill_word(state: &OptionsState, skill: trp_sim::condition::skills::Skill) -> String {
    match skill {
        trp_sim::condition::skills::Skill::TroubleShooting if !state.names.contains_key("troubleshooting") => "TROUBLESHOOTING".to_owned(),
        _ => state.name_of(skill.name()),
    }
}
