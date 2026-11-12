//! The skill's tooltip and the app's own list.

use super::*;

/// The skill tooltip as built for drawing: its panel with a line for each
/// buff shown, what is drawn of it and what its script writes and puts on
/// it, its size as its fitter makes it, and its description's text and
/// words (drawn apart, its lines broken at its width).
pub(super) struct TipBuilt {
    pub(super) panel: ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) texts: HashMap<usize, String>,
    pub(super) set: HashMap<usize, bool>,
    pub(super) colors: HashMap<usize, Color>,
    pub(super) sprites: HashMap<usize, Picture>,
    pub(super) size: [f32; 2],
    description: Option<(usize, String)>,
}

/// The tooltip's description's words (`SkillTooltip.Show`: the skill's
/// description, a blank line, how it is levelled; the app's reason after,
/// where the app dims it).
fn tip_description(said: &Words, skill: Skill, rate: &str, why: Option<&str>) -> String {
    let name = skill.name();
    let mut words = format!("{}\n\n{}{rate}", said.word(&format!("{name}Description"), ""), said.word(&format!("{name}LevelingUpDescription"), ""));
    if let Some(why) = why {
        words.push_str(&format!("\n\n<color=#949286>{why}</color>"));
    }
    words
}

/// The height of words broken into lines at a width, as the text's font
/// asset lays them out (`trp_data::tmp_text`), where it is had.
fn wrapped_height(look: &HealthLook, panel: &ScenePanel, index: usize, said: &str, width: f32) -> Option<f32> {
    let text = panel.nodes[index].text.as_ref()?;
    let (asset, material) = look.tmp_of(panel, text)?;
    let (face, material) = (look.tmp_faces.get(&asset)?, look.tmp_materials.get(&material)?);
    let colour = screen_numbers(rgba(text.color));
    let spans = rich_spans(said).unwrap_or_else(|| vec![(said.to_owned(), None)]);
    let runs: Vec<trp_data::interface::tmp_text::TmpRun> = spans.into_iter().map(|(words, tint)| trp_data::interface::tmp_text::TmpRun { text: words, size: text.size, color: tint.unwrap_or(colour), bold: text.style & 1 != 0 }).collect();
    let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size, spacing: text.spacing, kerning: text.kerning, wrap: Some(width), align: text.horizontal & 7, lead: 0.0 };
    Some(trp_data::interface::tmp_text::draw(face, material, &runs, &layout).layout[3])
}

/// The skill tooltip for a skill (`SkillTooltip.Show`): its name, class,
/// level panel, description, fills, level and progress, and a line for each
/// of its buffs that shows (all of them in the order of their kinds,
/// `UpdateVisibility`), each its icon on its ground and its words, "ELITE"
/// before an elite one's and a switching one's at elite (`BuffPanel.
/// UpdateBuff`); laid out as its groups lay it, its size what they ask.
pub(super) fn built_tip(look: &HealthLook, skills: &SkillsLook, said: &Words, skill: Skill, levels: &SkillLevels, why: Option<String>, effectiveness: f32) -> Option<TipBuilt> {
    let (mut panel, mut drawn) = skills.tooltip.clone()?;
    let at = LevelWords::of(said, levels, skill);
    let (class_colour, mark) = class_look(skill.class());
    let mut texts = HashMap::new();
    let mut set = HashMap::new();
    let mut colors = HashMap::new();
    let mut sprites = HashMap::new();
    let find = |panel: &ScenePanel, path: &str| panel.find(path);
    for (path, words) in [("Header/Name", skill_name(said, skill)), ("Header/Skill Class Icon/Symbol", mark.to_owned()), ("Header/Level Panel/Level", at.level.to_string()), ("Progress Panel/Level", at.said.clone()), ("Progress Panel/Progress", at.progress.clone())] {
        texts.extend(find(&panel, path).map(|index| (index, words)));
    }
    set.extend(find(&panel, "Header/Level Panel/Level").map(|index| (index, !at.elite)));
    set.extend(find(&panel, "Header/Level Panel/Elite Icon").map(|index| (index, at.elite)));
    let (up, down) = arrows_of(effectiveness, at.elite);
    set.extend(find(&panel, &format!("Progress Panel/{}", ARROWS[0])).map(|index| (index, up)));
    set.extend(find(&panel, &format!("Progress Panel/{}", ARROWS[1])).map(|index| (index, down)));
    colors.extend(find(&panel, "Header/Skill Class Icon").map(|index| (index, class_colour)));
    for (path, amount, picture) in [("Progress Panel/Fill Bar/Current Fill", at.fill, skills.tip_fills[1 + at.elite as usize]), ("Progress Panel/Fill Bar/Base Fill", at.base, skills.tip_fills[0])] {
        if let Some(index) = find(&panel, path) {
            if let Some(image) = panel.nodes[index].image.as_mut() {
                image.fill_amount = amount;
            }
            sprites.extend(picture.map(|picture| (index, picture)));
        }
    }
    if let (Some(holder), Some(template)) = (find(&panel, "Buff Container"), find(&panel, "Buff Container/Buff Panel")) {
        set.insert(template, false);
        let mut order: Vec<&BuffDef> = buffs_of(skill).iter().collect();
        order.sort_by_key(|buff| buff.kind);
        for buff in order.into_iter().filter(|buff| buff.shown(at.level)) {
            let copy = copy_within(&mut panel, &mut drawn, template, holder);
            set.insert(copy, true);
            let elite_word = buff.kind == trp_sim::condition::skills::BuffKind::Elite || (at.elite && buff.kind == trp_sim::condition::skills::BuffKind::Switching);
            let words = format!("{}{}", if elite_word { "<color=#c78b2c>ELITE</color> " } else { "" }, buff_words(said, buff, at.level));
            texts.extend(child(&panel, copy, "Description").map(|index| (index, words)));
            if let Some(icon) = child(&panel, copy, "Buff Icon") {
                sprites.extend(skills.tip_grounds[buff.elite_look(at.level) as usize].map(|picture| (icon, picture)));
                if let (Some(image), Some(picture)) = (child(&panel, icon, "Image"), skills.buff_icon(buff.id)) {
                    sprites.insert(image, picture);
                }
            }
        }
    }
    // What the texts and pictures ask of the groups, the description its
    // lines' height at its width.
    look.ask_by_content(&mut panel, &drawn, &texts);
    let rate = rate_line(said, effectiveness, at.elite);
    let description = find(&panel, "Description/Text").map(|index| (index, tip_description(said, skill, &rate, why.as_deref())));
    if let Some((index, words)) = &description {
        let width = panel.nodes[*index].element.map_or(400.0, |element| element.preferred[0].max(1.0));
        let height = wrapped_height(look, &panel, *index, words, width).unwrap_or(20.0);
        prefer_size(&mut panel, *index, [width, height]);
    }
    let size = [panel.asks(0, 0).1, panel.asks(0, 1).1];
    Some(TipBuilt { panel, drawn, texts, set, colors, sprites, size, description })
}

/// Draws the skill tooltip at its layer's corner: its objects, and its
/// description in its box, its lines broken at the box's width.
pub(super) fn draw_skill_tip(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &TipBuilt, s: f32) {
    let skip: Vec<usize> = built.description.iter().map(|(index, _)| *index).collect();
    let marks = Marks { set: Some(&built.set), words: Some(&built.texts), colors: Some(&built.colors), sprites: Some(&built.sprites), skip: &skip, ..Marks::default() };
    draw_nodes(column, look, &built.panel, &built.drawn, TIP, marks, said, Frame::Is(built.size), [0.0, 0.0], s);
    let Some((index, words)) = &built.description else { return };
    let Some(text) = built.panel.nodes[*index].text.clone() else { return };
    let [x, y, w, h] = built.panel.rect(*index, built.size);
    let place = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    match look.tmp_of(&built.panel, &text) {
        Some((asset, material)) => {
            let colour = screen_numbers(rgba(text.color));
            let spans = rich_spans(words).unwrap_or_else(|| vec![(words.clone(), None)]);
            let runs = spans.into_iter().map(|(said, tint)| trp_data::interface::tmp_text::TmpRun { text: said, size: text.size * s, color: tint.unwrap_or(colour), bold: text.style & 1 != 0 }).collect();
            let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: Some(w * s), align: text.horizontal & 7, lead: 0.0 };
            column.spawn((Piece(TIP + built.panel.nodes.len()), place)).with_children(|room| {
                tmp_words(room, Node::default(), TmpText { asset, material, runs, layout }, &mut |_| {});
            });
        }
        None => {
            column.spawn((place, Text::new(words.clone()), TextFont { font_size: text.size * s, ..default() }, TextColor(rgba(text.color))));
        }
    }
}

/// The game's simple tooltip (`SimpleTooltip`): its words, their colours,
/// on black in its frame, in its look where it was read (`tooltip_words`).
pub(super) fn simple_tip(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &str, s: f32) {
    let (font, size, color, frame) = look.tooltip_words();
    let node = Node { padding: UiRect { left: Val::Px(6.0 * s), right: Val::Px(6.0 * s), top: Val::Px(2.0 * s), bottom: Val::Px(5.0 * s) }, border: UiRect::all(Val::Px(1.0)), ..default() };
    column.spawn((node, BackgroundColor(Color::BLACK), BorderColor::all(frame))).with_children(|tip| {
        let spans = rich_spans(said).unwrap_or_else(|| vec![(said.to_owned(), None)]);
        tip.spawn((Text::default(), TextFont { font: font.clone(), font_size: size * s, ..default() }, TextColor(color), TextLayout::new_with_no_wrap())).with_children(|text| {
            for (words, tint) in spans {
                let tinted = tint.map_or(color, |c| Color::srgba(c[0], c[1], c[2], c[3]));
                text.spawn((TextSpan::new(words), TextFont { font: font.clone(), font_size: size * s, ..default() }, TextColor(tinted)));
            }
        });
    });
}

/// The app's own look of the tab (`TARKOV UI` off): every skill shown by
/// its class, its level and progress in the game's words, and under it its
/// buffs as they stand, faint; one that changes nothing here faint with the
/// reason.
pub(super) fn draw_own(page: &mut ChildSpawnerCommands, said: &Words, levels: &SkillLevels, on: bool, scroll: f32, rows: &[crate::screens::mastery_view::Row], mastery_on: bool) {
    // Under the app's row of tabs (`ESTIMATE`: the app's own place, as
    // `overall_view`'s `OWN_DROP`).
    let area = Node { position_type: PositionType::Absolute, left: Val::Px(40.0), right: Val::Px(40.0), top: Val::Px(OWN_TOP), bottom: Val::Px(20.0), overflow: Overflow::scroll_y(), ..default() };
    page.spawn((OwnList, area, ScrollPosition(Vec2::new(0.0, scroll)), BackgroundColor(PANEL.with_alpha(0.9)))).with_children(|area| {
        area.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(24.0), row_gap: Val::Px(10.0), padding: UiRect::all(Val::Px(12.0)), align_items: AlignItems::FlexStart, ..default() }).with_children(|columns| {
            for classes in [&[Class::Physical, Class::Mental][..], &[Class::Combat], &[Class::Practical]] {
                columns.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), width: Val::Px(560.0), ..default() }).with_children(|column| {
                    for class in classes {
                        column.spawn((Node { margin: UiRect::vertical(Val::Px(6.0)), ..default() }, words(said.word(class.name(), class.name()).to_uppercase(), 12.0, FAINT)));
                        for skill in shown().filter(|skill| skill.class() == *class) {
                            let at = level_words(said, levels, skill);
                            let why = dimmed_why(skill, on);
                            let colour = if why.is_some() { FAINT } else { WORDS };
                            column.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|row| {
                                row.spawn((Node { width: Val::Px(220.0), ..default() }, words(skill_name(said, skill), 13.0, colour)));
                                row.spawn(words(plain(&at.said), 12.0, colour));
                                row.spawn(words(plain(&at.progress), 12.0, FAINT));
                            });
                            if let Some(why) = why {
                                column.spawn((Node { padding: UiRect::left(Val::Px(12.0)), ..default() }, words(why, 10.0, FAINT)));
                                continue;
                            }
                            let mut order: Vec<&BuffDef> = buffs_of(skill).iter().filter(|buff| buff.shown(at.level)).collect();
                            order.sort_by_key(|buff| buff.kind);
                            for buff in order {
                                column.spawn((Node { padding: UiRect::left(Val::Px(12.0)), ..default() }, words(plain(&buff_words(said, buff, at.level)), 10.0, FAINT)));
                            }
                        }
                    }
                    if classes == [Class::Practical] {
                        crate::screens::mastery_view::draw_own(column, said, rows, mastery_on);
                    }
                });
            }
        });
    });
}

/// Words with their rich text's tags left out.
fn plain(said: &str) -> String {
    rich_spans(said).map_or_else(|| said.to_owned(), |spans| spans.into_iter().map(|(words, _)| words).collect())
}
