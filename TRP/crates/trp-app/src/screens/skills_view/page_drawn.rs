//! The skills and mastering views built, with their arrows and rates.

use super::*;

/// Draws the game's page over the whole screen (`TARKOV UI`; screen units,
/// `s` pixels to one): its objects as `draw_nodes` draws them, each listed
/// skill's view in the scroll view's mask, the bar's handle, and the list
/// open over all of it. Gives where each skill's icon and each buff's icon
/// lie, as the mouse finds them.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_page(column: &mut ChildSpawnerCommands, look: &HealthLook, skills: &SkillsLook, said: &Words, screen: [f32; 2], shows: &BodyShows, page_at: Option<&Scrolled>, s: f32) -> Vec<([f32; 4], Hover)> {
    let texts = page_texts(skills, said, shows.choice, &shows.xp);
    // What its pictures and words ask of its rows (the caption's tabs: an
    // icon and a word each, laid by their row), which the layout here does
    // not ask of them itself.
    let mut asked = skills.page.panel.clone();
    set_bar(&mut asked, shows.xp.share());
    look.ask_by_content(&mut asked, &skills.page_drawn, &texts);
    let panel = &asked;
    let find = |path: &str| panel.find(path);
    let mut set: HashMap<usize, bool> = HashMap::new();
    let mut colors: HashMap<usize, Color> = HashMap::new();
    caption_marks(panel, PageTab::Skills, &mut set);
    for (view, tab) in [View::List, View::Grid].into_iter().zip(VIEW_TABS) {
        let chosen = shows.view == view;
        for (end, on) in [("Normal", !chosen), ("Selected", chosen)] {
            set.extend(find(&format!("{tab}/{end}")).map(|at| (at, on)));
        }
    }
    for (role, selector) in SKILL_DROPDOWNS.iter().enumerate() {
        let open = shows.open == Some(role);
        for (end, on) in [("Open Panel", open), ("Button/Closed Image", !open), ("Button/Open Image", open)] {
            set.extend(find(&format!("{selector}/{end}")).map(|at| (at, on)));
        }
        // The box's ground under the mouse (`Graphic.CrossFadeColor`).
        if let (true, Some(ground), Some(lit)) = (shows.lit[role], find(&format!("{selector}/Background")), skills.page.dropdown_highlight) {
            if let Some(image) = panel.nodes[ground].image {
                let c = image.color;
                colors.insert(ground, Color::srgba(c[0] * lit[0], c[1] * lit[1], c[2] * lit[2], c[3] * lit[3]));
            }
        }
    }
    // The MASTERING tab dimmed where its page could not be read.
    let dimmed: &[&str] = if skills.mastery.is_some() { &[] } else { &[CAPTION_MASTERING] };
    let groups: HashMap<usize, f32> = dimmed.iter().filter_map(|path| find(path)).map(|at| (at, DIM)).collect();
    let mut sprites: HashMap<usize, Picture> = HashMap::new();
    rank_sprites(panel, skills, &shows.xp, &mut sprites);
    let open_panels: Vec<usize> = SKILL_DROPDOWNS.iter().filter_map(|selector| find(&format!("{selector}/Open Panel"))).collect();
    let mut skip = open_panels.clone();
    skip.extend(skills.page.view_scroll.and_then(|scroll| scroll.bar).and_then(|bar| bar.handle));
    if shows.bar_hidden {
        skip.extend(skills.page.view_scroll.and_then(|scroll| scroll.bar).map(|bar| bar.object));
    }
    skip.extend([SKILL_LIST, SKILL_GRID].iter().filter_map(|path| find(path)));
    let marks = Marks { set: Some(&set), words: Some(&texts), colors: Some(&colors), groups: Some(&groups), sprites: Some(&sprites), skip: &skip, ..Marks::default() };
    draw_nodes(column, look, panel, &skills.page_drawn, PAGE, marks, said, Frame::In(screen), [0.0, 0.0], s);
    let place = |[x, y, w, h]: [f32; 4]| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    // Each listed skill's view in the view's mask, from the scroll's place.
    let mut targets = Vec::new();
    if let Some(page) = page_at {
        let [vx, vy, vw, vh] = page.view;
        let cell = page.grid.cell_in(page.width);
        column.spawn((Node { overflow: Overflow::clip(), ..place(page.view) }, Visibility::Inherited)).with_children(|clip| {
            for (n, skill) in shows.listed.iter().enumerate() {
                let [gx, gy] = page.grid.place(n, page.width);
                let (x, y) = (page.corner[0] + gx, page.corner[1] + gy - shows.scroll);
                if y + cell[1] < vy || y > vy + vh {
                    continue;
                }
                let list = shows.view == View::List;
                let built = built_view(skills, said, *skill, shows.levels, list, shows.on, shows.rates.get(skill.index()).copied().unwrap_or(1.0));
                let marks = Marks { set: Some(&built.set), words: Some(&built.texts), colors: Some(&built.colors), groups: Some(&built.groups), sprites: Some(&built.sprites), ..Marks::default() };
                draw_nodes(clip, look, &built.panel, &built.drawn, VIEWS + (n << 8), marks, said, Frame::Is(cell), [x - vx, y - vy], s);
                // What the mouse finds: the icon, the buffs' icons.
                if let Some(icon) = built.icon {
                    let [ix, iy, iw, ih] = built.panel.rect(icon, cell);
                    targets.push(([x + ix, y + iy, iw, ih], Hover::Skill(*skill)));
                }
                for (copy, at) in &built.buffs {
                    let [bx, by, bw, bh] = built.panel.rect(*copy, cell);
                    targets.push(([x + bx, y + by, bw, bh], Hover::Buff(*skill, *at)));
                }
            }
        });
        let _ = vw;
    }
    if let Some((rect, color)) = shows.page_handle {
        column.spawn((place(rect), EXACT, BackgroundColor(color)));
    }
    // The list open, over all of it (`DropDownBox.SetPanelState`): its
    // panel, its lines from the template in its scroll area's mask, its
    // bar's handle.
    let Some(role) = shows.open else { return targets };
    let selector = SKILL_DROPDOWNS[role];
    let (Some(open), Some(area)) = (find(&format!("{selector}/Open Panel")), find(&format!("{selector}/Open Panel/Scroll Area"))) else { return targets };
    let handle = find(&format!("{selector}/Open Panel/Scrollbar/Sliding Area/Handle"));
    let skip: Vec<usize> = handle.into_iter().collect();
    let marks = Marks { set: Some(&set), words: Some(&texts), within: &[open], skip: &skip, ..Marks::default() };
    draw_nodes(column, look, panel, &skills.page_drawn, PAGE, marks, said, Frame::In(screen), [0.0, 0.0], s);
    let [ax, ay, aw, ah] = panel.placed_in(area, screen).rect;
    let content = find(&format!("{selector}/Open Panel/Scroll Area/Content"));
    let row = content.and_then(|content| panel.nodes[content].group).unwrap_or_default();
    let [cx, cy, cw, _] = content.map_or([ax, ay, aw, ah], |content| panel.placed_in(content, screen).rect);
    if let Some(line) = &skills.page.line {
        let line_height = line.asks(0, 1).0.max(1.0);
        let width = cw - row.padding[0] - row.padding[1];
        let text = (0..line.nodes.len()).find(|&index| line.nodes[index].text.is_some());
        column.spawn((Node { overflow: Overflow::clip(), ..place([ax, ay, aw, ah]) }, Visibility::Inherited)).with_children(|clip| {
            for (n, words) in lines_of(said, role).iter().enumerate() {
                let top = cy - ay + row.padding[2] + n as f32 * (line_height + row.spacing) - shows.list_scroll[role];
                if top + line_height < 0.0 || top > ah {
                    continue;
                }
                let said_line: HashMap<usize, String> = text.map(|text| (text, words.clone())).into_iter().collect();
                let marks = Marks { words: Some(&said_line), ..Marks::default() };
                draw_nodes(clip, look, line, &skills.line_drawn, LINES + (n << 8), marks, said, Frame::Is([width, line_height]), [cx - ax + row.padding[0], top], s);
            }
        });
    }
    if let Some((rect, color)) = shows.list_handle {
        column.spawn((place(rect), EXACT, BackgroundColor(color)));
    }
    targets
}

/// The caption's tabs as the chosen one has them, and each page's panel
/// on with its tab (`TabGroup`).
fn caption_marks(panel: &ScenePanel, tab: PageTab, set: &mut HashMap<usize, bool>) {
    for (this, path, page) in [(PageTab::Skills, CAPTION_SKILLS, SKILLS_PANEL), (PageTab::Mastering, CAPTION_MASTERING, trp_data::interface::screen::MASTERING_PANEL)] {
        let chosen = this == tab;
        for (end, on) in [("Normal", !chosen), ("Selected", chosen)] {
            set.extend(panel.find(&format!("{path}/{end}")).map(|at| (at, on)));
        }
        set.extend(panel.find(page).map(|at| (at, chosen)));
    }
}

/// Draws the page with its MASTERING tab chosen (`TARKOV UI`): the page's
/// objects (the caption, the experience, the MASTERING page's own, as
/// `mastery_view::page_marks` has them), then the page's rows, its bar's
/// handle and its list open (`mastery_view::draw_rows`).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_mastering(
    column: &mut ChildSpawnerCommands,
    look: &HealthLook,
    skills: &SkillsLook,
    mastery: &crate::screens::mastery_view::MasteryLook,
    fitted: &ScenePanel,
    said: &Words,
    screen: [f32; 2],
    rows: &[crate::screens::mastery_view::Row],
    state: &mut crate::screens::mastery_view::MasteryState,
    pictures: &mut crate::screens::mastery_view::Pictures,
    cursor: Option<[f32; 2]>,
    xp: &XpShown,
    s: f32,
) {
    let mut texts = page_texts(skills, said, [0, 0], xp);
    let mut set: HashMap<usize, bool> = HashMap::new();
    let mut colors: HashMap<usize, Color> = HashMap::new();
    let mut skip: Vec<usize> = Vec::new();
    crate::screens::mastery_view::page_marks(fitted, mastery, said, rows, state, cursor, screen, &mut set, &mut texts, &mut colors, &mut skip);
    let mut asked = fitted.clone();
    set_bar(&mut asked, xp.share());
    look.ask_by_content(&mut asked, &skills.page_drawn, &texts);
    let panel = &asked;
    caption_marks(panel, PageTab::Mastering, &mut set);
    let groups: HashMap<usize, f32> = HashMap::new();
    let mut sprites: HashMap<usize, Picture> = HashMap::new();
    rank_sprites(panel, skills, xp, &mut sprites);
    let marks = Marks { set: Some(&set), words: Some(&texts), colors: Some(&colors), groups: Some(&groups), sprites: Some(&sprites), skip: &skip, ..Marks::default() };
    draw_nodes(column, look, panel, &skills.page_drawn, PAGE, marks, said, Frame::In(screen), [0.0, 0.0], s);
    crate::screens::mastery_view::draw_rows(column, look, mastery, panel, said, rows, state, pictures, cursor, screen, s);
}

/// A skill's view as built for drawing: its panel (the template with its
/// buffs' icons copied in), what is drawn of it, what its scripts write and
/// put on it, its icon's place (what the mouse finds) and its buffs' icons
/// with each one's place among the skill's buffs.
pub(super) struct Built {
    pub(super) panel: ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) texts: HashMap<usize, String>,
    pub(super) set: HashMap<usize, bool>,
    pub(super) colors: HashMap<usize, Color>,
    pub(super) sprites: HashMap<usize, Picture>,
    groups: HashMap<usize, f32>,
    pub(super) icon: Option<usize>,
    buffs: Vec<(usize, usize)>,
}

/// An object's child by its name.
pub(super) fn child(panel: &ScenePanel, parent: usize, name: &str) -> Option<usize> {
    (0..panel.nodes.len()).find(|&index| panel.nodes[index].parent == Some(parent) && panel.nodes[index].name == name)
}

/// A skill's view in the list (`SkillPanel.Show` with its `SkillIcon`) or
/// the grid (`SkillIcon.Show`): its name, its class's mark and colour, its
/// level and progress, its fill, its icon and its border, the elite's mark;
/// in the list its buffs' icons, those not hidden from the player in the
/// order of their kinds (`OrderBy(BuffType)`), each shown as its kind and
/// the level have it (`UpdateVisibility`) on its ground (`UpdateBuff`).
/// Dimmed where the game locks it, or the app has nothing of it.
fn built_view(skills: &SkillsLook, said: &Words, skill: Skill, levels: &SkillLevels, list: bool, on: bool, effectiveness: f32) -> Built {
    let (mut panel, mut drawn) = if list { (skills.page.detailed.clone(), skills.detailed_drawn.clone()) } else { (skills.page.thumb.clone(), skills.thumb_drawn.clone()) };
    let at = LevelWords::of(said, levels, skill);
    let icon = if list { "Skill Icon/" } else { "" };
    let (class_colour, mark) = class_look(skill.class());
    let mut texts = HashMap::new();
    let mut set = HashMap::new();
    let mut colors = HashMap::new();
    let mut sprites = HashMap::new();
    let mut groups = HashMap::new();
    let find = |panel: &ScenePanel, path: &str| panel.find(path);
    let mut put = |path: &str, words: String| {
        if let Some(index) = find(&panel, path) {
            texts.insert(index, words);
        }
    };
    put(if list { "Skill Class Icon/Background/Symbol" } else { "Skill Class Icon/Symbol" }, mark.to_owned());
    put(&format!("{icon}Level Panel/Level"), at.level.to_string());
    if list {
        put("Skill Class Icon/Name", skill_name(said, skill));
        put("Level", at.said.clone());
        put("Progress", at.progress.clone());
    } else {
        put("Points", at.progress.clone());
    }
    set.extend(find(&panel, &format!("{icon}Level Panel/Level")).map(|index| (index, !at.elite)));
    set.extend(find(&panel, &format!("{icon}Level Panel/Elite Icon")).map(|index| (index, at.elite)));
    colors.extend(find(&panel, "Skill Class Icon/Background").map(|index| (index, class_colour)));
    colors.extend(find(&panel, &format!("{icon}Border")).map(|index| (index, if at.elite { ELITE_BORDER } else { Color::WHITE })));
    if let (Some(index), Some(picture)) = (find(&panel, &format!("{icon}IconContainer/Skill Image")), skills.skill_icons.get(skill.name())) {
        sprites.insert(index, *picture);
    }
    if list {
        if let Some(fill) = find(&panel, "Fill Bar/Current Fill") {
            if let Some(image) = panel.nodes[fill].image.as_mut() {
                image.fill_amount = at.fill;
            }
            if let Some(picture) = skills.fills[at.elite as usize] {
                sprites.insert(fill, picture);
            }
        }
        if let Some(base) = find(&panel, "Fill Bar/Base Fill") {
            if let Some(image) = panel.nodes[base].image.as_mut() {
                image.fill_amount = at.base;
            }
        }
        // The arrow of the levelling's rate in a raid, below elite.
        let (up, down) = arrows_of(effectiveness, at.elite);
        set.extend(find(&panel, ARROWS[0]).map(|index| (index, up)));
        set.extend(find(&panel, ARROWS[1]).map(|index| (index, down)));
    }
    // The app's skills that change nothing, as the game draws a locked one
    // (`SetUnlockStatus`): the view and its icon.
    let container = find(&panel, &format!("{icon}IconContainer"));
    if dimmed_why(skill, on).is_some() {
        if list {
            groups.insert(0, DIM);
        }
        groups.extend(container.map(|index| (index, DIM)));
    }
    // The buffs' icons.
    let mut buffs = Vec::new();
    if let (true, Some(holder), Some(template)) = (list, find(&panel, "Buffs"), find(&panel, "Buffs/Buff Icon")) {
        // (The template itself never: what hangs from it is drawn beside
        // it, hidden only by its being hidden.)
        set.insert(template, false);
        let mut order: Vec<(usize, &BuffDef)> = buffs_of(skill).iter().enumerate().filter(|(_, buff)| !buff.hidden).collect();
        order.sort_by_key(|(_, buff)| buff.kind);
        for (n, buff) in order.into_iter().filter(|(_, buff)| buff.shown(at.level)) {
            let copy = copy_within(&mut panel, &mut drawn, template, holder);
            set.insert(copy, true);
            if let (Some(ground), Some(picture)) = (child(&panel, copy, "Background"), skills.grounds[buff.elite_look(at.level) as usize]) {
                sprites.insert(ground, picture);
            }
            if let (Some(image), Some(picture)) = (child(&panel, copy, "Image"), skills.buff_icon(buff.id)) {
                sprites.insert(image, picture);
            }
            buffs.push((copy, n));
        }
    }
    Built { panel, drawn, texts, set, colors, sprites, groups, icon: container, buffs }
}

impl LevelWords {
    pub(super) fn of(said: &Words, levels: &SkillLevels, skill: Skill) -> Self {
        level_words(said, levels, skill)
    }
}

/// The levelling's rate's arrows, up and down (`SkillPanel.
/// OnSkillLevelChanged`, `SkillTooltip`): in a raid (always here) and
/// below elite, up where the effectiveness is over 1, down under it.
pub(super) fn arrows_of(effectiveness: f32, elite: bool) -> (bool, bool) {
    (!elite && effectiveness > 1.0, !elite && effectiveness < 1.0)
}

/// The tooltip's line of the levelling's rate (`SkillTooltip.Show`): in a
/// raid and below elite, "Leveling speed increased: 129%" in blue over 1,
/// its "decreased" in red under it, the percent cut to a whole (`(int)`).
/// The single 1.3 times 100 is taken at double precision, 129.99999523,
/// which cuts to 129 (`courses10`'s `health-skills`: 129 on all three
/// tooltips; in single precision the product rounds to 130).
pub(super) fn rate_line(said: &Words, effectiveness: f32, elite: bool) -> String {
    let percent = percent_of(effectiveness);
    match arrows_of(effectiveness, elite) {
        (true, _) => format!("\n\n<color=#54C1FFFF>{}: {percent}%</color>", said.word("SKILLS_SPEED_UP", "Leveling speed increased")),
        (_, true) => format!("\n\n<color=#C40000FF>{}: {percent}%</color>", said.word("SKILLS_SPEED_DOWN", "Leveling speed decreased")),
        _ => String::new(),
    }
}

/// A rate as the tooltip's whole percent: `(int)(rate * 100f)`, the
/// product at double precision.
pub(super) fn percent_of(effectiveness: f32) -> i32 {
    (f64::from(effectiveness) * 100.0) as i32
}
