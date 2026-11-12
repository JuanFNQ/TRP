//! The page's layers and texts drawn each frame.

use super::*;

/// The app's own list's node, scrolled by the wheel.
#[derive(Component)]
pub(super) struct OwnList;

/// The page's two layers, made once for each page (the screen made anew
/// makes a new page), before it is drawn: on the frame it first shows, as
/// the game's (made on that frame, the page would be
/// drawn on the next: `courses10`'s `inventory-tabs`, the skills a frame
/// late on the run's first visit).
pub(super) fn make_layers(
    mut commands: Commands,
    controller: Res<crate::player::controller::Controller>,
    selected: Res<crate::screens::inventory_view::Selected>,
    page: Query<Entity, With<SkillsPage>>,
    bodies: Query<(), With<SkillsBody>>,
    tips: Query<(), With<SkillsTip>>,
    mut state: ResMut<SkillsState>,
) {
    let Ok(page) = page.single() else { return };
    if !(controller.inventory_open && selected.0 == crate::screens::inventory_view::SKILLS) {
        return;
    }
    if bodies.single().is_ok() && tips.single().is_ok() && state.page == Some(page) {
        return;
    }
    let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.entity(page).despawn_related::<Children>().with_children(|page| {
        page.spawn((SkillsBody, whole.clone()));
        page.spawn((SkillsTip, Node { position_type: PositionType::Absolute, ..default() }, ZIndex(5), GlobalZIndex(45)));
    });
    (state.page, state.drawn, state.tip_drawn) = (Some(page), None, None);
}

/// Draws the page afresh as what it shows changes, and its tooltip as
/// the mouse moves.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    mut commands: Commands,
    controller: Res<crate::player::controller::Controller>,
    selected: Res<crate::screens::inventory_view::Selected>,
    look: Res<HealthLook>,
    said: Res<Words>,
    features: Res<crate::screens::options_view::Features>,
    skills: Option<Res<crate::player::skills::Skills>>,
    window: Query<&Window, With<PrimaryWindow>>,
    page: Query<Entity, With<SkillsPage>>,
    bodies: Query<Entity, With<SkillsBody>>,
    mut tips: Query<(Entity, &mut Node), With<SkillsTip>>,
    mut state: ResMut<SkillsState>,
    (mastery, carried, mut icons, game_data, mut images, experience): (Option<Res<crate::player::skills::Mastery>>, Res<crate::screens::inventory_view::Carried>, ResMut<crate::screens::icons::Icons>, Res<crate::GameData>, ResMut<Assets<Image>>, Option<Res<crate::player::experience::Experience>>),
) {
    let Ok(page) = page.single() else { return };
    if !(controller.inventory_open && selected.0 == crate::screens::inventory_view::SKILLS) {
        return;
    }
    let Ok(window) = window.single() else { return };
    let (screen, s) = screen_of(window);
    let state = &mut *state;
    let (Ok(body), Ok((tip, mut tip_node)), true) = (bodies.single(), tips.single_mut(), state.page == Some(page)) else { return };
    let levels = skills.as_ref().map(|skills| skills.levels.clone()).unwrap_or_default();
    let on = skills.as_ref().is_some_and(|skills| skills.on());
    // Each skill's leveling speed (`Skill.Effectiveness`): its session's
    // with the experience earned by Tarkov's rules, whole with SIMPLE XP
    // GAIN, the fresh one of a raid's start where nothing is earned; when
    // each last earned; the character's experience.
    let earned = experience.as_deref();
    let fresh = look.skill_effectiveness.unwrap_or(1.0);
    let rates: Vec<f32> = Skill::ALL.iter().map(|skill| match earned {
        Some(xp) if xp.on() && xp.simple() => 1.0,
        Some(xp) if xp.on() => xp.session(*skill).effectiveness,
        _ => fresh,
    }).collect();
    let recent: Vec<Option<f64>> = Skill::ALL.iter().map(|skill| earned.and_then(|xp| xp.session(*skill).last)).collect();
    let xp = XpShown::of(earned, &look);
    let game = look.skills.as_ref().filter(|_| features.tarkov_ui && look.tarkov_ui);
    // The weapons' mastering rows (`mastery_view::rows`), and whether the
    // system is on.
    let rows = mastery.as_deref().map(|mastery| crate::screens::mastery_view::rows(mastery, &carried.texts, &game_data.db)).unwrap_or_default();
    let mastery_on = mastery.as_ref().is_some_and(|mastery| mastery.on());
    let Some(skills_look) = game else {
        let key = format!("own {screen:?} {levels:?} {on} {rows:?} {mastery_on}");
        if state.drawn.as_deref() != Some(key.as_str()) {
            state.drawn = Some(key);
            let scroll = state.own_scroll;
            commands.entity(body).despawn_related::<Children>().with_children(|body| draw_own(body, &said, &levels, on, scroll, &rows, mastery_on));
        }
        return;
    };
    let cursor = window.cursor_position().map(|at| [at.x / s, at.y / s]);
    // The MASTERING page, while its tab is chosen: the page's frame with
    // it, its rows, its tooltip over a grid's icon.
    if let (PageTab::Mastering, Some(mastery_look)) = (state.tab, skills_look.mastery.as_ref()) {
        fit(&mut state.fitted, skills_look, mastery_look, crate::screens::mastery_view::counts(&said, &rows));
        let mut pictures = crate::screens::mastery_view::Pictures { carried: &carried, icons: &mut icons, db: &game_data.db, on: mastery_on, images: &mut images };
        let had = crate::screens::mastery_view::pictures_had(&mut state.mastery, &rows, &mut pictures);
        let panel = &state.fitted.as_ref().expect("fitted").1;
        let lit = crate::screens::mastery_view::lit(panel, mastery_look, &state.mastery, cursor, screen);
        let key = format!("{screen:?} mastering {mastery_on} {lit:?} {xp:?} {}", crate::screens::mastery_view::key(&state.mastery, &rows, &had));
        if state.drawn.as_deref() != Some(key.as_str()) {
            state.drawn = Some(key);
            let mastery_state = &mut state.mastery;
            commands.entity(body).despawn_related::<Children>().with_children(|body| {
                draw_mastering(body, &look, skills_look, mastery_look, panel, &said, screen, &rows, mastery_state, &mut pictures, cursor, &xp, s);
            });
            state.targets = Vec::new();
        }
        let tip_key = format!("{} {s}", crate::screens::mastery_view::tip_key(&state.mastery, &rows));
        if state.tip_drawn.as_deref() != Some(tip_key.as_str()) {
            state.tip_drawn = Some(tip_key);
            commands.entity(tip).despawn_related::<Children>();
            let mastery_state = &mut state.mastery;
            let mut size = None;
            commands.entity(tip).with_children(|tip| size = crate::screens::mastery_view::draw_tip(tip, &look, mastery_look, &said, &rows, mastery_state, s));
            if let Some(size) = size {
                tip_node.width = Val::Px(size[0] * s);
                tip_node.height = Val::Px(size[1] * s);
            }
        }
        if let Some(at) = cursor.and_then(|cursor| crate::screens::mastery_view::tip_at(&state.mastery, cursor, screen)) {
            tip_node.left = Val::Px((at[0] * s).round());
            tip_node.top = Val::Px((at[1] * s).round());
            tip_node.bottom = Val::Auto;
        }
        return;
    }
    // This frame's view made (`Fill`), and those shown.
    state.fill.step();
    let skills_listed = state.fill.listed_by(&levels, state.choice[0], state.choice[1], &recent);
    let lit: [bool; 2] = std::array::from_fn(|role| {
        let Some(cursor) = cursor else { return false };
        places(skills_look, role, screen).is_some_and(|at| inside(at.boxed, cursor) || (state.open == Some(role) && inside(at.open, cursor)))
    });
    // The bars' handles as tinted by their bars' state; the page's by the
    // views laid out the frame before.
    let page_at = scrolled(skills_look, state.view, skills_listed.len(), screen);
    let bar_at = scrolled(skills_look, state.view, state.fill.laid_out, screen);
    state.fill.laid_out = skills_listed.len();
    let held = state.holding.map(|(held, _)| held);
    let tinted = |bar: trp_data::interface::screen::ScrollBar, rect: [f32; 4], held_now: bool| {
        let own = bar.handle.and_then(|handle| skills_look.page.panel.nodes[handle].image).map_or([1.0; 4], |image| image.color);
        let bar_rect = panel_rect(skills_look, &skills_look.page.panel.nodes[bar.object].path, screen);
        let over = cursor.is_some_and(|cursor| bar_rect.is_some_and(|rect| inside(rect, cursor)));
        let tint = bar.tints.map_or([1.0; 4], |tints| {
            let t = if held_now { tints.pressed } else if over { tints.highlighted } else { tints.normal };
            t.map(|c| (c * tints.multiplier).min(1.0))
        });
        (rect, Color::srgba(own[0] * tint[0], own[1] * tint[1], own[2] * tint[2], own[3] * tint[3]))
    };
    let page_handle = bar_at.as_ref().and_then(|page| {
        let scrolling = skills_look.page.view_scroll?;
        let bar = scrolling.bar?;
        if scrolling.hides && page.whole <= page.shown {
            return None;
        }
        let rect = bar_handle(&skills_look.page.panel, bar, screen, page.whole, page.shown, state.scroll)?;
        Some(tinted(bar, rect, held == Some(Held::Page)))
    });
    // The bar hidden whole while its views fit (`AutoHide`: its ground
    // with its handle, `courses10`'s `inventory-tabs`: both come with the
    // views' sixth row).
    let bar_hidden = skills_look.page.view_scroll.is_some_and(|scrolling| scrolling.hides) && bar_at.as_ref().is_none_or(|page| page.whole <= page.shown);
    let list_handle = state.open.and_then(|role| {
        let at = places(skills_look, role, screen)?;
        let (whole, shown) = at.heights(lines_of(&said, role).len());
        let bar = scrolling_bar(skills_look, role)?;
        if whole <= shown {
            return None;
        }
        let rect = bar_handle(&skills_look.page.panel, bar, screen, whole, shown, state.list_scroll[role])?;
        Some(tinted(bar, rect, held == Some(Held::List(role))))
    });
    let key = format!("{screen:?} {levels:?} {on} {:?} {:?} {:?} {:?} {} {lit:?} {page_handle:?} {list_handle:?} {skills_listed:?} {rates:?} {xp:?}", state.view, state.choice, state.open, state.list_scroll, state.scroll);
    if state.drawn.as_deref() != Some(key.as_str()) {
        state.drawn = Some(key);
        let shows = BodyShows { view: state.view, choice: state.choice, open: state.open, list_scroll: state.list_scroll, scroll: state.scroll, lit, page_handle, bar_hidden, list_handle, levels: &levels, on, listed: &skills_listed, rates: &rates, xp };
        let mut targets = Vec::new();
        commands.entity(body).despawn_related::<Children>().with_children(|body| {
            targets = draw_page(body, &look, skills_look, &said, screen, &shows, page_at.as_ref(), s);
        });
        state.targets = targets;
    }
    // The tooltip: drawn anew as what it shows changes, put by the mouse
    // each frame (`Tooltip.Update`), kept on the screen.
    let tip_key = format!("skills {:?} {levels:?} {on} {s} {rates:?}", state.hover);
    if state.tip_drawn.as_deref() != Some(tip_key.as_str()) {
        state.tip_drawn = Some(tip_key);
        commands.entity(tip).despawn_related::<Children>();
        match state.hover {
            Some(Hover::Skill(skill)) => {
                if let Some(built) = built_tip(&look, skills_look, &said, skill, &levels, dimmed_why(skill, on), rates[skill.index()]) {
                    let size = built.size;
                    commands.entity(tip).with_children(|tip| draw_skill_tip(tip, &look, &said, &built, s));
                    tip_node.width = Val::Px(size[0] * s);
                    tip_node.height = Val::Px(size[1] * s);
                    state.tip_size = size;
                }
            }
            Some(Hover::Buff(skill, at)) => {
                let level = levels.level(skill);
                if let Some(buff) = buffs_of(skill).get(at) {
                    let prefix = if buff.elite_look(level) { "<color=#c78b2c>ELITE</color> " } else { "" };
                    let words = format!("{prefix}{}", buff_words(&said, buff, level));
                    commands.entity(tip).with_children(|tip| simple_tip(tip, &look, &words, s));
                }
                (tip_node.width, tip_node.height) = (Val::Auto, Val::Auto);
            }
            None => {}
        }
    }
    let Some(cursor) = cursor else { return };
    match state.hover {
        Some(Hover::Skill(_)) => {
            // Its corner 25 right and down of the mouse, moved back onto
            // the screen where it would leave it (`CorrectPositionResolution`:
            // the left edge before the right, the bottom before the top),
            // on whole pixels.
            let size = state.tip_size;
            let mut at = [cursor[0] + TIP_OFFSET, cursor[1] + TIP_OFFSET];
            if at[0] + size[0] > screen[0] {
                at[0] = screen[0] - size[0];
            }
            at[0] = at[0].max(0.0);
            if at[1] + size[1] > screen[1] {
                at[1] = screen[1] - size[1];
            }
            at[1] = at[1].max(0.0);
            tip_node.left = Val::Px((at[0] * s).round());
            tip_node.top = Val::Px((at[1] * s).round());
            tip_node.bottom = Val::Auto;
        }
        Some(_) => {
            tip_node.left = Val::Px(((cursor[0] + SIMPLE_OFFSET) * s).min(window.width() - 40.0 * s).max(0.0));
            tip_node.bottom = Val::Px(((screen[1] - cursor[1] + SIMPLE_OFFSET) * s).min(window.height() - 30.0 * s).max(0.0));
            tip_node.top = Val::Auto;
        }
        None => {}
    }
}

/// What the page shows as it stands, for `draw_page`.
pub(super) struct BodyShows<'a> {
    pub(super) view: View,
    pub(super) choice: [usize; 2],
    pub(super) open: Option<usize>,
    pub(super) list_scroll: [f32; 2],
    pub(super) scroll: f32,
    pub(super) lit: [bool; 2],
    pub(super) page_handle: Option<([f32; 4], Color)>,
    pub(super) bar_hidden: bool,
    pub(super) list_handle: Option<([f32; 4], Color)>,
    pub(super) levels: &'a SkillLevels,
    pub(super) on: bool,
    pub(super) listed: &'a [Skill],
    pub(super) rates: &'a [f32],
    pub(super) xp: XpShown,
}

/// The words the page's own texts say: the install's for its localized
/// ones; the dropdowns' lines chosen; the character's experience
/// (`PlayerExperiencePanel.Set(exp, exp)`: its level and the next, what it
/// has, what the next takes to go, none new).
pub(super) fn page_texts(skills: &SkillsLook, said: &Words, choice: [usize; 2], xp: &XpShown) -> HashMap<usize, String> {
    let panel = &skills.page.panel;
    let mut texts: HashMap<usize, String> = skills.page.localized.iter().map(|(index, key, case)| (*index, trp_data::interface::screen::in_case(said.word(key, key), *case))).collect();
    for (role, selector) in SKILL_DROPDOWNS.iter().enumerate() {
        if let (Some(at), Some(line)) = (panel.find(&format!("{selector}/Text")), lines_of(said, role).get(choice[role])) {
            texts.insert(at, line.clone());
        }
    }
    let mut put = |path: &str, words: String| {
        if let Some(at) = panel.find(&format!("{EXPERIENCE}/{path}")) {
            texts.insert(at, words);
        }
    };
    put("Current Level Panel/Level", format!("{:02}", xp.level));
    put("Next Level Panel/Level", format!("{:02}", xp.level + 1));
    put("Current Text", format!("<color=#949286>{}</color> {}", said.word("current:", "current: "), thousands(xp.total.max(0) as u32)));
    put("Remaining Text", format!("<color=#949286>{}</color> {}", said.word("remaining:", "remaining: "), thousands((xp.next - xp.total).max(0) as u32)));
    put("Bar/New Glow/Experience", "+0".to_owned());
    texts
}
