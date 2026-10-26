//! The inventory's SKILLS tab (skills spec, "The SKILLS tab"; the user,
//! 2026-10-23: "Build the skills tab in the UI to reflect what skills are
//! enabled", and chose the game's own page with `TARKOV UI`, a skill whose
//! effects the app does not have drawn as the game draws a locked one).
//!
//! With `TARKOV UI` it is the game's page (`SkillsAndMasteringScreen`,
//! `SkillsScreen`), drawn from the install's scene: every skill of the
//! game's list as its list's view (`SkillPanel`) or its grid's
//! (`SkillIcon`), at its level as set (the options' SKILLS page, or a host
//! game in play), with its fill, its buffs' icons (`BuffIcon`) and, with the
//! mouse on its icon, the game's skill tooltip (`SkillTooltip`) with a line
//! for each buff; a buff's icon under the mouse says its buff in the simple
//! tooltip. The sort and the filter (`DropDownBox`), the list's and the
//! grid's tabs, the scroll view and its bar work as the game's. A skill the
//! game locks is drawn as it does (`CanvasGroup.SetUnlockStatus`, 0.3, on
//! the view and on its icon, which says "Not available in alpha" for the
//! skill's tooltip); one whose effects the app does not have is drawn the
//! same way with its tooltip, the reason at the tooltip's end; with the
//! options' SKILLS off, every one. The MASTERING tab is dimmed (not built),
//! and the experience at the top is the game's at none, dimmed, as the
//! OVERALL tab's statistics (no profile is kept). Without `TARKOV UI` it is
//! the app's own list.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use trp_data::screen::{ScenePanel, SkillsScreen, SKILL_DROPDOWNS, SKILL_GRID, SKILL_LIST, SKILL_SCROLL};
use trp_sim::skills::{buffs_of, BuffDef, BuffValue, Class, Skill, SkillLevels};

use crate::health_view::{copy_within, draw_nodes, inset_by_margins, prefer_size, rgba, rich_spans, screen_numbers, shown_of, tmp_words, Drawn, Frame, HealthLook, Marks, Picture, PictureReader, Piece, TmpText, Words, EXACT};
use crate::inventory_view::{words, FAINT, PANEL, WORDS};
use crate::overall_view::{bar_handle, dropdown_places, inside, screen_of, Places};
use crate::ui_sound::{Sound, Ui};

pub struct SkillsPlugin;

impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillsState>().add_systems(Update, (hear, draw).chain().after(crate::inventory_view::ScreenDrawn).run_if(crate::running));
    }
}

/// The page's node (as large as the window), drawn into afresh as what it
/// shows changes.
#[derive(Component)]
pub struct SkillsPage;

/// The page's own drawing, and the tooltip's layer over it, moved with the
/// mouse.
#[derive(Component)]
struct SkillsBody;
#[derive(Component)]
struct SkillsTip;

/// Where the page's objects are numbered among the drawn pieces: the
/// page's, each skill's view's (each one's past the one before it), the
/// tooltip's and the dropdowns' lines'.
const PAGE: usize = 25 << 20;
const VIEWS: usize = 26 << 20;
const TIP: usize = 27 << 20;
const LINES: usize = 28 << 20;

/// What a locked skill's view and icon are drawn at (`MyExtensions.
/// SetUnlockStatus`: its `CanvasGroup`'s alpha), and what the app's skills
/// that change nothing here are drawn at with them (the user's choice).
const DIM: f32 = 0.3;
/// The words a locked skill's icon says (`HoverTooltipArea._message` on
/// its `IconContainer`, as the scene has it).
const LOCKED_WORDS: &str = "Not available in alpha";
/// The level's words at elite, a key to the install's words.
const ELITE_LEVEL: &str = "<color=#c78b2c>ELITE Level</color>";
/// The sort's lines (`SkillsScreen.Show`) and the filter's after "All
/// types": the classes' names in `ESkillClass`'s order.
const SORTS: [&str; 3] = ["Default", "Most leveled", "Most recent"];
const CLASSES: [&str; 5] = ["Physical", "Combat", "Special", "Practical", "Mental"];
/// The two tabs of the list's and the grid's views, below the page's top.
const VIEW_TABS: [&str; 2] = ["BottomPanel/SkillsPanel/Options/Tabs/List Tab", "BottomPanel/SkillsPanel/Options/Tabs/Grid Tab"];
/// A skill's fill's arrows, up and down (`_effectivenessUp`,
/// `_effectivenessDown`), below the list's view's top.
const ARROWS: [&str; 2] = ["Fill Bar/Container/EffectivenessUp", "Fill Bar/Container/EffectivenessDown"];
const CAPTION_SKILLS: &str = "Caption Bar/Tabs/Skills";
const CAPTION_MASTERING: &str = "Caption Bar/Tabs/Mastering";
const EXPERIENCE: &str = "TopPanel/Progress Panel";
/// How far right and down of the mouse the skill tooltip's corner is
/// (`SkillTooltip.Show`: `(25, -25)`, the engine's y up), and the simple
/// tooltip's lower left corner up and right of it (`Tooltip.SetPosition`).
const TIP_OFFSET: f32 = 25.0;
const SIMPLE_OFFSET: f32 = 10.0;
/// How far down the app's own list starts, in pixels: under its row of
/// tabs. `ESTIMATE`: the app's own figure, with no game figure behind it
/// (the game's look has none: it is the game's page).
const OWN_TOP: f32 = 110.0;

// ==== The look read from the install ====

/// The SKILLS tab as the game draws it (`TARKOV UI`): its page, a skill's
/// views in the list and the grid, a dropdown's line, the skill tooltip,
/// and the pictures its scripts put on them: the skills' and the buffs'
/// icons (`StaticIcons`), the fills' (normal, elite; the tooltip's base,
/// normal, elite), a buff's grounds (normal, elite), the level's rank.
pub(crate) struct SkillsLook {
    pub(crate) page: SkillsScreen,
    page_drawn: Drawn,
    detailed_drawn: Drawn,
    thumb_drawn: Drawn,
    line_drawn: Drawn,
    tooltip: Option<(ScenePanel, Drawn)>,
    skill_icons: HashMap<String, Picture>,
    buff_icons: HashMap<String, Picture>,
    fills: [Option<Picture>; 2],
    grounds: [Option<Picture>; 2],
    tip_fills: [Option<Picture>; 3],
    tip_grounds: [Option<Picture>; 2],
    rank: Option<Picture>,
}

impl SkillsLook {
    /// Reads it, its pictures through the reader (`HealthLook::read`).
    pub(crate) fn read(install: &trp_data::Install, file: &str, reader: &mut PictureReader, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Option<Self> {
        let mut page = match SkillsScreen::load(install, file) {
            Ok(page) => page,
            Err(e) => {
                eprintln!("the game's SKILLS tab could not be read; the app's own list is drawn: {e:#}");
                return None;
            }
        };
        // Each dropdown's list as high as it shows of its lines (at most
        // `_maxVisibleHeight`, `DropDownBox.Update`), its words within
        // their margins, its lines' too.
        let most_seen = page.dropdown_most_seen.unwrap_or(120.0);
        let line_height = page.line.as_ref().map_or(28.0, |line| line.asks(0, 1).0.max(1.0));
        for (selector, count) in SKILL_DROPDOWNS.iter().zip([CLASSES.len() + 1, SORTS.len()]) {
            let row = page.panel.find(&format!("{selector}/Open Panel/Scroll Area/Content")).and_then(|content| page.panel.nodes[content].group).unwrap_or_default();
            let whole = row.padding[2] + row.padding[3] + count as f32 * line_height + count.saturating_sub(1) as f32 * row.spacing;
            if let Some(area) = page.panel.find(&format!("{selector}/Open Panel/Scroll Area")) {
                let element = page.panel.nodes[area].element.get_or_insert(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
                element.min[1] = whole.min(most_seen);
            }
        }
        let boxes: Vec<usize> = SKILL_DROPDOWNS.iter().filter_map(|selector| page.panel.find(&format!("{selector}/Text"))).collect();
        inset_by_margins(&mut page.panel, &boxes);
        if let Some(line) = page.line.as_mut() {
            let texts: Vec<usize> = (0..line.nodes.len()).filter(|&index| line.nodes[index].text.is_some()).collect();
            inset_by_margins(line, &texts);
        }
        // The experience's bar at none (`TwoValueBar.SetBase(0)`,
        // `SetNew(0)`): its fills closed at its left end, the glow there.
        let bar = |page: &SkillsScreen, end: &str| page.panel.find(&format!("{EXPERIENCE}/Bar/{end}"));
        if let Some(base) = bar(&page, "Base Fill") {
            page.panel.nodes[base].layout.anchor_max[0] = 0.0;
        }
        if let Some(new) = bar(&page, "New Fill") {
            let layout = &mut page.panel.nodes[new].layout;
            (layout.anchor_min[0], layout.anchor_max[0], layout.offset[0], layout.size_delta[0]) = (0.0, 0.0, 0.0, 0.0);
        }
        if let Some(glow) = bar(&page, "New Glow") {
            let layout = &mut page.panel.nodes[glow].layout;
            (layout.anchor_min[0], layout.anchor_max[0]) = (0.0, 0.0);
        }
        // What the scripts show and hide: the caption's tabs, the views'
        // tabs, the dropdowns' lists and arrows; the skills' panel on.
        let panel = &page.panel;
        let mut scripted_paths: Vec<String> = [format!("{CAPTION_SKILLS}/Normal"), format!("{CAPTION_SKILLS}/Selected")].into();
        for tab in VIEW_TABS {
            scripted_paths.extend([format!("{tab}/Normal"), format!("{tab}/Selected")]);
        }
        for selector in SKILL_DROPDOWNS {
            scripted_paths.extend(["Open Panel", "Button/Closed Image", "Button/Open Image"].map(|end| format!("{selector}/{end}")));
        }
        let scripted: Vec<usize> = scripted_paths.iter().filter_map(|path| panel.find(path)).collect();
        let mut turned = scripted.clone();
        turned.extend(panel.find("BottomPanel/SkillsPanel"));
        let mut page_drawn = Drawn { shown: shown_of(&panel.nodes, &turned, &[], &scripted), pictures: Vec::new() };
        page_drawn.pictures = reader.panel(panel, &page_drawn.shown, file, kept);
        // A skill's views: the buff's icon's template, the elite's mark and
        // the level as the scripts have them.
        let view = |reader: &mut PictureReader, kept: &mut Vec<_>, panel: &ScenePanel, scripted: &[&str]| {
            let scripted: Vec<usize> = scripted.iter().filter_map(|path| panel.find(path)).collect();
            let mut drawn = Drawn { shown: shown_of(&panel.nodes, &scripted, &[], &scripted), pictures: Vec::new() };
            drawn.pictures = reader.panel(panel, &drawn.shown, file, kept);
            drawn
        };
        let detailed_drawn = view(reader, kept, &page.detailed, &["Buffs/Buff Icon", "Skill Icon/Level Panel/Level", "Skill Icon/Level Panel/Elite Icon", ARROWS[0], ARROWS[1]]);
        let thumb_drawn = view(reader, kept, &page.thumb, &["Level Panel/Level", "Level Panel/Elite Icon"]);
        let line_drawn = page.line.as_ref().map(|line| view(reader, kept, line, &[])).unwrap_or_default();
        let sprite = |reader: &mut PictureReader, kept: &mut Vec<_>, at: &Option<(String, i64)>| at.as_ref().and_then(|(file, id)| reader.sprite(file, *id, kept));
        let fills = [sprite(reader, kept, &page.fill_sprites[0]), sprite(reader, kept, &page.fill_sprites[1])];
        let grounds = [sprite(reader, kept, &page.buff_grounds[0]), sprite(reader, kept, &page.buff_grounds[1])];
        // The tooltip: its box, as a panel of its own.
        let (mut tooltip, mut tip_fills, mut tip_grounds) = (None, [None, None, None], [None, None]);
        match trp_data::screen::skill_tooltip(install) {
            Ok(tip) => {
                if let Some(top) = tip.panel.find("Skill Tooltip") {
                    let panel = tip.panel.subtree(top);
                    let arrows = ARROWS.map(|end| format!("Progress Panel/{end}"));
                    let scripted: Vec<usize> = ["Buff Container/Buff Panel", "Header/Level Panel/Level", "Header/Level Panel/Elite Icon", arrows[0].as_str(), arrows[1].as_str()].iter().filter_map(|path| panel.find(path)).collect();
                    let mut drawn = Drawn { shown: shown_of(&panel.nodes, &scripted, &[], &scripted), pictures: Vec::new() };
                    let own = panel.file.clone();
                    drawn.pictures = reader.panel(&panel, &drawn.shown, &own, kept);
                    tooltip = Some((panel, drawn));
                }
                tip_fills = [0, 1, 2].map(|n| sprite(reader, kept, &tip.fill_sprites[n]));
                tip_grounds = [0, 1].map(|n| sprite(reader, kept, &tip.buff_grounds[n]));
            }
            Err(e) => eprintln!("the game's skill tooltip could not be read; the skills show none: {e:#}"),
        }
        let icons = |reader: &mut PictureReader, kept: &mut Vec<_>, table: anyhow::Result<Vec<(String, (String, i64))>>, what: &str| -> HashMap<String, Picture> {
            match table {
                Ok(table) => table.into_iter().filter_map(|(name, (file, id))| Some((name, reader.sprite(&file, id, kept)?))).collect(),
                Err(e) => {
                    eprintln!("the game's icons of the {what} could not be read; they are left out: {e:#}");
                    HashMap::new()
                }
            }
        };
        let skill_icons = icons(reader, kept, trp_data::screen::skill_icons(install), "skills");
        let buff_icons = icons(reader, kept, trp_data::screen::buff_icons(install), "skills' buffs");
        let rank = trp_data::screen::resource_sprite(install, "Level Icons/rank5").and_then(|(file, id)| reader.sprite(&file, id, kept));
        Some(Self { page, page_drawn, detailed_drawn, thumb_drawn, line_drawn, tooltip, skill_icons, buff_icons, fills, grounds, tip_fills, tip_grounds, rank })
    }

    /// Its panels, whose texts' fonts are read with the rest.
    pub(crate) fn panels(&self) -> Vec<&ScenePanel> {
        let mut panels = vec![&self.page.panel, &self.page.detailed, &self.page.thumb];
        panels.extend(self.page.line.as_ref());
        panels.extend(self.tooltip.as_ref().map(|(panel, _)| panel));
        panels
    }

    /// A buff's icon by its name (`BuffIdSprites`, keyed by the names the
    /// game saves its buffs by: `JsonEnumName`, which differs for one).
    fn buff_icon(&self, id: &str) -> Option<Picture> {
        let saved = match id {
            "CharismaFenceRepPenaltyReduction" => "CharismaFenceReducePenalty",
            other => other,
        };
        self.buff_icons.get(saved).copied()
    }
}

/// A skill's effectiveness with so many points earned in the raid
/// (`SkillManager.GetEffectiveness`, the install's figures): fresh
/// (`SkillFreshEffectiveness`, 1.3) below `SkillFreshPoints`, then 1 for
/// `SkillPointsBeforeFatigue` more, then falling by `SkillFatiguePerPoint`
/// a point to at least `SkillMinEffectiveness`. Nothing earns here, so a
/// skill keeps its effectiveness at none earned, as at a raid's start
/// (`Skill`'s constructor: `GetEffectiveness(0)`). None where the install
/// lacks a figure.
pub fn effectiveness(config: &serde_json::Value, earned: i32) -> Option<f32> {
    let figure = |key: &str| config.get(key)?.as_f64().map(|v| v as f32);
    let fresh = figure("SkillFreshPoints")? as i32;
    let rested = fresh + figure("SkillPointsBeforeFatigue")? as i32;
    if earned < fresh {
        return figure("SkillFreshEffectiveness");
    }
    if earned < rested {
        return Some(1.0);
    }
    Some(figure("SkillMinEffectiveness")?.max(figure("SkillFatiguePerPoint")?.powi(1 + earned - rested)))
}

/// The install's words the tab says (`Words::load`).
pub fn word_keys() -> Vec<String> {
    let mut keys: Vec<String> = ["Level ", ELITE_LEVEL, "maxed", "All types", "current:", "remaining:", LOCKED_WORDS, "SKILLS_SPEED_UP", "SKILLS_SPEED_DOWN"].into_iter().chain(SORTS).chain(CLASSES).map(str::to_owned).collect();
    for skill in Skill::ALL {
        let name = skill.name();
        keys.extend([name.to_owned(), format!("{name}Description"), format!("{name}LevelingUpDescription")]);
        keys.extend(buffs_of(skill).iter().map(|buff| buff.id.to_owned()));
    }
    keys
}

// ==== What a skill does here, and its words ====

/// What a skill changes in the app as it stands (`docs/spec/skills.md`,
/// "What each skill changes"): where the game's code has it act and the
/// app has the thing it acts on (`Ok`, what), else why it changes nothing
/// (`Err`). The options' SKILLS page writes these under each skill; the
/// SKILLS tab dims a skill that changes nothing.
pub fn acts_here(skill: Skill) -> Result<&'static str, &'static str> {
    use Skill as S;
    match skill {
        S::Endurance => Ok("the legs', arms' and breath's stamina, their recovery, a jump's cost"),
        S::Strength => Ok("top speeds, jump, weight limits, inertia, aiming's cost, a high throw, a knife's blow; elite: weapons weigh nothing"),
        S::Vitality => Ok("a hit's bleeding odds; elite: a bleeding stops by itself"),
        S::Health => Ok("a fracture's odds; hunger and thirst"),
        S::StressResistance => Ok("pain's strength and time"),
        S::Metabolism => Ok("what food and drink give; elite: no damage from hunger or thirst"),
        S::Immunity => Ok("what food and drink take; a painkiller's time"),
        S::Assault | S::Pistol | S::Revolver | S::Smg | S::Shotgun | S::Sniper | S::Lmg | S::Dmr => Ok("its weapons' reload and draw speeds and ergonomics, from the next draw"),
        S::Launcher => Err("nothing here: Tarkov ties it to an underbarrel launcher, not built here"),
        S::Hmg | S::AttachedLauncher | S::Melee => Err("nothing: Tarkov ties it to no weapon"),
        S::Throwing => Ok("elite: a throw not weakened by tired arms"),
        S::AimDrills => Ok("how fast the sights come up, from the next draw"),
        S::Surgery => Ok("a surgery's speed and the health it leaves"),
        S::CovertMovement => Ok("the steps' and the kit's noise, walking quietly"),
        S::MagDrills => Ok("loading, unloading and checking magazines; elite: rounds quicker after the first"),
        S::Perception | S::Attention | S::Search | S::TroubleShooting | S::LightVests | S::HeavyVests | S::WeaponTreatment => Err("nothing yet: what it acts on is not built here"),
        _ => Err("nothing here: out of a raid in Tarkov"),
    }
}

/// Why a skill is dimmed on the tab though the game does not lock it: the
/// options' SKILLS off, or nothing it changes built here; none where it
/// acts.
fn dimmed_why(skill: Skill, on: bool) -> Option<String> {
    if !on {
        return Some("In this app it changes nothing: SKILLS is off under the options' FEATURES.".to_owned());
    }
    acts_here(skill).err().map(|why| format!("In this app it changes {why}."))
}

/// A skill's class as the game's page has it (`SkillClassIcon`): its
/// colour and its mark.
fn class_look(class: Class) -> (Color, &'static str) {
    let (rgb, mark) = match class {
        Class::Physical => ([103, 177, 184], "Ph"),
        Class::Combat => ([184, 87, 57], "C"),
        Class::Mental => ([220, 215, 185], "M"),
        Class::Practical => ([175, 220, 169], "P"),
    };
    (Color::srgb_u8(rgb[0], rgb[1], rgb[2]), mark)
}

/// A skill's icon's border at elite (`SkillIcon.EliteBorderColor`), white
/// below.
const ELITE_BORDER: Color = Color::srgb(183.0 / 255.0, 112.0 / 255.0, 0.0);

/// The points a level of a skill takes as its page counts them
/// (`Skill.CalculateLevelExp`).
fn level_exp(level: u32) -> u32 {
    if level >= 9 { 100 } else { (level + 1) * 10 }
}

/// A skill's level and progress as its page says them (`SkillPanel.
/// OnSkillLevelChanged`, `SkillIcon`, `SkillTooltip`): "Level: 23" or the
/// elite's words; the points into the level of what it takes, or "maxed";
/// the fill's share (1 at elite) and the base fill's (0 at elite; the same
/// as the fill, nothing being earned: `BaseProgress`).
struct LevelWords {
    level: u32,
    elite: bool,
    said: String,
    progress: String,
    fill: f32,
    base: f32,
}

fn level_words(said: &Words, levels: &SkillLevels, skill: Skill) -> LevelWords {
    let level = levels.level(skill);
    let elite = level > 50;
    let share = (levels.progress(skill) / trp_sim::skills::POINTS_PER_LEVEL - level as f32).clamp(0.0, 1.0);
    let exp = level_exp(level);
    let points = ((share * exp as f32) as f64 * 100.0).round() / 100.0;
    let progress = if elite { format!("<color=#c5c3b2>{}</color>", said.word("maxed", "maxed")) } else { format!("<color=#c5c3b2><b>{}</b></color>/{exp}", crate::health_view::trimmed_figure(points as f32)) };
    let words = if elite { said.word(ELITE_LEVEL, ELITE_LEVEL) } else { format!("{}{level}", said.word("Level ", "Level: ")) };
    LevelWords { level, elite, said: words, progress, fill: if elite { 1.0 } else { share }, base: if elite { 0.0 } else { share } }
}

/// A skill's name in the install's words (`ESkillId`'s name localized).
fn skill_name(said: &Words, skill: Skill) -> String {
    said.word(skill.name(), skill.name())
}

/// A buff's words (`BuffIcon.GetBuffDescription`): its name's words with
/// its brackets blue, its value in its hole as .NET writes it.
fn buff_words(said: &Words, buff: &BuffDef, level: u32) -> String {
    let text = said.word(buff.id, buff.id).replace('[', "<color=#54c1ff>").replace(']', "</color>");
    match text.find("{0") {
        Some(_) => fill_hole(&text, buff.value(level)),
        None => text,
    }
}

/// A text's `{0...}` filled with a value as `string.Format` writes it
/// (`Single`'s formats: `0.#%`, `0.##%`, `0.#`, `F1`, `N2`, none).
fn fill_hole(text: &str, value: BuffValue) -> String {
    let Some(open) = text.find("{0") else { return text.to_owned() };
    let Some(close) = text[open..].find('}').map(|at| open + at) else { return text.to_owned() };
    let format = text[open + 2..close].strip_prefix(':').unwrap_or("");
    let written = match value {
        BuffValue::Flag(on) => if on { "True" } else { "False" }.to_owned(),
        BuffValue::Figure(figure) => dotnet_figure(figure, format),
    };
    format!("{}{written}{}", &text[..open], &text[close + 1..])
}

/// A `Single` as .NET writes it in a format: its seven figures first, then
/// rounded half away from zero to the format's places (`#` dropped where
/// nought, `0` and `F`/`N` kept), a percent a hundred times it.
fn dotnet_figure(value: f32, format: &str) -> String {
    let seven: f64 = format!("{value:.6e}").parse().unwrap_or(value as f64);
    let percent = format.ends_with('%');
    let body = format.trim_end_matches('%');
    let value = if percent { seven * 100.0 } else { seven };
    let rounded = |places: usize| {
        let k = 10f64.powi(places as i32);
        ((value.abs() * k + 0.5 + 1e-9).floor() / k).copysign(value)
    };
    let (places, kept) = match body {
        "" => return crate::health_view::trimmed_figure(value as f32),
        "F1" => (1, true),
        "N2" => (2, true),
        b => match b.split_once('.') {
            Some((_, decimals)) => (decimals.len(), decimals.chars().all(|c| c == '0')),
            None => (0, true),
        },
    };
    let mut written = format!("{:.*}", places, rounded(places));
    if !kept && written.contains('.') {
        written = written.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    if body == "N2" {
        let (whole, decimals) = written.split_once('.').unwrap_or((&written, ""));
        let (sign, digits) = whole.strip_prefix('-').map_or(("", whole), |d| ("-", d));
        let grouped: Vec<String> = digits.as_bytes().rchunks(3).rev().map(|chunk| String::from_utf8_lossy(chunk).into_owned()).collect();
        written = format!("{sign}{}.{decimals}", grouped.join(","));
    }
    if percent {
        written.push('%');
    }
    written
}

/// A whole number as the game writes it with its thousands apart
/// (`ToThousandsString`: a space between).
fn thousands(value: u32) -> String {
    let digits = value.to_string();
    let grouped: Vec<String> = digits.as_bytes().rchunks(3).rev().map(|chunk| String::from_utf8_lossy(chunk).into_owned()).collect();
    grouped.join(" ")
}

/// The skills listed, filtered and sorted as the page's dropdowns have
/// them (`SkillContainer`): all, or one class's (the filter's line less
/// one, `ESkillClass`); in the game's order, by level from the most (a
/// stable sort), or by when each last earned (none earns here: the game's
/// order, `PROVISIONAL`).
fn listed(levels: &SkillLevels, filter: usize, sort: usize) -> Vec<Skill> {
    let class = filter.checked_sub(1).map(|n| CLASSES[n]);
    let mut skills: Vec<Skill> = Skill::ALL.into_iter().filter(|skill| class.is_none_or(|class| skill.class().name() == class)).collect();
    if sort == 1 {
        skills.sort_by_key(|skill| std::cmp::Reverse(levels.level(*skill)));
    }
    skills
}

// ==== The state, the mouse ====

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum View {
    #[default]
    List,
    Grid,
}

/// What the left button went down on.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Pressed {
    Box(usize),
    Line(usize, usize),
    Tab(View),
}

/// What the mouse is on that shows a tooltip: a skill's icon (its
/// tooltip, or a locked one's words), a buff's icon in a skill's view.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Hover {
    Skill(Skill),
    Locked(Skill),
    Buff(Skill, usize),
}

/// The bar's handle held: the page's, or a dropdown's list's.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Held {
    Page,
    List(usize),
}

/// The page as it stands: its view, each dropdown's line chosen (the
/// filter's, the sort's), the list open, each list's scroll and the page's,
/// what the left button went down on, the handle held (the mouse's offset
/// from its middle), what the mouse is on, where the last drawing put what
/// shows a tooltip, whether the page showed on the frame before, and what
/// was drawn.
#[derive(Resource, Default)]
struct SkillsState {
    view: View,
    choice: [usize; 2],
    open: Option<usize>,
    list_scroll: [f32; 2],
    scroll: f32,
    pressed: Option<Pressed>,
    holding: Option<(Held, f32)>,
    hover: Option<Hover>,
    targets: Vec<([f32; 4], Hover)>,
    showed: bool,
    own_scroll: f32,
    page: Option<Entity>,
    drawn: Option<String>,
    tip_drawn: Option<String>,
    tip_size: [f32; 2],
}

/// The dropdowns' lines: the filter's, the sort's.
fn lines_of(said: &Words, role: usize) -> Vec<String> {
    match role {
        0 => std::iter::once(said.word("All types", "All types")).chain(CLASSES.iter().map(|class| said.word(class, class))).collect(),
        _ => SORTS.iter().map(|sort| said.word(sort, sort)).collect(),
    }
}

/// Where the page's scrolled things lie: the view (its mask), the first
/// cell's corner unscrolled, the grid, its width, and how high all of it
/// is and how much shows.
struct Scrolled {
    view: [f32; 4],
    corner: [f32; 2],
    grid: trp_data::screen::CellGrid,
    width: f32,
    whole: f32,
    shown: f32,
}

fn scrolled(look: &SkillsLook, view: View, count: usize, screen: [f32; 2]) -> Option<Scrolled> {
    let panel = &look.page.panel;
    let view_at = panel.find(SKILL_SCROLL)?;
    let content = panel.find(&format!("{SKILL_SCROLL}/Content"))?;
    let rect = panel.placed_in(view_at, screen).rect;
    let [cx, cy, cw, _] = panel.placed_in(content, screen).rect;
    let row = panel.nodes[content].group.unwrap_or_default();
    let grid = match view {
        View::List => look.page.list_grid?,
        View::Grid => look.page.thumb_grid?,
    };
    let width = cw - row.padding[0] - row.padding[1];
    let whole = row.padding[2] + row.padding[3] + grid.height(count, width);
    Some(Scrolled { view: rect, corner: [cx + row.padding[0], cy + row.padding[2]], grid, width, whole, shown: rect[3] })
}

/// The page's places on a screen so large: each dropdown's.
fn places(look: &SkillsLook, role: usize, screen: [f32; 2]) -> Option<Places> {
    dropdown_places(&look.page.panel, SKILL_DROPDOWNS[role], look.page.line.as_ref(), screen)
}

fn panel_rect(look: &SkillsLook, path: &str, screen: [f32; 2]) -> Option<[f32; 4]> {
    look.page.panel.find(path).map(|index| look.page.panel.placed_in(index, screen).rect)
}

/// The game's sounds, the mouse and the wheel on the page.
#[allow(clippy::too_many_arguments)]
fn hear(
    controller: Res<crate::controller::Controller>,
    selected: Res<crate::inventory_view::Selected>,
    look: Res<HealthLook>,
    features: Res<crate::options_view::Features>,
    skills: Option<Res<crate::skills::Skills>>,
    window: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut own_list: Query<&mut ScrollPosition, With<OwnList>>,
    mut state: ResMut<SkillsState>,
    mut sounds: MessageWriter<Sound>,
) {
    let shows = controller.inventory_open && selected.0 == crate::inventory_view::SKILLS;
    let state = &mut *state;
    if !shows {
        (state.open, state.pressed, state.holding, state.hover, state.showed) = (None, None, None, None, false);
        return;
    }
    let game = look.skills.as_ref().filter(|_| features.tarkov_ui && look.tarkov_ui);
    // Shown anew (`SkillsScreen.Show`): the list's view, the dropdowns at
    // their first lines, each with its sound (`UpdateValue`).
    if !state.showed {
        state.showed = true;
        (state.view, state.choice, state.scroll, state.list_scroll) = (View::List, [0, 0], 0.0, [0.0, 0.0]);
        if game.is_some() {
            sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
            sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
        }
    }
    let notches = match wheel.unit {
        bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y,
        bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y.signum(),
    };
    let Some(look) = game else {
        // The app's own list, scrolled by the wheel.
        if notches != 0.0 {
            for mut at in &mut own_list {
                at.0.y = (at.0.y - notches * 40.0).max(0.0);
                state.own_scroll = at.0.y;
            }
        }
        return;
    };
    let Ok(window) = window.single() else { return };
    let (screen, s) = screen_of(window);
    let Some(cursor_px) = window.cursor_position() else { return };
    let cursor = [cursor_px.x / s, cursor_px.y / s];
    let levels = skills.as_ref().map(|skills| skills.levels.clone()).unwrap_or_default();
    let count = listed(&levels, state.choice[0], state.choice[1]).len();
    let counts = [CLASSES.len() + 1, SORTS.len()];
    let places: [Option<Places>; 2] = std::array::from_fn(|role| places(look, role, screen));
    let over_box = |role: usize| places[role].as_ref().is_some_and(|at| inside(at.boxed, cursor));
    let over_list = |open: Option<usize>, role: usize| open == Some(role) && places[role].as_ref().is_some_and(|at| inside(at.open, cursor));
    let hovering = state.open.is_some_and(|role| over_box(role) || over_list(state.open, role));
    let page = scrolled(look, state.view, count, screen);
    let most = page.as_ref().map_or(0.0, |page| (page.whole - page.shown).max(0.0));
    state.scroll = state.scroll.clamp(0.0, most);
    // Any button pressed off the box and its list shuts it.
    if buttons.get_just_pressed().next().is_some() && state.open.is_some() && !hovering {
        state.open = None;
    }
    // The wheel: the open list's, else the page's.
    if notches != 0.0 {
        let list = state.open.filter(|role| places[*role].as_ref().is_some_and(|at| inside(at.area, cursor)));
        match (list, look.page.list_scroll) {
            (Some(role), Some(scrolling)) => {
                let (whole, shown) = places[role].as_ref().expect("its places").heights(counts[role]);
                state.list_scroll[role] = (state.list_scroll[role] - notches * scrolling.sensitivity).clamp(0.0, (whole - shown).max(0.0));
            }
            _ => {
                if let (Some(page), Some(scrolling)) = (page.as_ref(), look.page.view_scroll) {
                    if inside(page.view, cursor) && state.open.is_none() {
                        state.scroll = (state.scroll - notches * scrolling.sensitivity).clamp(0.0, most);
                    }
                }
            }
        }
    }
    let view_tab = |at: [f32; 2]| [View::List, View::Grid].into_iter().zip(VIEW_TABS).find(|(_, path)| panel_rect(look, path, screen).is_some_and(|rect| inside(rect, at))).map(|(view, _)| view);
    let page_handle = |scroll: f32| -> Option<[f32; 4]> {
        let page = page.as_ref()?;
        bar_handle(&look.page.panel, look.page.view_scroll?.bar?, screen, page.whole, page.shown, scroll)
    };
    let list_handle = |role: usize, scroll: f32| -> Option<[f32; 4]> {
        let (whole, shown) = places[role].as_ref()?.heights(counts[role]);
        bar_handle(&look.page.panel, scrolling_bar(look, role)?, screen, whole, shown, scroll)
    };
    if buttons.just_pressed(MouseButton::Left) {
        state.pressed = None;
        if let Some(role) = state.open.filter(|role| over_list(state.open, *role)) {
            let at = places[role].as_ref().expect("over its list");
            if let Some([_, hy, _, hh]) = list_handle(role, state.list_scroll[role]).filter(|handle| inside(*handle, cursor)) {
                state.holding = Some((Held::List(role), cursor[1] - (hy + hh / 2.0)));
            } else if let Some(line) = at.line_at(cursor, state.list_scroll[role], counts[role]) {
                state.pressed = Some(Pressed::Line(role, line));
            }
        } else if let Some(role) = (0..2).find(|&role| over_box(role)) {
            state.pressed = Some(Pressed::Box(role));
        } else if let Some(view) = view_tab(cursor) {
            state.pressed = Some(Pressed::Tab(view));
        } else if let Some([_, hy, _, hh]) = page_handle(state.scroll).filter(|handle| inside(*handle, cursor) && most > 0.0) {
            state.holding = Some((Held::Page, cursor[1] - (hy + hh / 2.0)));
        }
    }
    // The handle held: what it scrolls follows it.
    if let Some((held, offset)) = state.holding {
        let (bar, handle, whole_less_shown) = match held {
            Held::Page => (look.page.view_scroll.and_then(|scroll| scroll.bar), page_handle(state.scroll), most),
            Held::List(role) => {
                let (whole, shown) = places[role].as_ref().map_or((0.0, 0.0), |at| at.heights(counts[role]));
                (scrolling_bar(look, role), list_handle(role, state.list_scroll[role]), (whole - shown).max(0.0))
            }
        };
        let sliding = bar.and_then(|bar| bar.sliding).and_then(|sliding| {
            let path = &look.page.panel.nodes[sliding].path;
            panel_rect(look, path, screen)
        });
        if let (Some([_, sy, _, sh]), Some([_, _, _, hh])) = (sliding, handle) {
            let share = ((cursor[1] - offset - hh / 2.0 - sy) / (sh - hh).max(1e-3)).clamp(0.0, 1.0);
            match held {
                Held::Page => state.scroll = share * whole_less_shown,
                Held::List(role) => state.list_scroll[role] = share * whole_less_shown,
            }
        }
    }
    if buttons.just_released(MouseButton::Left) {
        let now = if let Some(role) = state.open.filter(|role| over_list(state.open, *role)) {
            places[role].as_ref().and_then(|at| at.line_at(cursor, state.list_scroll[role], counts[role])).map(|line| Pressed::Line(role, line))
        } else if let Some(role) = (0..2).find(|&role| over_box(role)) {
            Some(Pressed::Box(role))
        } else {
            view_tab(cursor).map(Pressed::Tab)
        };
        match (state.pressed.take(), now) {
            (Some(Pressed::Box(role)), Some(Pressed::Box(under))) if role == under => {
                if state.open == Some(role) {
                    state.open = None;
                } else {
                    state.open = Some(role);
                    sounds.write(Sound::Ui(Ui::MenuDropdown));
                }
            }
            (Some(Pressed::Line(role, line)), Some(Pressed::Line(under, under_line))) if role == under && line == under_line => {
                state.choice[role] = line;
                sounds.write(Sound::Ui(Ui::MenuDropdownSelect));
                state.open = None;
            }
            (Some(Pressed::Tab(view)), Some(Pressed::Tab(under))) if view == under => {
                state.view = view;
            }
            _ => {}
        }
        state.holding = None;
    }
    // What the mouse is on, by where the last drawing put things; none
    // under an open list, none outside the view's mask.
    let over_open = state.open.is_some_and(|role| over_list(state.open, role) || over_box(role));
    let in_view = page.as_ref().is_some_and(|page| inside(page.view, cursor));
    let hover = (!over_open && in_view && state.holding.is_none()).then(|| state.targets.iter().find(|(rect, _)| inside(*rect, cursor)).map(|(_, hover)| *hover)).flatten();
    if state.hover != hover {
        state.hover = hover;
    }
}

/// A dropdown's list's bar.
fn scrolling_bar(look: &SkillsLook, role: usize) -> Option<trp_data::screen::ScrollBar> {
    let panel = &look.page.panel;
    let area = panel.find(&format!("{}/Open Panel/Scroll Area", SKILL_DROPDOWNS[role]))?;
    trp_data::screen::Scrolling::of(panel, area, "ScrollRectNoDrag")?.bar
}

// ==== Drawing ====

/// The app's own list's node, scrolled by the wheel.
#[derive(Component)]
struct OwnList;

/// Draws the page afresh as what it shows changes, and its tooltip as
/// the mouse moves.
#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    controller: Res<crate::controller::Controller>,
    selected: Res<crate::inventory_view::Selected>,
    look: Res<HealthLook>,
    said: Res<Words>,
    features: Res<crate::options_view::Features>,
    skills: Option<Res<crate::skills::Skills>>,
    window: Query<&Window, With<PrimaryWindow>>,
    page: Query<Entity, With<SkillsPage>>,
    bodies: Query<Entity, With<SkillsBody>>,
    mut tips: Query<(Entity, &mut Node), With<SkillsTip>>,
    mut state: ResMut<SkillsState>,
) {
    let Ok(page) = page.single() else { return };
    if !(controller.inventory_open && selected.0 == crate::inventory_view::SKILLS) {
        return;
    }
    let Ok(window) = window.single() else { return };
    let (screen, s) = screen_of(window);
    let state = &mut *state;
    // The page's two layers, made once for each page (the screen made
    // anew makes a new page).
    let (Ok(body), Ok((tip, mut tip_node)), true) = (bodies.single(), tips.single_mut(), state.page == Some(page)) else {
        let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        commands.entity(page).despawn_related::<Children>().with_children(|page| {
            page.spawn((SkillsBody, whole.clone()));
            page.spawn((SkillsTip, Node { position_type: PositionType::Absolute, ..default() }, ZIndex(5), GlobalZIndex(45)));
        });
        (state.page, state.drawn, state.tip_drawn) = (Some(page), None, None);
        return;
    };
    let levels = skills.as_ref().map(|skills| skills.levels.clone()).unwrap_or_default();
    let on = skills.as_ref().is_some_and(|skills| skills.on());
    let game = look.skills.as_ref().filter(|_| features.tarkov_ui && look.tarkov_ui);
    let Some(skills_look) = game else {
        let key = format!("own {screen:?} {levels:?} {on}");
        if state.drawn.as_deref() != Some(key.as_str()) {
            state.drawn = Some(key);
            let scroll = state.own_scroll;
            commands.entity(body).despawn_related::<Children>().with_children(|body| draw_own(body, &said, &levels, on, scroll));
        }
        return;
    };
    let cursor = window.cursor_position().map(|at| [at.x / s, at.y / s]);
    let skills_listed = listed(&levels, state.choice[0], state.choice[1]);
    let lit: [bool; 2] = std::array::from_fn(|role| {
        let Some(cursor) = cursor else { return false };
        places(skills_look, role, screen).is_some_and(|at| inside(at.boxed, cursor) || (state.open == Some(role) && inside(at.open, cursor)))
    });
    // The bars' handles as tinted by their bars' state.
    let page_at = scrolled(skills_look, state.view, skills_listed.len(), screen);
    let held = state.holding.map(|(held, _)| held);
    let tinted = |bar: trp_data::screen::ScrollBar, rect: [f32; 4], held_now: bool| {
        let own = bar.handle.and_then(|handle| skills_look.page.panel.nodes[handle].image).map_or([1.0; 4], |image| image.color);
        let bar_rect = panel_rect(skills_look, &skills_look.page.panel.nodes[bar.object].path, screen);
        let over = cursor.is_some_and(|cursor| bar_rect.is_some_and(|rect| inside(rect, cursor)));
        let tint = bar.tints.map_or([1.0; 4], |tints| {
            let t = if held_now { tints.pressed } else if over { tints.highlighted } else { tints.normal };
            t.map(|c| (c * tints.multiplier).min(1.0))
        });
        (rect, Color::srgba(own[0] * tint[0], own[1] * tint[1], own[2] * tint[2], own[3] * tint[3]))
    };
    let page_handle = page_at.as_ref().and_then(|page| {
        let scrolling = skills_look.page.view_scroll?;
        let bar = scrolling.bar?;
        if scrolling.hides && page.whole <= page.shown {
            return None;
        }
        let rect = bar_handle(&skills_look.page.panel, bar, screen, page.whole, page.shown, state.scroll)?;
        Some(tinted(bar, rect, held == Some(Held::Page)))
    });
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
    let key = format!("{screen:?} {levels:?} {on} {:?} {:?} {:?} {:?} {} {lit:?} {page_handle:?} {list_handle:?}", state.view, state.choice, state.open, state.list_scroll, state.scroll);
    if state.drawn.as_deref() != Some(key.as_str()) {
        state.drawn = Some(key);
        let shows = BodyShows { view: state.view, choice: state.choice, open: state.open, list_scroll: state.list_scroll, scroll: state.scroll, lit, page_handle, list_handle, levels: &levels, on, listed: &skills_listed };
        let mut targets = Vec::new();
        commands.entity(body).despawn_related::<Children>().with_children(|body| {
            targets = draw_page(body, &look, skills_look, &said, screen, &shows, page_at.as_ref(), s);
        });
        state.targets = targets;
    }
    // The tooltip: drawn anew as what it shows changes, put by the mouse
    // each frame (`Tooltip.Update`), kept on the screen.
    let tip_key = format!("{:?} {levels:?} {on} {s}", state.hover);
    if state.tip_drawn.as_deref() != Some(tip_key.as_str()) {
        state.tip_drawn = Some(tip_key);
        commands.entity(tip).despawn_related::<Children>();
        match state.hover {
            Some(Hover::Skill(skill)) => {
                if let Some(built) = built_tip(&look, skills_look, &said, skill, &levels, dimmed_why(skill, on)) {
                    let size = built.size;
                    commands.entity(tip).with_children(|tip| draw_skill_tip(tip, &look, &said, &built, s));
                    tip_node.width = Val::Px(size[0] * s);
                    tip_node.height = Val::Px(size[1] * s);
                    state.tip_size = size;
                }
            }
            Some(Hover::Locked(_)) => {
                let words = said.word(LOCKED_WORDS, LOCKED_WORDS);
                commands.entity(tip).with_children(|tip| simple_tip(tip, &look, &words, s));
                (tip_node.width, tip_node.height) = (Val::Auto, Val::Auto);
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
struct BodyShows<'a> {
    view: View,
    choice: [usize; 2],
    open: Option<usize>,
    list_scroll: [f32; 2],
    scroll: f32,
    lit: [bool; 2],
    page_handle: Option<([f32; 4], Color)>,
    list_handle: Option<([f32; 4], Color)>,
    levels: &'a SkillLevels,
    on: bool,
    listed: &'a [Skill],
}

/// The words the page's own texts say: the install's for its localized
/// ones; the dropdowns' lines chosen; the experience at none
/// (`PlayerExperiencePanel.Set(0, 0)`: the first level and the second, none
/// had, what the second takes to go, none new).
fn page_texts(look: &HealthLook, skills: &SkillsLook, said: &Words, choice: [usize; 2]) -> HashMap<usize, String> {
    let panel = &skills.page.panel;
    let mut texts: HashMap<usize, String> = skills.page.localized.iter().map(|(index, key, case)| (*index, trp_data::screen::in_case(said.word(key, key), *case))).collect();
    for (role, selector) in SKILL_DROPDOWNS.iter().enumerate() {
        if let (Some(at), Some(line)) = (panel.find(&format!("{selector}/Text")), lines_of(said, role).get(choice[role])) {
            texts.insert(at, line.clone());
        }
    }
    let second = look.level_experience.iter().take(2).sum::<u32>();
    let mut put = |path: &str, words: String| {
        if let Some(at) = panel.find(&format!("{EXPERIENCE}/{path}")) {
            texts.insert(at, words);
        }
    };
    put("Current Level Panel/Level", "01".to_owned());
    put("Next Level Panel/Level", "02".to_owned());
    put("Current Text", format!("<color=#949286>{}</color> 0", said.word("current:", "current: ")));
    put("Remaining Text", format!("<color=#949286>{}</color> {}", said.word("remaining:", "remaining: "), thousands(second)));
    put("Bar/New Glow/Experience", "+0".to_owned());
    texts
}

/// Draws the game's page over the whole screen (`TARKOV UI`; screen units,
/// `s` pixels to one): its objects as `draw_nodes` draws them, each listed
/// skill's view in the scroll view's mask, the bar's handle, and the list
/// open over all of it. Gives where each skill's icon and each buff's icon
/// lie, as the mouse finds them.
#[allow(clippy::too_many_arguments)]
fn draw_page(column: &mut ChildSpawnerCommands, look: &HealthLook, skills: &SkillsLook, said: &Words, screen: [f32; 2], shows: &BodyShows, page_at: Option<&Scrolled>, s: f32) -> Vec<([f32; 4], Hover)> {
    let texts = page_texts(look, skills, said, shows.choice);
    // What its pictures and words ask of its rows (the caption's tabs: an
    // icon and a word each, laid by their row), which the layout here does
    // not ask of them itself.
    let mut asked = skills.page.panel.clone();
    look.ask_by_content(&mut asked, &skills.page_drawn, &texts);
    let panel = &asked;
    let find = |path: &str| panel.find(path);
    let mut set: HashMap<usize, bool> = HashMap::new();
    let mut colors: HashMap<usize, Color> = HashMap::new();
    for (end, on) in [("Normal", false), ("Selected", true)] {
        set.extend(find(&format!("{CAPTION_SKILLS}/{end}")).map(|at| (at, on)));
    }
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
    // The MASTERING tab not built, and the experience not kept, dimmed.
    let groups: HashMap<usize, f32> = [CAPTION_MASTERING, EXPERIENCE].iter().filter_map(|path| find(path)).map(|at| (at, DIM)).collect();
    let mut sprites: HashMap<usize, Picture> = HashMap::new();
    if let Some(rank) = skills.rank {
        sprites.extend(["Current Level Panel/Level Icon", "Next Level Panel/Level Icon"].iter().filter_map(|end| find(&format!("{EXPERIENCE}/{end}"))).map(|at| (at, rank)));
    }
    let open_panels: Vec<usize> = SKILL_DROPDOWNS.iter().filter_map(|selector| find(&format!("{selector}/Open Panel"))).collect();
    let mut skip = open_panels.clone();
    skip.extend(skills.page.view_scroll.and_then(|scroll| scroll.bar).and_then(|bar| bar.handle));
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
                let built = built_view(skills, said, *skill, shows.levels, list, shows.on, look.skill_effectiveness.unwrap_or(1.0));
                let marks = Marks { set: Some(&built.set), words: Some(&built.texts), colors: Some(&built.colors), groups: Some(&built.groups), sprites: Some(&built.sprites), ..Marks::default() };
                draw_nodes(clip, look, &built.panel, &built.drawn, VIEWS + (n << 8), marks, said, Frame::Is(cell), [x - vx, y - vy], s);
                // What the mouse finds: the icon (a locked one's words),
                // the buffs' icons.
                if let Some(icon) = built.icon {
                    let [ix, iy, iw, ih] = built.panel.rect(icon, cell);
                    let hover = if skill.locked() { Hover::Locked(*skill) } else { Hover::Skill(*skill) };
                    targets.push(([x + ix, y + iy, iw, ih], hover));
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

/// A skill's view as built for drawing: its panel (the template with its
/// buffs' icons copied in), what is drawn of it, what its scripts write and
/// put on it, its icon's place (what the mouse finds) and its buffs' icons
/// with each one's place among the skill's buffs.
struct Built {
    panel: ScenePanel,
    drawn: Drawn,
    texts: HashMap<usize, String>,
    set: HashMap<usize, bool>,
    colors: HashMap<usize, Color>,
    sprites: HashMap<usize, Picture>,
    groups: HashMap<usize, f32>,
    icon: Option<usize>,
    buffs: Vec<(usize, usize)>,
}

/// An object's child by its name.
fn child(panel: &ScenePanel, parent: usize, name: &str) -> Option<usize> {
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
    // Locked (`SetUnlockStatus`): the view and its icon; the app's
    // skills that change nothing, the same.
    let container = find(&panel, &format!("{icon}IconContainer"));
    if skill.locked() || dimmed_why(skill, on).is_some() {
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
    fn of(said: &Words, levels: &SkillLevels, skill: Skill) -> Self {
        level_words(said, levels, skill)
    }
}

/// The levelling's rate's arrows, up and down (`SkillPanel.
/// OnSkillLevelChanged`, `SkillTooltip`): in a raid (always here) and
/// below elite, up where the effectiveness is over 1, down under it.
fn arrows_of(effectiveness: f32, elite: bool) -> (bool, bool) {
    (!elite && effectiveness > 1.0, !elite && effectiveness < 1.0)
}

/// The tooltip's line of the levelling's rate (`SkillTooltip.Show`): in a
/// raid and below elite, "Leveling speed increased: 130%" in blue over 1,
/// its "decreased" in red under it, the percent cut to a whole (`(int)`).
fn rate_line(said: &Words, effectiveness: f32, elite: bool) -> String {
    let percent = (effectiveness * 100.0) as i32;
    match arrows_of(effectiveness, elite) {
        (true, _) => format!("\n\n<color=#54C1FFFF>{}: {percent}%</color>", said.word("SKILLS_SPEED_UP", "Leveling speed increased")),
        (_, true) => format!("\n\n<color=#C40000FF>{}: {percent}%</color>", said.word("SKILLS_SPEED_DOWN", "Leveling speed decreased")),
        _ => String::new(),
    }
}

/// The skill tooltip as built for drawing: its panel with a line for each
/// buff shown, what is drawn of it and what its script writes and puts on
/// it, its size as its fitter makes it, and its description's text and
/// words (drawn apart, its lines broken at its width).
struct TipBuilt {
    panel: ScenePanel,
    drawn: Drawn,
    texts: HashMap<usize, String>,
    set: HashMap<usize, bool>,
    colors: HashMap<usize, Color>,
    sprites: HashMap<usize, Picture>,
    size: [f32; 2],
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
    let runs: Vec<trp_data::tmp_text::TmpRun> = spans.into_iter().map(|(words, tint)| trp_data::tmp_text::TmpRun { text: words, size: text.size, color: tint.unwrap_or(colour), bold: text.style & 1 != 0 }).collect();
    let layout = trp_data::tmp_text::TmpLayout { em: text.size, spacing: text.spacing, kerning: text.kerning, wrap: Some(width), align: text.horizontal & 7 };
    Some(trp_data::tmp_text::draw(face, material, &runs, &layout).layout[3])
}

/// The skill tooltip for a skill (`SkillTooltip.Show`): its name, class,
/// level panel, description, fills, level and progress, and a line for each
/// of its buffs that shows (all of them in the order of their kinds,
/// `UpdateVisibility`), each its icon on its ground and its words, "ELITE"
/// before an elite one's and a switching one's at elite (`BuffPanel.
/// UpdateBuff`); laid out as its groups lay it, its size what they ask.
fn built_tip(look: &HealthLook, skills: &SkillsLook, said: &Words, skill: Skill, levels: &SkillLevels, why: Option<String>) -> Option<TipBuilt> {
    let effectiveness = look.skill_effectiveness.unwrap_or(1.0);
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
            let elite_word = buff.kind == trp_sim::skills::BuffKind::Elite || (at.elite && buff.kind == trp_sim::skills::BuffKind::Switching);
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
fn draw_skill_tip(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &TipBuilt, s: f32) {
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
            let runs = spans.into_iter().map(|(said, tint)| trp_data::tmp_text::TmpRun { text: said, size: text.size * s, color: tint.unwrap_or(colour), bold: text.style & 1 != 0 }).collect();
            let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: Some(w * s), align: text.horizontal & 7 };
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
fn simple_tip(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &str, s: f32) {
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

/// The app's own look of the tab (`TARKOV UI` off): every skill of the
/// game's list by its class, its level and progress in the game's words,
/// and under it its buffs as they stand, faint; a skill locked in Tarkov, or
/// one that changes nothing here, faint with the reason.
fn draw_own(page: &mut ChildSpawnerCommands, said: &Words, levels: &SkillLevels, on: bool, scroll: f32) {
    // Under the app's row of tabs (`ESTIMATE`: the app's own place, as
    // `overall_view`'s `OWN_DROP`).
    let area = Node { position_type: PositionType::Absolute, left: Val::Px(40.0), right: Val::Px(40.0), top: Val::Px(OWN_TOP), bottom: Val::Px(20.0), overflow: Overflow::scroll_y(), ..default() };
    page.spawn((OwnList, area, ScrollPosition(Vec2::new(0.0, scroll)), BackgroundColor(PANEL.with_alpha(0.9)))).with_children(|area| {
        area.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(24.0), row_gap: Val::Px(10.0), padding: UiRect::all(Val::Px(12.0)), align_items: AlignItems::FlexStart, ..default() }).with_children(|columns| {
            for classes in [&[Class::Physical, Class::Mental][..], &[Class::Combat], &[Class::Practical]] {
                columns.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), width: Val::Px(560.0), ..default() }).with_children(|column| {
                    for class in classes {
                        column.spawn((Node { margin: UiRect::vertical(Val::Px(6.0)), ..default() }, words(said.word(class.name(), class.name()).to_uppercase(), 12.0, FAINT)));
                        for skill in Skill::ALL.into_iter().filter(|skill| skill.class() == *class) {
                            let at = level_words(said, levels, skill);
                            let why = if skill.locked() { Some(said.word(LOCKED_WORDS, LOCKED_WORDS)) } else { dimmed_why(skill, on) };
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
                });
            }
        });
    });
}

/// Words with their rich text's tags left out.
fn plain(said: &str) -> String {
    rich_spans(said).map_or_else(|| said.to_owned(), |spans| spans.into_iter().map(|(words, _)| words).collect())
}

#[cfg(test)]
mod tests {
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
        assert_eq!(listed(&levels, 0, 0).len(), Skill::ALL.len());
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
        assert_eq!(rate_line(&said, 1.3, false), "\n\n<color=#54C1FFFF>Leveling speed increased: 130%</color>");
        assert_eq!(rate_line(&said, 1.3, true), "");
    }

    /// The skills the app dims: those it has nothing of, all of them with
    /// SKILLS off; never one that acts.
    #[test]
    fn a_skill_is_dimmed_where_it_changes_nothing_here() {
        assert!(dimmed_why(Skill::Endurance, true).is_none());
        assert!(dimmed_why(Skill::Endurance, false).is_some());
        assert!(dimmed_why(Skill::Perception, true).is_some());
        assert!(dimmed_why(Skill::Charisma, true).is_some());
        assert!(dimmed_why(Skill::Assault, true).is_none());
    }
}
