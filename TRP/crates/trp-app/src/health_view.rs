//! Health on the screen and in the world (`docs/spec/health.md`): the
//! inventory's HEALTH tab, the death screen and the run begun again, and
//! the range's barbed wire cutting the body as the game's does.
//!
//! The HEALTH tab is the game's own health panel
//! (`InventoryScreenHealthPanel`) as the install's scene lays it out and
//! pictures it (`trp_data::screen::HealthPanel`): drawn in the inventory's
//! left panel where the GEAR tab has the equipment panel, the containers
//! beside it as on that tab (`ItemsPanel.UpdateEquipment`), and each of its
//! objects kept as the game's scripts keep it (`BodyPartView`,
//! `HealthParametersPanel`, `HealthParameterPanel`). The words are the
//! install's where it has them (`locales/global/en.json`), the app's plain
//! English where it has none.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::RelativeCursorPosition;
use trp_sim::health::{BarbTally, DamageKind, Health, Kind, Part};

use crate::body_view::{BodyParts, BodyRig};
use crate::controller::Controller;
use crate::inventory_view::{words, FAINT, WORDS};
use crate::range::Range;

// ==== The plugin and the words ====

pub struct HealthViewPlugin;

impl Plugin for HealthViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_death_screen, make_pictures, share_item_looks.after(make_pictures)))
            .add_systems(Update, (fill_panel, fill_own, fill_effect_icons, show_effect_tooltip, light_drop_glow, light_info_buttons).after(crate::inventory_view::ScreenDrawn))
            .add_systems(Update, (show_death, damage_asked, damage_keys.after(crate::inventory_view::ScreenDrawn), show_version))
            .add_systems(Update, hits_taken.after(damage_asked).after(damage_keys).after(barbed_wire))
            .add_systems(Update, armour_slows)
            .add_systems(Update, barbed_wire.after(crate::controller::step))
            // Texts drawn as the game draws them, as they are made or
            // changed, before the screen is laid out.
            .add_systems(PostUpdate, draw_tmp_texts.before(bevy::ui::UiSystems::Layout));
    }
}

/// The install's words for what the page and the death screen show.
#[derive(Resource, Default)]
pub struct Words(HashMap<String, String>);

impl Words {
    /// The words wanted, and those of the health panel's own texts
    /// (`extra`: a header's text is the key its words are found by,
    /// `LocalizedText`).
    pub fn load(install: &trp_data::Install, extra: &[String]) -> Self {
        let mut wanted: Vec<String> = Part::ALL.iter().map(|part| part.name().to_owned()).collect();
        let kinds = [Kind::LightBleeding, Kind::HeavyBleeding, Kind::Wound, Kind::Pain, Kind::Tremor, Kind::Dehydration, Kind::Exhaustion, Kind::ChronicFatigue, Kind::Contusion];
        wanted.extend(kinds.iter().map(|kind| kind.name().to_owned()));
        wanted.extend(["Painkiller", "BrokenBone", "Encumbered", "OverEncumbered", "Killed in Action", "Killed by"].map(str::to_owned));
        wanted.extend(FIGURES.iter().map(|(_, key, _)| (*key).to_owned()));
        let causes = [
            DamageKind::Undefined,
            DamageKind::Fall,
            DamageKind::Barbed,
            DamageKind::Existence,
            DamageKind::Medicine,
            DamageKind::Bullet,
            DamageKind::Melee,
            DamageKind::Blunt,
            DamageKind::GrenadeFragment,
            DamageKind::Explosion,
            DamageKind::LightBleeding,
            DamageKind::HeavyBleeding,
            DamageKind::Dehydration,
            DamageKind::Exhaustion,
        ];
        wanted.extend(causes.iter().map(|kind| format!("DamageType_{}", kind.name())));
        wanted.push(SPECIAL_SLOTS_HEADER.to_owned());
        // The menu of a right click: its lines' captions and its reasons.
        wanted.extend(trp_sim::item_menu::Line::ALL.iter().map(|line| line.name().to_owned()));
        wanted.extend(trp_sim::item_menu::REASONS.iter().map(|key| (*key).to_owned()));
        wanted.extend(trp_sim::item_menu::WishLine::ALL.iter().map(|line| line.caption_key().to_owned()));
        // The window that edits a tag: its caption, its field's placeholder
        // and its button.
        wanted.extend(["ADD/EDIT CONTAINER TAG", "container tag", "Save"].map(str::to_owned));
        // The weapon's malfunctions' notices (`crate::wear`) and the repair
        // window's words (`crate::repair_view`).
        wanted.extend(crate::wear::word_keys());
        wanted.extend(crate::repair_view::word_keys());
        // The SKILLS tab's: the skills', the buffs' and the page's words.
        wanted.extend(crate::skills_view::word_keys());
        wanted.extend(extra.iter().cloned());
        let names: Vec<&str> = wanted.iter().map(String::as_str).collect();
        match trp_data::inventory::texts(install, "en", &names) {
            Ok(texts) => Self(texts),
            Err(e) => {
                eprintln!("the install's texts could not be read; the health screen is in the app's words: {e:#}");
                Self::default()
            }
        }
    }

    /// The install's words for a name, found whatever its case, or the
    /// app's.
    fn say(&self, key: &str, otherwise: &str) -> String {
        self.0.get(&key.to_lowercase()).filter(|said| !said.is_empty()).cloned().unwrap_or_else(|| otherwise.to_owned())
    }

    /// The same, for the rest of the app (the menu of a right click).
    pub fn word(&self, key: &str, otherwise: &str) -> String {
        self.say(key, otherwise)
    }

    /// An effect's name as the game's screens give it (`HealthHelper.
    /// EffectName`: a fracture is a `BrokenBone`); low health, medicine at
    /// work and hunger have none there, and are not listed.
    fn effect(&self, kind: Kind) -> Option<String> {
        let (key, otherwise) = match kind {
            Kind::LowHealth | Kind::MedEffect | Kind::Existence => return None,
            Kind::PainKiller => ("Painkiller", "On painkillers"),
            Kind::Fracture => ("BrokenBone", "Fracture"),
            Kind::Encumbered => ("Encumbered", "Overweight"),
            Kind::OverEncumbered => ("OverEncumbered", "Critical overweight"),
            other => (other.name(), other.name()),
        };
        Some(self.say(key, otherwise))
    }

    fn cause(&self, kind: DamageKind) -> String {
        self.say(&format!("DamageType_{}", kind.name()), kind.name())
    }

    /// A part's name in the install's words (a chest is a `THORAX`).
    fn part(&self, part: Part) -> String {
        let otherwise = match part {
            Part::Head => "HEAD",
            Part::Chest => "THORAX",
            Part::Stomach => "STOMACH",
            Part::LeftArm => "LEFT ARM",
            Part::RightArm => "RIGHT ARM",
            Part::LeftLeg => "LEFT LEG",
            Part::RightLeg => "RIGHT LEG",
        };
        self.say(part.name(), otherwise).to_uppercase()
    }
}

// ==== The looks read from the game's scene: HEALTH, the inspect rows, the edit build screen, OVERALL ====

/// The game's own panels of the inventory's left panel and their pictures:
/// the health panel (the HEALTH tab), and with `TARKOV UI` the GEAR tab's
/// strip of figures and the left panel's foot under both.
#[derive(Resource, Default)]
pub struct HealthLook {
    /// Whether the game's own panels are drawn (`TARKOV UI`), as the run
    /// began.
    pub tarkov_ui: bool,
    /// The game's line of its version at the screen's foot, over every
    /// screen (`trp_data::screen::version_label`), and how high over the
    /// foot it lies.
    version: Option<(trp_data::screen::ScenePanel, f32)>,
    pub panel: Option<trp_data::screen::HealthPanel>,
    /// Each panel's objects as drawn: the health panel's, the strip's and
    /// the foot's.
    health: Drawn,
    strip: Drawn,
    foot: Drawn,
    gear: Drawn,
    border: Drawn,
    /// The textures the pictures are cut from: read at the start, made
    /// images by `make_pictures`.
    textures: Vec<Option<trp_data::unity::extract::TextureData>>,
    pub(crate) images: Vec<Option<Handle<Image>>>,
    /// The views' glow as their button's animator holds it standing (its
    /// `Normal` clip: the alpha of `BackGlowImage`), lit only as a thing
    /// that can be used on the part is held over it (`Highlighted`).
    glow_alpha: Option<f32>,
    /// The glow lit as a thing that can be used on the part is carried over
    /// it (`HealthBarButton.OnPointerEnter`: the animator's `Highlighted`,
    /// its glow's alpha from 0 to 1 in a third of a second): the clip and
    /// its curve of the glow's alpha.
    glow_lit: Option<(trp_data::unity::anim::Clip, usize)>,
    /// A rate's arrows while the figure moves (their animator's
    /// `BuffPaneActive`, looping): the clip, and its curves of the arrow's
    /// height and alpha. Standing (`BuffPanePassive`), they are still and
    /// whole.
    bob: Option<(trp_data::unity::anim::Clip, usize, usize)>,
    /// The carried weight's two limits the weight's figure is coloured by
    /// (`Stamina.LowerOverweightLimit`, `UpperOverweightLimit`).
    weight_limits: Option<[f32; 2]>,
    /// The game's own fonts its panels' texts are drawn in, by their font
    /// asset (the file it is in among the scene's, and its number): read
    /// at the start, made fonts by `make_pictures`.
    font_data: Vec<((String, i64), Vec<u8>)>,
    fonts: HashMap<(String, i64), Handle<Font>>,
    /// The game's views of a thing in a slot and in a grid (`TARKOV UI`)
    /// and their pictures; made, at the start, what the inventory draws a
    /// thing by (`item_looks`).
    items: Option<trp_data::screen::ItemViews>,
    slot_item: Drawn,
    grid_item: Drawn,
    /// The load component a magazine's view shows while it is worked
    /// (`ItemViewLoadAmmoComponent`), and its pictures.
    spin: Option<trp_data::screen::ScenePanel>,
    spin_drawn: Drawn,
    pub item_looks: [Option<std::sync::Arc<ItemViewLook>>; 2],
    /// The containers' column (`TARKOV UI`): the scene's panel and its
    /// pictures, a grid view's, each container slot's empty picture
    /// (`Slots/Equipment/<slot>`), and containers' own layouts of their
    /// grids by name, each with its size and its grids' rectangles.
    pub containers: Option<trp_data::screen::ContainerViews>,
    containers_drawn: Drawn,
    grid_drawn: Drawn,
    container_backs: HashMap<String, Picture>,
    /// The parts' marks by their keys (`ItemViewLook::mods`).
    mod_pictures: HashMap<String, Picture>,
    /// The health's effects' icons by the game's names
    /// (`trp_data::screen::EFFECT_ICONS`), and the game's simple tooltip,
    /// which says what one is with the mouse on it.
    pub(crate) effect_pictures: HashMap<&'static str, Picture>,
    tooltip: Option<trp_data::screen::ScenePanel>,
    /// The battle screen's panels of the character (`TARKOV UI`): the
    /// stance panel and the health panel as drawn, and the sprites their
    /// scripts put on them (the stamina's fills, the noise's levels), by
    /// the stance panel's pointer to each.
    pub(crate) battle: Option<trp_data::screen::BattleViews>,
    pub(crate) stance_drawn: Drawn,
    pub(crate) body_drawn: Drawn,
    /// The battle screen's quick bar (`QuickViews::load_battle`) as drawn.
    pub(crate) battle_quick: Option<trp_data::screen::QuickViews>,
    pub(crate) battle_quick_drawn: Drawn,
    pub(crate) battle_sprites: HashMap<(i32, i64), Picture>,
    /// The game's own settings of when they show and how the parts are
    /// coloured.
    pub(crate) battle_modes: crate::battle_view::BattleModes,
    /// What each font asset's material draws under its letters (a rim, a
    /// drop shadow), by the asset's file and number.
    text_effects: HashMap<(String, i64), TextEffect>,
    pub layouts: HashMap<String, ([f32; 2], Vec<[f32; 4]>)>,
    /// The bar of tabs (`TARKOV UI`): the scene's bar with its tabs laid
    /// out at their width (`TabBar::lay_out`), its pictures, each tab's
    /// words, the tabs' width, and each tab's masked fill by its mask's
    /// object (`MaskedFill`).
    pub tab_bar: Option<trp_data::screen::TabBar>,
    tabs_drawn: Drawn,
    /// The screen's back button as it is drawn at rest.
    back_drawn: Drawn,
    pub tab_words: Vec<String>,
    pub tab_width: f32,
    pub tab_fills: HashMap<usize, std::sync::Arc<MaskedFill>>,
    /// The panel for a container opened (`TARKOV UI`), its pictures, its
    /// masked stripes, and the font asset of the container's name, which
    /// sizes the name's box.
    pub stash: Option<trp_data::screen::StashViews>,
    stash_drawn: Drawn,
    pub stash_fills: HashMap<usize, std::sync::Arc<MaskedFill>>,
    name_font: Option<trp_data::screen::TmpFont>,
    /// The quick-use panel (`TARKOV UI`), its views on as its script shows
    /// them, and its pictures.
    pub quick: Option<trp_data::screen::QuickViews>,
    quick_drawn: Drawn,
    /// A font asset's bold typeface, where it has one (its weight table's
    /// 700), which TextMeshPro draws its bold in.
    bold_fonts: HashMap<(String, i64), (String, i64)>,
    /// The window that shows a thing (`TARKOV UI`; `InfoWindow`), its
    /// pictures, and the font asset of its weight's figure, which sizes
    /// the figure's box (`ContentSizeFitter`).
    pub(crate) info: Option<trp_data::screen::ScenePanel>,
    info_drawn: Drawn,
    /// The window of a container's grids (`TARKOV UI`; `GridWindow`) as its
    /// script leaves it in a raid, and its pictures.
    pub(crate) grid_window: Option<trp_data::screen::GridWindowLook>,
    grid_window_drawn: Drawn,
    /// The game's repair window (`RepairWindow`; `repair_built`): its
    /// prefab, its objects as drawn, its scripts' figures and the two
    /// icons a kit's points take.
    pub(crate) repair: Option<trp_data::screen::ScenePanel>,
    repair_drawn: Drawn,
    pub(crate) repair_figures: trp_data::screen::RepairFigures,
    repair_icons: [Option<Picture>; 2],
    info_mass_font: Option<trp_data::screen::TmpFont>,
    /// The menu of a right click (`TARKOV UI`; `ItemContextMenu`), its
    /// pictures with its templates of a line on, the font asset of a
    /// line's caption, which sizes the line, and each line's icon by the
    /// line's name (`Characteristics/Icons/<line>`).
    pub(crate) menu: Option<trp_data::screen::ScenePanel>,
    menu_drawn: Drawn,
    /// The menu of a line's own lines (`TARKOV UI`; `SubMenuTemplate`) and
    /// its pictures with its templates of a line on.
    pub(crate) submenu: Option<trp_data::screen::ScenePanel>,
    submenu_drawn: Drawn,
    /// The wish list's groups' icons (`StaticIcons.WishlistSprites`), and
    /// the outlined ones a thing's view shows (`WishlistSpritesOutline`), by
    /// the group's name.
    wish_icons: HashMap<&'static str, Picture>,
    wish_outlines: HashMap<&'static str, Picture>,
    /// The window that edits a tag (`TARKOV UI`; `EditTagWindow`), with its
    /// seven colour cells on and what its texts and its grid of colours ask
    /// of its layout put on them; its pictures; and the most letters its
    /// field takes (`m_CharacterLimit`).
    pub(crate) tag_window: Option<trp_data::screen::ScenePanel>,
    tag_window_drawn: Drawn,
    pub(crate) tag_limit: u32,
    menu_font: Option<trp_data::screen::TmpFont>,
    menu_icons: HashMap<&'static str, Picture>,
    /// The font assets its texts are drawn from as the game draws them
    /// (`trp_data::tmp_text`), each with its atlas and its bold typeface's,
    /// by the asset's file and number; the materials they are drawn with,
    /// by theirs; and each asset's own material.
    pub(crate) tmp_faces: HashMap<(String, i64), std::sync::Arc<trp_data::tmp_text::TmpFace>>,
    pub(crate) tmp_materials: HashMap<(String, i64), trp_data::tmp_text::TmpMaterial>,
    tmp_default_material: HashMap<(String, i64), (String, i64)>,
    /// The OVERALL tab (`TARKOV UI`; inventory spec, "The OVERALL tab"): the
    /// scene's panel, its clothing panel on where the options' `CLOTHING`
    /// is; its pictures, a dropdown line's, and those its scripts put on it
    /// (the level's rank, the side's, the prestige's), by their names.
    pub overall: Option<trp_data::screen::OverallPanel>,
    overall_drawn: Drawn,
    line_drawn: Drawn,
    overall_pictures: HashMap<&'static str, Picture>,
    /// The SKILLS tab as the game draws it (`TARKOV UI`; `skills_view`),
    /// and the experience each level takes (`exp_table`), which its top
    /// panel counts by.
    pub(crate) skills: Option<crate::skills_view::SkillsLook>,
    pub(crate) level_experience: Vec<u32>,
    /// A skill's effectiveness as the raid begins (`skills_view::
    /// effectiveness` at none earned), which the skills' views and
    /// tooltip show; 1 where it was not read.
    pub(crate) skill_effectiveness: Option<f32>,
    /// The edit build screen as the game draws it (`TARKOV UI` with the
    /// workbench on; workbench spec, "The screen as the game draws it").
    pub(crate) edit_build: Option<EditBuildLook>,
    /// The inspect window's rows (inspect-window spec, "A row"): its bar,
    /// compact and dropdown rows' templates and pictures, their scripts'
    /// colours and the bar's adding and subtracting pictures; and the
    /// attributes' icons by the attribute's name.
    pub(crate) info_rows: Option<InfoRowsLook>,
    attribute_icons: HashMap<String, Picture>,
    /// The other three tables of a figure's icons, by the enum its id is of
    /// (`trp_sim::item_attributes::Table`): a health effect's, a damage
    /// effect's and a buff's (the buffs' by our name for the kind: the
    /// table cuts its names to thirteen letters).
    figure_icons: [HashMap<String, Picture>; 3],
    /// The slots' pictures (`Resources/Slots/<name>`, by the name in lower
    /// case), which an empty slot of the parts' panel shows; how far the
    /// window stretches (`StretchWindow._maxSize`).
    slot_pictures: HashMap<String, Picture>,
    pub(crate) info_stretch: Option<[f32; 2]>,
    /// The armour classes' marks a tooltip's words name
    /// (`<sprite name="armor_classes_N">`), by class.
    class_marks: HashMap<u32, Picture>,
}

/// The inspect window's row templates as drawn.
pub(crate) struct InfoRowsLook {
    bar: (trp_data::screen::ScenePanel, Drawn),
    compact: (trp_data::screen::ScenePanel, Drawn),
    dropdown: (trp_data::screen::ScenePanel, Drawn),
    bar_look: trp_data::screen::RowLook,
    compact_look: trp_data::screen::RowLook,
    add: Option<Picture>,
    sub: Option<Picture>,
    /// A slot of the parts' panel (`ModSlotView`), its pieces as its script
    /// shows them per slot.
    mod_slot: Option<(trp_data::screen::ScenePanel, Drawn)>,
}

/// The edit build screen as the game draws it (`EditBuildScreen`): its
/// scene, with its objects as drawn (the dropdown, the panel of a build
/// ready to assemble and the loader shown as their scripts have them); a
/// slot's view (`Prefabs/UGUI/modding_screen_slot`), its dropdown's button
/// and its bone's name shown as its script has them; an empty slot's view
/// (`EmptyItemView`, the slot's `_emptyItemViewTemplate`); a part's view
/// (`Prefabs/UGUI/Layouts/modding_selectable_layout`), made at the start
/// into how a thing's view is drawn; the bone's icons (`Modding Screen
/// Icons/<name>`, `ModdingScreenSlotView.UpdateInfo`) and the dropdown's
/// button as open and as shut (`button_mod_dropdown_open`, `_close`).
pub(crate) struct EditBuildLook {
    pub(crate) scene: std::sync::Arc<trp_data::edit_build::EditBuildScene>,
    pub(crate) screen: Drawn,
    pub(crate) slot: trp_data::screen::ScenePanel,
    pub(crate) slot_drawn: Drawn,
    pub(crate) empty: trp_data::screen::ScenePanel,
    pub(crate) empty_drawn: Drawn,
    part: Option<(trp_data::screen::ScenePanel, Drawn)>,
    pub(crate) part_look: Option<std::sync::Arc<ItemViewLook>>,
    pub(crate) bones: HashMap<String, Picture>,
    pub(crate) dropdown: [Option<Picture>; 2],
    /// Its characteristics panel (`CharacteristicsPanel`), where it was read.
    pub(crate) stats: Option<EbStatsLook>,
}

/// The edit build screen's characteristics panel as the scene has it
/// (`CharacteristicsPanel`: its durability and weight rows, its row
/// templates, the container the rows go in, its switcher and its INFO
/// button); its templates' scripts' colours.
pub(crate) struct EbStatsLook {
    panel: trp_data::screen::ScenePanel,
    drawn: Drawn,
    bar: (trp_data::screen::ScenePanel, Drawn),
    compact: (trp_data::screen::ScenePanel, Drawn),
    bar_look: trp_data::screen::RowLook,
    compact_look: trp_data::screen::RowLook,
    weight_look: trp_data::screen::RowLook,
}

impl EditBuildLook {
    /// A part's view's size as the rows and grids that hold it make it
    /// (its `LayoutElement`'s preferred size, which the slot's row and the
    /// dropdown's grid both size it by: a part of any cells is shown in a
    /// box of that size, its picture fitted in it).
    pub(crate) fn part_side(&self) -> [f32; 2] {
        let top = self.part.as_ref().and_then(|(panel, _)| panel.nodes.first());
        let element = top.and_then(|top| top.element).map(|element| element.preferred);
        match (element, top) {
            (Some([w, h]), _) if w > 0.0 && h > 0.0 => [w, h],
            (_, Some(top)) => top.layout.size_delta,
            _ => [64.0, 64.0],
        }
    }
}

/// The objects of the edit build screen its scripts show and hide: the
/// dropdown (`DropDownMenu.Show`), the panel of a build ready to assemble
/// (`SetItemReadyToAssembleActive`), the loader (`_loader`, while the
/// preview is made); and those never shown here: the warning of a vital
/// part missing (no script of the screen's shows it), the characteristics
/// (not built), the windows of the saved builds (not built), the ellipse's
/// rectangle (it draws nothing).
pub(crate) const EB_DROPDOWN: &str = "Preview Panel/DropdownMenu";
pub(crate) const EB_READY: &str = "ItemReadyToAssemble";
pub(crate) const EB_LOADER: &str = "Loader";
const EB_NEVER: [&str; 4] = ["Attention", "CharacteristicsPanel", "Context Menu Area", "Ellipse Rect"];
/// The characteristics panel's row templates, in it.
const EB_CHAR_TEMPLATE: &str = "CharacteristicsExpanded/CharTemplate";
const EB_COMPACT_TEMPLATE: &str = "CharacteristicsExpanded/CompactTemplate";
/// A slot's view's objects its script shows and hides: the dropdown's
/// button (on the mouse), the bone's name (a vital or gear slot), the
/// locked slot's fade.
pub(crate) const EB_SLOT_BUTTON: &str = "modding_menu_item/Button";
pub(crate) const EB_SLOT_NAME: &str = "Bone Icon/Bone Name Panel";
pub(crate) const EB_SLOT_FADE: &str = "modding_menu_item/LockedFade";
/// The bone's icons, by class and full or empty.
pub(crate) const EB_BONES: [&str; 6] = [
    "icon_mod_tactical_big_installed",
    "icon_mod_tactical_big_nomod",
    "icon_mod_vital_big_installed",
    "icon_mod_vital_big_nomod",
    "icon_mod_gear_big_installed",
    "icon_mod_gear_big_nomod",
];
/// Where a slot's view's script keeps its empty view's template: its
/// fourteenth field (`_emptyItemViewTemplate`), a pointer of twelve bytes
/// after the forty its fields begin at.
const EB_EMPTY_FIELD: usize = 40 + 13 * 12;

/// The OVERALL tab's objects the app goes by, below its panel's top.
pub const OVERALL_VIEW: &str = "LeftSide/CharacterPanel/PlayerModelView";
pub const OVERALL_CLOTHING: &str = "LeftSide/ClothingPanel";
pub const OVERALL_SELECTORS: [&str; 2] = ["LeftSide/ClothingPanel/Upper/ClothingSelector", "LeftSide/ClothingPanel/Lower/ClothingSelector"];
/// What of the tab is the profile's figures (its level and rank, its
/// nickname and experience, the statistics), drawn dimmed (the user,
/// 2026-10-18: "Dim out the stats for now, since we won't track them").
const OVERALL_DIMMED: [&str; 3] = ["LeftSide/CharacterPanel/Level Panel", "LeftSide/CharacterPanel/PlayerModelView/BottomField", "RightSide"];
/// How far they are dimmed: the alpha the game's own screens give what does
/// not work (`Tab.SetUnlockStatus`: its group at 0.3), as the tabs not built
/// are.
const OVERALL_DIM: f32 = 0.3;
/// The level of a character with no experience (`ProfileInfo.GetLevel(0)`:
/// the first of the experience table's levels), its rank's picture
/// (`PlayerLevelPanel.SetLevelIcon`: `Level Icons/rank` and the level's
/// five up from it) and how it is written (`{level:D2}`).
const OVERALL_RANK: &str = "Level Icons/rank5";
const OVERALL_LEVEL: &str = "01";

// ==== Pixels, masked fills and colour spaces ====

/// A texture's pixels as the engine keeps them (RGBA, its lowest row
/// first), and whether it is sampled at its nearest pixel.
pub struct Pixels {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    point: bool,
}

/// A fill cut to its mask's shape, as the game's tabs are drawn: the mask's
/// picture sliced to the object (`FillingMask`, a `Mask` that does not
/// show its own picture) and the fill's picture tiled across it from its
/// lower left at its own size (`Filling`, `Image.Type.Tiled`); what of the
/// fill lies where the mask's alpha is above none is drawn
/// (`UNITY_UI_ALPHACLIP`, which a mask's stencil is written with), in the
/// fill's colour. Each picture with its texture's pixels and its units to
/// a pixel of the sprite (one over its image's multiplier).
pub struct MaskedFill {
    mask: (std::sync::Arc<Pixels>, Picture, f32),
    fill: (std::sync::Arc<Pixels>, Picture, f32),
    color: [f32; 4],
}

impl MaskedFill {
    /// Its pixels for an object so large (units) drawn so many pixels
    /// across and down: RGBA in sRGB, its top row first.
    pub fn compose(&self, size: [f32; 2], pixels: [u32; 2]) -> Vec<u8> {
        let [w, h] = size;
        let (mask, cut, per) = (&self.mask.0, &self.mask.1, self.mask.2);
        // The sliced mask's columns and rows (`Image.GenerateSlicedSprite`):
        // its transparent margins trimmed, its border kept whole, from the
        // object's left and bottom, in units, and their texels.
        let [l, b, r, t] = cut.border;
        let [c0, c1, c2, c3] = cut.covers;
        let pad = [c0 * cut.size[0], c1 * cut.size[1], (1.0 - c2) * cut.size[0], (1.0 - c3) * cut.size[1]];
        let xs = [pad[0] * per, l * per, w - r * per, w - pad[2] * per];
        let ys = [pad[1] * per, b * per, h - t * per, h - pad[3] * per];
        let us = [cut.cut.min.x, cut.cut.min.x + (l - pad[0]), cut.cut.max.x - (r - pad[2]), cut.cut.max.x];
        let vs = [cut.cut.min.y, cut.cut.min.y + (b - pad[1]), cut.cut.max.y - (t - pad[3]), cut.cut.max.y];
        let along = |at: f32, places: [f32; 4], texels: [f32; 4]| -> Option<f32> {
            if at < places[0] || at > places[3] {
                return None;
            }
            let span = if at < places[1] { 0 } else if at < places[2] { 1 } else { 2 };
            let (from, to) = (places[span], places[span + 1]);
            let share = if to > from { (at - from) / (to - from) } else { 0.0 };
            Some(texels[span] + share * (texels[span + 1] - texels[span]))
        };
        let (fill, fill_cut, fill_per) = (&self.fill.0, &self.fill.1, self.fill.2);
        let tile = [fill_cut.size[0] * fill_per, fill_cut.size[1] * fill_per];
        let mut out = vec![0u8; (pixels[0] * pixels[1] * 4) as usize];
        for row in 0..pixels[1] {
            // From the bottom, in units, at the pixel's middle.
            let y = h - (row as f32 + 0.5) * h / pixels[1] as f32;
            for column in 0..pixels[0] {
                let x = (column as f32 + 0.5) * w / pixels[0] as f32;
                let (Some(u), Some(v)) = (along(x, xs, us), along(y, ys, vs)) else { continue };
                if sample(mask, u, v)[3] <= 0.001 {
                    continue;
                }
                let fu = fill_cut.cut.min.x + (x % tile[0]) / tile[0] * fill_cut.cut.width();
                let fv = fill_cut.cut.min.y + (y % tile[1]) / tile[1] * fill_cut.cut.height();
                let colour = sample(fill, fu, fv);
                let at = ((row * pixels[0] + column) * 4) as usize;
                for channel in 0..3 {
                    out[at + channel] = to_srgb(colour[channel] * self.color[channel]);
                }
                out[at + 3] = ((colour[3] * self.color[3]).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        out
    }
}

/// Each mask of a panel over a picture hanging from it (a `Mask` with a
/// child that draws), as that picture cut to the mask's shape
/// (`MaskedFill`), by the mask's object; the textures' pixels decoded
/// once (`decoded`, by their place among those kept).
fn masked_fills(panel: &trp_data::screen::ScenePanel, drawn: &Drawn, kept: &[Option<trp_data::unity::extract::TextureData>], decoded: &mut HashMap<usize, std::sync::Arc<Pixels>>) -> HashMap<usize, std::sync::Arc<MaskedFill>> {
    let mut pixels_of = |texture: usize| -> Option<std::sync::Arc<Pixels>> {
        if let Some(pixels) = decoded.get(&texture) {
            return Some(pixels.clone());
        }
        let data = kept.get(texture)?.as_ref()?;
        let level = *trp_data::unity::texture::mips(data).ok()?.first()?;
        let rgba = trp_data::unity::texture::decode_rgba(data, level).ok()?;
        let pixels = std::sync::Arc::new(Pixels { rgba, width: data.width, height: data.height, point: data.point });
        decoded.insert(texture, pixels.clone());
        Some(pixels)
    };
    let mut fills = HashMap::new();
    for (mask, node) in panel.nodes.iter().enumerate().filter(|(_, node)| node.scripts.iter().any(|(class, _)| class == "Mask")) {
        let Some(filling) = (0..panel.nodes.len()).find(|&index| panel.nodes[index].parent == Some(mask) && panel.nodes[index].image.is_some()) else { continue };
        let (Some(Some(mask_picture)), Some(Some(fill_picture))) = (drawn.pictures.get(mask).copied(), drawn.pictures.get(filling).copied()) else { continue };
        let (Some(mask_image), Some(fill_image)) = (node.image, panel.nodes[filling].image) else { continue };
        let (Some(mask_pixels), Some(fill_pixels)) = (pixels_of(mask_picture.texture), pixels_of(fill_picture.texture)) else { continue };
        let fill = MaskedFill {
            mask: (mask_pixels, mask_picture, 1.0 / mask_image.pixels_per_unit.max(1e-3)),
            fill: (fill_pixels, fill_picture, 1.0 / fill_image.pixels_per_unit.max(1e-3)),
            color: fill_image.color,
        };
        fills.insert(mask, std::sync::Arc::new(fill));
    }
    fills
}

/// A texture's colour at a place (texels from its lower left), filtered as
/// the graphics card filters an sRGB texture (in linear light, between
/// the four nearest texels, or the nearest alone where it is so set),
/// clamped at its edges: linear RGB and alpha.
fn sample(pixels: &Pixels, u: f32, v: f32) -> [f32; 4] {
    let (w, h) = (pixels.width as i64, pixels.height as i64);
    let texel = |x: i64, y: i64| -> [f32; 4] {
        let at = ((y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) * 4) as usize;
        let p = &pixels.rgba[at..at + 4];
        [to_linear(p[0]), to_linear(p[1]), to_linear(p[2]), p[3] as f32 / 255.0]
    };
    if pixels.point {
        return texel(u.floor() as i64, v.floor() as i64);
    }
    let (x, y) = (u - 0.5, v - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let corners = [texel(x0, y0), texel(x0 + 1, y0), texel(x0, y0 + 1), texel(x0 + 1, y0 + 1)];
    std::array::from_fn(|c| (corners[0][c] * (1.0 - fx) + corners[1][c] * fx) * (1.0 - fy) + (corners[2][c] * (1.0 - fx) + corners[3][c] * fx) * fy)
}

fn to_linear(value: u8) -> f32 {
    let c = value as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb(value: f32) -> u8 {
    let c = value.clamp(0.0, 1.0);
    let c = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round() as u8
}

// ==== What is drawn: the pieces, the pictures, the limits ====

/// One of the game's panels as drawn: whether each object shows, and its
/// picture, by its place in the panel.
#[derive(Default, Clone)]
pub(crate) struct Drawn {
    pub(crate) shown: Vec<Shown>,
    pub(crate) pictures: Vec<Option<Picture>>,
}

/// Where each panel's objects are numbered among the drawn pieces: the
/// health panel's from 0, the strip's and the foot's past it.
const STRIP: usize = 1 << 20;
const FOOT: usize = 2 << 20;
const GEAR: usize = 3 << 20;
const CONTAINERS: usize = 4 << 20;
const GRID: usize = 5 << 20;
const BORDER: usize = 6 << 20;
const TABS: usize = 7 << 20;
const STASH: usize = 8 << 20;
const QUICK: usize = 9 << 20;
const BACK_BUTTON: usize = 10 << 20;
/// The windows that show a thing, each window's past the one before it.
const INFO: usize = 14 << 20;
/// The menu of a right click.
const MENU: usize = 15 << 20;
/// The windows that edit a tag, each window's past the one before it.
const TAG_WINDOW: usize = 16 << 20;
/// The menu's lines' templates below its top (an ordinary line's, the wish
/// list's, Discard's: `SimpleSpecifiedContextButtons`), and what holds the
/// lines.
const MENU_TEMPLATES: [&str; 3] = ["InteractionButtonsContainer/Button Template", "InteractionButtonsContainer/Wishlist Template", "InteractionButtonsContainer/Dispose Template"];
const MENU_LINES: &str = "InteractionButtonsContainer";

/// Whether an object of the panel is drawn: never (it, or something it
/// hangs from, is off and nothing turns it on in a raid), always, or as a
/// script of the panel's has it (a destroyed part's cross, a figure's
/// glows, a rate's figure and arrows).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Shown {
    Never,
    Always,
    ByScript,
}

#[derive(Clone, Copy)]
pub(crate) struct Picture {
    pub(crate) texture: usize,
    /// The cut, in the texture's pixels from its first row, which is its
    /// lowest: the texture is kept as the engine has it and drawn turned
    /// up.
    pub(crate) cut: Rect,
    /// The share of the object's rectangle that the cut covers
    /// (`SpriteCut::covers`).
    covers: [f32; 4],
    /// How far in from each side a sliced image keeps the sprite whole
    /// (left, bottom, right, top), in its pixels.
    border: [f32; 4],
    /// The sprite's own size, untrimmed, a unit to a pixel.
    size: [f32; 2],
}


/// The carried weight's limits as the inventory's figures go by them
/// (`GlobalConfiguration.Stamina`): the least start of the four
/// overweight limits, and the base limit's end; the Strength skill's
/// share of them is not had, as the movement has none.
pub fn weight_limits(config: &serde_json::Value) -> Option<[f32; 2]> {
    let stamina = config.get("Stamina")?;
    let at = |name: &str, axis: &str| stamina.get(name)?.get(axis)?.as_f64().map(|v| v as f32);
    let lower = ["WalkOverweightLimits", "BaseOverweightLimits", "SprintOverweightLimits", "WalkSpeedOverweightLimits"].iter().map(|name| at(name, "x")).collect::<Option<Vec<f32>>>()?;
    Some([lower.into_iter().fold(f32::MAX, f32::min), at("BaseOverweightLimits", "y")?])
}

/// The experience each level takes beyond the one before
/// (`GlobalConfiguration.Experience.Level.Table`, `exp.level.exp_table`),
/// the first level's first.
pub fn level_experience(config: &serde_json::Value) -> Vec<u32> {
    let table = config.get("exp").and_then(|exp| exp.get("level")).and_then(|level| level.get("exp_table")).and_then(|table| table.as_array());
    table.map_or_else(Vec::new, |rows| rows.iter().filter_map(|row| row.get("exp")?.as_u64().map(|v| v as u32)).collect())
}

// ==== HealthLook: the scene read ====

impl HealthLook {
    /// Reads the panels from the scene file that holds the inventory
    /// screen (`file`), and with `TARKOV UI` on (`pictures`) the pictures
    /// their objects draw and the clips their animators play; off, the
    /// health panel's layout alone, where the app's own look puts its
    /// boxes.
    pub fn read(install: &trp_data::Install, file: &str, pictures: bool, weight_limits: Option<[f32; 2]>, layouts: &[String], clothing: bool, workbench: bool) -> Self {
        let panel = match trp_data::screen::HealthPanel::load(install, file) {
            Ok(panel) => panel,
            Err(e) => {
                eprintln!("the install's health panel could not be read; the HEALTH tab shows none of it: {e:#}");
                return Self { tarkov_ui: pictures, weight_limits, ..Self::default() };
            }
        };
        let mut look = Self { tarkov_ui: pictures, weight_limits, ..Self::default() };
        look.health.shown = health_shown(&panel);
        if let Some(strip) = &panel.strip {
            let scripted: Vec<usize> = panel.strip_figures.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.ok_glow, f.glow, f.up, f.down, f.rate]).flatten().collect();
            look.strip.shown = shown_of(&strip.nodes, &[], &[], &scripted);
        }
        if let Some(foot) = &panel.foot {
            look.foot.shown = shown_of(&foot.nodes, &[], &[], &[]);
        }
        if let Some(border) = &panel.border {
            look.border.shown = shown_of(&border.nodes, &[], &[], &[]);
        }
        if let Some(gear) = &panel.gear_panel {
            // Each slot's borders, fill and picture as it is full or empty
            // (`SlotView.SetSlotGraphics`); its header's arrow only out of
            // a raid (`SlotViewHeader.Show`, the header not clickable).
            let scripted: Vec<usize> = panel.slot_views.iter().flat_map(|(_, p)| [p.full_border, p.back_image, p.background, p.empty_border]).flatten().collect();
            let arrows: Vec<usize> = panel.slot_views.iter().filter_map(|(_, p)| gear.find(&format!("{}/ArrowHolder", gear.nodes[p.header?].path))).collect();
            look.gear.shown = shown_of(&gear.nodes, &[], &arrows, &scripted);
        }
        if !pictures {
            // The OVERALL tab's layout, where the app's own look puts its
            // boxes.
            look.overall = overall_panel(install, file, clothing);
            look.panel = Some(panel);
            return look;
        }
        match trp_data::screen::ScenePictures::new(install) {
            Ok(mut pictures) => {
                let mut reader = PictureReader { pictures: &mut pictures, textures: HashMap::new(), missing: Vec::new() };
                let mut read = |scene: &trp_data::screen::ScenePanel, shown: &[Shown], kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>| reader.panel(scene, shown, file, kept);
                look.health.pictures = read(&panel.panel, &look.health.shown, &mut look.textures);
                if let Some(strip) = &panel.strip {
                    look.strip.pictures = read(strip, &look.strip.shown, &mut look.textures);
                }
                if let Some(foot) = &panel.foot {
                    look.foot.pictures = read(foot, &look.foot.shown, &mut look.textures);
                }
                if let Some(border) = &panel.border {
                    look.border.pictures = read(border, &look.border.shown, &mut look.textures);
                }
                if let Some(gear) = &panel.gear_panel {
                    look.gear.pictures = read(gear, &look.gear.shown, &mut look.textures);
                }
                // The containers' column: its panel with its row template,
                // its grid view, the slots' empty pictures, the layouts.
                match trp_data::screen::ContainerViews::load(install, file) {
                    Ok(views) => {
                        let template = &views.panel.nodes[views.template].path;
                        let by_slot: Vec<usize> = ["Full Border", "Empty Border", "Background", "BackImage"].iter().filter_map(|name| views.panel.find(&format!("{template}/Slot Panel/{name}"))).collect();
                        // The row template, and its special slots' panel
                        // and view, which a container with special slots
                        // turns on (`SearchableSlotView.CreateSlots`).
                        let mut turned = vec![views.template];
                        turned.extend(["Slot Template/Special Slots Panel Template", "Slot Template/Special Slot Template"].iter().filter_map(|end| (0..views.panel.nodes.len()).find(|&index| views.panel.nodes[index].path.ends_with(end))));
                        look.containers_drawn.shown = shown_of(&views.panel.nodes, &turned, &[], &by_slot);
                        look.containers_drawn.pictures = read(&views.panel, &look.containers_drawn.shown, &mut look.textures);
                        if let Some(grid) = &views.grid {
                            look.grid_drawn.shown = shown_of(&grid.nodes, &[], &[], &[]);
                            look.grid_drawn.pictures = read(grid, &look.grid_drawn.shown, &mut look.textures);
                        }
                        look.containers = Some(views);
                    }
                    Err(e) => eprintln!("the game's containers' column could not be read; it is drawn the app's way: {e:#}"),
                }

                look.layouts = trp_data::screen::rig_layouts(install, layouts);
                // The bar of tabs, its tabs laid out at the width the game
                // works out from their words in their fonts.
                match trp_data::screen::TabBar::load(install, file) {
                    Ok(mut bar) => {
                        let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
                        let texts = trp_data::inventory::texts(install, "en", &keys).unwrap_or_default();
                        let say = |key: &str| texts.get(&key.to_lowercase()).cloned();
                        let width = bar.width(install, &say);
                        bar.lay_out(width);
                        look.tab_words = bar.tabs.iter().map(|tab| trp_data::screen::TabBar::words(tab, &say)).collect();
                        look.tab_width = width;
                        // Each tab's versions shown as it is chosen and as
                        // the mouse is on it; its words' samples, which
                        // only size it, never.
                        let turned: Vec<usize> = bar.tabs.iter().flat_map(|tab| [tab.selected, tab.hover]).flatten().collect();
                        let scripted: Vec<usize> = bar.tabs.iter().flat_map(|tab| [tab.normal, tab.selected, tab.hover, tab.idle]).flatten().collect();
                        // The prestige tab's mark of a level ready to finish
                        // (`NewState`, the screen's `_prestigeNewLevelIcon`):
                        // off unless the next level's task can be finished and
                        // has not been seen (`PrestigeLevelAvailabilityChanged`),
                        // which a character with no prestige never has.
                        let mut never: Vec<usize> = bar.tabs.iter().flat_map(|tab| tab.size_definers.clone()).collect();
                        never.extend((0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "NewState"));
                        look.tabs_drawn.shown = shown_of(&bar.panel.nodes, &turned, &never, &scripted);
                        look.tabs_drawn.pictures = read(&bar.panel, &look.tabs_drawn.shown, &mut look.textures);
                        // The back button at rest (`DefaultUIButtonAnimation`,
                        // normal: its ground clear, its words in their own
                        // colour), as the scene keeps it.
                        if let Some(back) = &bar.back {
                            look.back_drawn.shown = shown_of(&back.nodes, &[], &[], &[]);
                            look.back_drawn.pictures = read(back, &look.back_drawn.shown, &mut look.textures);
                        }
                        look.tab_bar = Some(bar);
                    }
                    Err(e) => eprintln!("the game's bar of tabs could not be read; the app's tabs are drawn: {e:#}"),
                }
                // The panel for a container opened, as a raid shows it: the
                // panel and its header turned on; the money, what is only in
                // the lobby, the sorting table's and the search's tabs and
                // the mark of what is had from outside a raid off
                // (`SimpleStashPanel.Show`); its scroll bars drawn with the
                // scrolling (`scroll_views`); its filters' versions as the
                // filter chosen (`FilterPanel.Show`, all).
                match trp_data::screen::StashViews::load(install, file) {
                    Ok(mut stash) => {
                        let turned: Vec<usize> = [Some(stash.simple), stash.header].into_iter().flatten().collect();
                        let bars: Vec<usize> = (0..stash.panel.nodes.len()).filter(|&index| stash.panel.nodes[index].scripts.iter().any(|(class, _)| class == "Scrollbar")).collect();
                        let mut never: Vec<usize> = [stash.money, stash.sorting_table, stash.search, stash.external].into_iter().flatten().chain(stash.lobby_only.iter().copied()).chain(bars).collect();
                        never.extend(stash.panel.find("Complex Loot Panel"));
                        // On and off as the script leaves them, which is what
                        // its rows lay out (the header's row is sized to it).
                        for &index in &turned {
                            stash.panel.nodes[index].active = true;
                        }
                        for &index in &never {
                            stash.panel.nodes[index].active = false;
                        }
                        let scripted: Vec<usize> = stash.filter_tabs.iter().flat_map(|tab| [tab.normal, tab.selected]).flatten().collect();
                        look.stash_drawn.shown = shown_of(&stash.panel.nodes, &turned, &never, &scripted);
                        look.stash_drawn.pictures = read(&stash.panel, &look.stash_drawn.shown, &mut look.textures);
                        look.name_font = stash.name.and_then(|name| stash.panel.nodes[name].text.as_ref()).and_then(|text| {
                            let file = stash.panel.file_of(text.font.0)?;
                            trp_data::screen::TmpFont::read(install, &file, text.font.1).ok()
                        });
                        look.stash = Some(stash);
                    }
                    Err(e) => eprintln!("the game's panel for a container opened could not be read; the app's is drawn: {e:#}"),
                }
                // The quick-use panel, every view on as its script shows
                // them (`InventoryScreenQuickAccessPanel.Show`).
                match trp_data::screen::QuickViews::load(install, file) {
                    Ok(mut quick) => {
                        for slot in quick.slots.clone() {
                            quick.panel.nodes[slot.object].active = true;
                        }
                        look.quick_drawn.shown = shown_of(&quick.panel.nodes, &[], &[], &[]);
                        look.quick_drawn.pictures = read(&quick.panel, &look.quick_drawn.shown, &mut look.textures);
                        look.quick = Some(quick);
                    }
                    Err(e) => eprintln!("the game's quick-use panel could not be read; the app's is drawn: {e:#}"),
                }
                // The battle screen's panels of the character: the stance's
                // figures shown as its script has them; the speed's notch
                // made by the script (`battle_view`); the effects' icon
                // made for each effect.
                look.battle_modes = crate::battle_view::BattleModes::read(install);
                match trp_data::screen::BattleViews::load(install, file) {
                    Ok(views) => {
                        if let Some(why) = &views.action_missing {
                            eprintln!("the game's interaction panel could not be read; the app's prompt is drawn: {why}");
                        }
                        // (And the mount's icon and arrows, which its script
                        // turns on: `battle_view`.)
                        let figures: Vec<usize> = views.figures.iter().map(|(object, _)| *object).chain(views.mount.iter().flat_map(|mount| mount.scripted())).collect();
                        look.stance_drawn.shown = shown_of(&views.stance.nodes, &[], &[], &figures);
                        look.stance_drawn.pictures = read(&views.stance, &look.stance_drawn.shown, &mut look.textures);
                        look.body_drawn.shown = shown_of(&views.health.nodes, &[], &[], &[]);
                        look.body_drawn.pictures = read(&views.health, &look.body_drawn.shown, &mut look.textures);
                        look.battle = Some(views);
                    }
                    Err(e) => eprintln!("the game's panels of the stance and of the health in a raid could not be read; the app's meters are drawn: {e:#}"),
                }
                match trp_data::screen::QuickViews::load_battle(install, file) {
                    Ok(quick) => {
                        look.battle_quick_drawn.shown = shown_of(&quick.panel.nodes, &[], &[], &[]);
                        look.battle_quick_drawn.pictures = read(&quick.panel, &look.battle_quick_drawn.shown, &mut look.textures);
                        look.battle_quick = Some(quick);
                    }
                    Err(e) => eprintln!("the game's quick bar in a raid could not be read; none is drawn: {e:#}"),
                }
                // The views of a thing, in a slot and in a grid.
                match trp_data::screen::ItemViews::load(install) {
                    Ok(items) => {
                        for (view, drawn) in [(&items.slot, &mut look.slot_item), (&items.grid, &mut look.grid_item)] {
                            if let Some(view) = view {
                                // What the views' scripts turn on for a
                                // weapon (`ValidateMastering`) and a part
                                // (`ItemViewStats`): their pictures read.
                                let named = |name: &str| (0..view.nodes.len()).filter(|&index| view.nodes[index].name == name).collect::<Vec<_>>();
                                let mut turned: Vec<usize> =
                                    ["MasteringContainer", "MasteringIcon", "Level", "ModIconContainer", "WishlistGlow", "WishlistBorder", "WishlistBorders", "WishlistIcon", "Loader"].iter().flat_map(|name| named(name)).collect();
                                turned.extend(named("Image").into_iter().filter(|&index| view.nodes[index].parent.is_some_and(|parent| view.nodes[parent].name == "MasteringIcon")));
                                drawn.shown = shown_of(&view.nodes, &turned, &[], &[]);
                                drawn.pictures = read(view, &drawn.shown, &mut look.textures);
                            }
                        }
                        look.items = Some(items);
                    }
                    Err(e) => eprintln!("the game's views of a thing could not be read; things are drawn the app's way: {e:#}"),
                }
                // The load component over a magazine's view, all of it.
                match trp_data::screen::load_ammo_component(install) {
                    Ok(panel) => {
                        let all: Vec<usize> = (0..panel.nodes.len()).collect();
                        look.spin_drawn.shown = shown_of(&panel.nodes, &all, &[], &[]);
                        look.spin_drawn.pictures = read(&panel, &look.spin_drawn.shown, &mut look.textures);
                        look.spin = Some(panel);
                    }
                    Err(e) => eprintln!("the game's load component of a magazine could not be read; the app's is drawn: {e:#}"),
                }
                // The window that shows a thing, as its script leaves it
                // for a thing with no tag (`InfoWindow.UpdateTag`), not on
                // the wish list (`_wishlistIcon`) and with its preview drawn
                // (its loader gone); the panel of the parts on it off
                // (`_modsPanel`, on for a thing with slots: not built).
                match trp_data::screen::info_window(install) {
                    Ok(mut info) => {
                        let off: Vec<usize> = INFO_OFF.iter().filter_map(|path| info.find(path)).collect();
                        for &index in &off {
                            info.nodes[index].active = false;
                        }
                        // (Its loader while the model is made, and a
                        // weapon's durability row: as its script shows
                        // them.)
                        let scripted: Vec<usize> = [INFO_TAG, INFO_WISH, INFO_LOADER, INFO_DURABILITY, INFO_MODS, INFO_MASTERING, INFO_MASTERING_BADGE, INFO_CORNER].iter().filter_map(|path| info.find(path)).collect();
                        if let Some(wish) = info.find(INFO_WISH) {
                            info.nodes[wish].active = false;
                        }
                        // (The buttons' templates and the mastering row's
                        // value read as on, for the copies made of them.)
                        let turned: Vec<usize> = scripted.iter().copied().chain(INFO_BUTTON_TEMPLATES.iter().chain([&INFO_MASTERING_VALUE]).filter_map(|path| info.find(path))).collect();
                        look.info_drawn.shown = shown_of(&info.nodes, &turned, &off, &scripted);
                        look.info_stretch = trp_data::screen::stretch_limit(&info);
                        look.info_drawn.pictures = read(&info, &look.info_drawn.shown, &mut look.textures);
                        look.info_mass_font = info.find(INFO_MASS).and_then(|mass| info.nodes[mass].text.as_ref()).and_then(|text| trp_data::screen::TmpFont::read(install, &info.file_of(text.font.0)?, text.font.1).ok());
                        // Its rows' templates: each whole, its value on
                        // (`SetValues`), a bar's comparison never.
                        match trp_data::screen::info_rows(install, &info) {
                            Ok(rows) => {
                                let mut drawn_of = |panel: &trp_data::screen::ScenePanel| {
                                    let turned: Vec<usize> = panel.find("Value").into_iter().collect();
                                    let never: Vec<usize> = panel.find("bars/CompareDelta").into_iter().collect();
                                    let mut drawn = Drawn { shown: shown_of(&panel.nodes, &turned, &never, &[]), pictures: Vec::new() };
                                    drawn.pictures = read(panel, &drawn.shown, &mut look.textures);
                                    drawn
                                };
                                let (bar, compact, dropdown) = (drawn_of(&rows.bar), drawn_of(&rows.compact), drawn_of(&rows.dropdown));
                                let bar_look = trp_data::screen::row_look(&rows.bar, "CharacteristicPanel").unwrap_or_default();
                                let compact_look = trp_data::screen::row_look(&rows.compact, "CompactCharacteristicPanel").unwrap_or_default();
                                // A slot's pieces that its script turns on
                                // and off (`SetSlotGraphics`, `Show`); a
                                // revolver's drum's border never here.
                                let mod_slot = rows.mod_slot.map(|panel| {
                                    let scripted: Vec<usize> = MOD_SLOT_PIECES.iter().filter_map(|path| panel.find(path)).collect();
                                    let never: Vec<usize> = panel.find("CanvasGroup/ActiveCamoraBorder").into_iter().collect();
                                    let mut drawn = Drawn { shown: shown_of(&panel.nodes, &[], &never, &scripted), pictures: Vec::new() };
                                    drawn.pictures = read(&panel, &drawn.shown, &mut look.textures);
                                    (panel, drawn)
                                });
                                look.info_rows = Some(InfoRowsLook { bar: (rows.bar, bar), compact: (rows.compact, compact), dropdown: (rows.dropdown, dropdown), bar_look, compact_look, add: None, sub: None, mod_slot });
                            }
                            Err(e) => eprintln!("the game's rows of a thing's figures could not be read; none are listed: {e:#}"),
                        }
                        look.info = Some(info);
                    }
                    Err(e) => eprintln!("the game's window that shows a thing could not be read; the app's is drawn: {e:#}"),
                }
                // The window of a container's grids, as its script leaves it
                // in a raid with the install's `PriorityWindowMode` (Auto):
                // its priority toggle off (`GridWindow.Show`), as are its
                // tag, its sorting, its search and its loader in the prefab;
                // so its caption's row holds the icon, the caption and the
                // cross. The icon and the cross ask the row for their
                // pictures' own sizes where their elements set none (an
                // `Image`'s preferred size, which the panel's layout does
                // not have of itself). PROVISIONAL: a picture's pixel taken
                // as a unit (the sprite's `pixelsPerUnit` against the
                // canvas's not read); the icon is then 19 wide, the words
                // from 24.
                match trp_data::screen::grid_window(install) {
                    Ok(mut window) => {
                        if let Some(priority) = window.priority {
                            window.panel.nodes[priority].active = false;
                        }
                        look.grid_window_drawn.shown = shown_of(&window.panel.nodes, &[], &[], &[]);
                        look.grid_window_drawn.pictures = read(&window.panel, &look.grid_window_drawn.shown, &mut look.textures);
                        for node in [window.icon, window.close] {
                            let Some(picture) = look.grid_window_drawn.pictures[node] else { continue };
                            let element = window.panel.nodes[node].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0; 2], preferred: [-1.0; 2], flexible: [-1.0; 2] });
                            let preferred = [0, 1].map(|axis| if element.preferred[axis] >= 0.0 { element.preferred[axis] } else { picture.size[axis] });
                            window.panel.nodes[node].element = Some(trp_data::screen::Element { preferred, ..element });
                        }
                        look.grid_window = Some(window);
                    }
                    Err(e) => eprintln!("the game's window of a container's grids could not be read; the app's is drawn: {e:#}"),
                }
                // The window that edits a tag: the seven cells its colours'
                // panel makes from its template (`TagColorsPanel.Show`),
                // drawn as the prefab's own seven of the same colours lie
                // (where its grid puts them: 25 by 25, 15 apart, one row),
                // each cell's border, the field's text and its placeholder
                // left to its script.
                match trp_data::screen::tag_window(install) {
                    Ok(mut panel) => {
                        let cells: Vec<usize> = (0..TAG_COLORS).filter_map(|n| panel.find(&tag_cell(n))).collect();
                        for &cell in &cells {
                            panel.nodes[cell].active = true;
                        }
                        let mut scripted: Vec<usize> = (0..TAG_COLORS).filter_map(|n| panel.find(&format!("{}/Border", tag_cell(n)))).collect();
                        scripted.extend([TAG_TEXT, TAG_PLACEHOLDER].iter().filter_map(|path| panel.find(path)));
                        look.tag_limit = panel.find(TAG_INPUT).and_then(|input| panel.script(input, "ValidationInputField")).and_then(trp_data::screen::input_limit).unwrap_or(TAG_LIMIT);
                        // The field's text and placeholder kept off its edges
                        // by their margins (left, top, right, bottom), put
                        // into their rectangles: the panel's drawing lays a
                        // text in its rectangle as it is.
                        let fields: Vec<usize> = [TAG_TEXT, TAG_PLACEHOLDER].iter().filter_map(|path| panel.find(path)).collect();
                        for index in fields {
                            let Some(margin) = panel.nodes[index].text.as_ref().map(|text| text.margin) else { continue };
                            let layout = &mut panel.nodes[index].layout;
                            layout.offset = [layout.offset[0] + margin[0], layout.offset[1] - margin[1]];
                            layout.size_delta = [layout.size_delta[0] - margin[0] - margin[2], layout.size_delta[1] - margin[1] - margin[3]];
                            if let Some(text) = panel.nodes[index].text.as_mut() {
                                text.margin = [0.0; 4];
                            }
                        }
                        look.tag_window_drawn.shown = shown_of(&panel.nodes, &[], &[], &scripted);
                        look.tag_window_drawn.pictures = read(&panel, &look.tag_window_drawn.shown, &mut look.textures);
                        look.tag_window = Some(panel);
                    }
                    Err(e) => eprintln!("the game's window that edits a tag could not be read; the app's is drawn: {e:#}"),
                }
                // The menu of a right click, its templates of a line on.
                match trp_data::screen::context_menu(install) {
                    Ok(menu) => {
                        let templates: Vec<usize> = MENU_TEMPLATES.iter().filter_map(|path| menu.find(path)).collect();
                        look.menu_drawn.shown = shown_of(&menu.nodes, &templates, &[], &[]);
                        look.menu_drawn.pictures = read(&menu, &look.menu_drawn.shown, &mut look.textures);
                        let caption = menu.find(&format!("{}/Text", MENU_TEMPLATES[0])).and_then(|text| menu.nodes[text].text.as_ref());
                        look.menu_font = caption.and_then(|text| trp_data::screen::TmpFont::read(install, &menu.file_of(text.font.0)?, text.font.1).ok());
                        look.menu = Some(menu);
                    }
                    Err(e) => eprintln!("the game's menu of a right click could not be read; the app's is drawn: {e:#}"),
                }
                // The menu of a line's own lines, from the same templates.
                match trp_data::screen::context_submenu(install) {
                    Ok(mut sub) => {
                        // Its holder of lines is kept off, and shown with
                        // the menu (`ShowMenu`: the holder's `Show`).
                        if let Some(holder) = sub.find(MENU_LINES) {
                            sub.nodes[holder].active = true;
                        }
                        let templates: Vec<usize> = MENU_TEMPLATES.iter().filter_map(|path| sub.find(path)).collect();
                        look.submenu_drawn.shown = shown_of(&sub.nodes, &templates, &[], &[]);
                        look.submenu_drawn.pictures = read(&sub, &look.submenu_drawn.shown, &mut look.textures);
                        look.submenu = Some(sub);
                    }
                    Err(e) => eprintln!("the game's menu of a line's own lines could not be read; the app's is drawn: {e:#}"),
                }
                // The OVERALL tab, as it shows in a raid: its clothing panel
                // on where the options' `CLOTHING` is (the game's only out of
                // one, `ShowClothingPanel`: the user's departure), its lists'
                // scroll areas as high as they show of their lines (at most
                // `_maxVisibleHeight`: the install's suits are many more
                // than that holds, `DropDownBox.Update`), its back button
                // and its lines' template never (the inventory's own back
                // button is the tab bar's; the lines are drawn from the
                // template, `line`), its lists, their arrows and the
                // loading spinner as its scripts have them.
                if let Some(overall) = overall_panel(install, file, clothing) {
                    let panel = &overall.panel;
                    let never: Vec<usize> = ["BackButton"].into_iter().chain(OVERALL_SELECTORS.map(|selector| format!("{selector}/Element")).iter().map(String::as_str).collect::<Vec<_>>()).filter_map(|path| panel.find(path)).collect();
                    let mut scripted: Vec<usize> = OVERALL_SELECTORS.iter().flat_map(|selector| ["Open Panel", "Button/Closed Image", "Button/Open Image"].map(|end| format!("{selector}/{end}"))).filter_map(|path| panel.find(&path)).collect();
                    scripted.extend(panel.find(&format!("{OVERALL_VIEW}/Progress Spinner")));
                    look.overall_drawn.shown = shown_of(&panel.nodes, &scripted, &never, &scripted);
                    look.overall_drawn.pictures = read(panel, &look.overall_drawn.shown, &mut look.textures);
                    if let Some(line) = &overall.line {
                        look.line_drawn.shown = shown_of(&line.nodes, &[], &[], &[]);
                        look.line_drawn.pictures = read(line, &look.line_drawn.shown, &mut look.textures);
                    }
                    look.overall = Some(overall);
                }
                // The edit build screen, with the workbench on.
                let mut edit_build = None;
                if workbench {
                    match edit_build_panels(install) {
                        Ok((scene, slot, empty, part)) => {
                            let panel = &scene.panel;
                            let find = |path: &str| panel.find(path);
                            let turned: Vec<usize> = [EB_DROPDOWN, EB_READY].iter().filter_map(|path| find(path)).collect();
                            let never: Vec<usize> = EB_NEVER.iter().filter_map(|path| find(path)).collect();
                            let mut scripted = turned.clone();
                            scripted.extend(find(EB_LOADER));
                            scripted.extend((0..panel.nodes.len()).filter(|&index| panel.nodes[index].path.starts_with("Toggle Group/") && panel.nodes[index].path.ends_with("/Checkmark/Mark")));
                            let mut screen = Drawn { shown: shown_of(&panel.nodes, &turned, &never, &scripted), pictures: Vec::new() };
                            screen.pictures = read(panel, &screen.shown, &mut look.textures);
                            let slot_turned: Vec<usize> = [EB_SLOT_BUTTON].iter().filter_map(|path| slot.find(path)).collect();
                            let slot_scripted: Vec<usize> = [EB_SLOT_BUTTON, EB_SLOT_NAME, EB_SLOT_FADE].iter().filter_map(|path| slot.find(path)).collect();
                            let mut slot_drawn = Drawn { shown: shown_of(&slot.nodes, &slot_turned, &[], &slot_scripted), pictures: Vec::new() };
                            slot_drawn.pictures = read(&slot, &slot_drawn.shown, &mut look.textures);
                            let mut empty_drawn = Drawn { shown: shown_of(&empty.nodes, &[], &[], &[]), pictures: Vec::new() };
                            empty_drawn.pictures = read(&empty, &empty_drawn.shown, &mut look.textures);
                            // A part's view as the grid's is read: what its
                            // scripts turn on for a part.
                            let named = |name: &str| (0..part.nodes.len()).filter(|&index| part.nodes[index].name == name).collect::<Vec<_>>();
                            let part_turned: Vec<usize> = ["ModIconContainer", "Loader"].iter().flat_map(|name| named(name)).collect();
                            let mut part_drawn = Drawn { shown: shown_of(&part.nodes, &part_turned, &[], &[]), pictures: Vec::new() };
                            part_drawn.pictures = read(&part, &part_drawn.shown, &mut look.textures);
                            // The characteristics panel: its templates and
                            // the mastering row never drawn in it, the
                            // expanded rows and the INFO button by the
                            // fold, each bar's comparison by its figures.
                            let stats = panel.find("CharacteristicsPanel").and_then(|top| {
                                let sub = panel.subtree(top);
                                let find = |path: &str| sub.find(path);
                                let template = |path: &str| find(path).map(|at| sub.subtree(at));
                                let (bar, compact) = (template(EB_CHAR_TEMPLATE)?, template(EB_COMPACT_TEMPLATE)?);
                                let never: Vec<usize> = [EB_CHAR_TEMPLATE, EB_COMPACT_TEMPLATE, "CharacteristicsExpanded/Mastering"].iter().filter_map(|path| find(path)).collect();
                                let scripted: Vec<usize> = ["CharacteristicsExpanded", "InfoButton", "CharacteristicsExpanded/Durability/bars/CompareDelta", "CharacteristicsExpanded/Weight/bars/CompareDelta"].iter().filter_map(|path| find(path)).collect();
                                let mut drawn = Drawn { shown: shown_of(&sub.nodes, &scripted, &never, &scripted), pictures: Vec::new() };
                                drawn.pictures = read(&sub, &drawn.shown, &mut look.textures);
                                let mut row_drawn = |row: &trp_data::screen::ScenePanel| {
                                    let turned: Vec<usize> = row.find("Value").into_iter().collect();
                                    let scripted: Vec<usize> = row.find("bars/CompareDelta").into_iter().collect();
                                    let mut drawn = Drawn { shown: shown_of(&row.nodes, &turned, &[], &scripted), pictures: Vec::new() };
                                    drawn.pictures = read(row, &drawn.shown, &mut look.textures);
                                    drawn
                                };
                                let (bar_drawn, compact_drawn) = (row_drawn(&bar), row_drawn(&compact));
                                let bar_look = trp_data::screen::row_look(&bar, "CharacteristicPanel").unwrap_or_default();
                                let compact_look = trp_data::screen::row_look(&compact, "CompactCharacteristicPanel").unwrap_or_default();
                                let weight_look = find("CharacteristicsExpanded/Weight").and_then(|row| trp_data::screen::row_look(&sub.subtree(row), "CharacteristicPanel")).unwrap_or_else(|| bar_look.clone());
                                Some(EbStatsLook { panel: sub, drawn, bar: (bar, bar_drawn), compact: (compact, compact_drawn), bar_look, compact_look, weight_look })
                            });
                            edit_build = Some(EditBuildLook {
                                scene: std::sync::Arc::new(scene),
                                screen,
                                slot,
                                slot_drawn,
                                empty,
                                empty_drawn,
                                part: Some((part, part_drawn)),
                                part_look: None,
                                bones: HashMap::new(),
                                dropdown: [None, None],
                                stats,
                            });
                        }
                        Err(e) => eprintln!("the game's edit build screen could not be read; the app's look of it is drawn: {e:#}"),
                    }
                }
                drop(read);
                // The edit build screen's pictures its scripts put on it.
                if let Some(screen) = edit_build.as_mut() {
                    for name in EB_BONES {
                        let Some((file, id)) = trp_data::screen::resource_sprite(install, &format!("Modding Screen Icons/{name}")) else { continue };
                        if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                            screen.bones.insert(name.to_owned(), picture);
                        }
                    }
                    for (at, name) in ["button_mod_dropdown_open", "button_mod_dropdown_close"].iter().enumerate() {
                        screen.dropdown[at] = trp_data::screen::resource_sprite(install, name).and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures));
                    }
                }
                look.edit_build = edit_build;
                // The SKILLS tab: its page, a skill's two views, the
                // tooltip, and the pictures its scripts put on them.
                look.skills = crate::skills_view::SkillsLook::read(install, file, &mut reader, &mut look.textures);
                // The inspect window's bar's pictures and the attributes'
                // icons.
                if let Some(rows) = look.info_rows.as_mut() {
                    let figures = rows.bar_look.bar.clone().unwrap_or_default();
                    rows.add = figures.add_sprite.and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures));
                    rows.sub = figures.sub_sprite.and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures));
                }
                // The armour classes' marks (`Sprite Assets/armor_classes`).
                match trp_data::screen::tmp_sprite_names(install, "Sprite Assets/armor_classes") {
                    Ok(marks) => {
                        for (name, (file, id)) in marks {
                            let Some(class) = name.strip_prefix("armor_classes_").and_then(|class| class.parse::<u32>().ok()) else { continue };
                            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                look.class_marks.insert(class, picture);
                            }
                        }
                    }
                    Err(e) => eprintln!("the game's marks of the armour classes could not be read; a tooltip shows none: {e:#}"),
                }
                // The slots' pictures, by their names.
                match trp_data::screen::resource_sprites(install, "Slots/") {
                    Ok(sprites) => {
                        for (name, (file, id)) in sprites.into_iter().filter(|(name, _)| !name.contains('/')) {
                            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                look.slot_pictures.insert(name, picture);
                            }
                        }
                    }
                    Err(e) => eprintln!("the game's pictures of the slots could not be read; an empty slot of a thing's parts shows none: {e:#}"),
                }
                match trp_data::screen::attribute_icons(install) {
                    Ok(icons) => {
                        for (name, (file, id)) in icons {
                            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                look.attribute_icons.insert(name, picture);
                            }
                        }
                    }
                    Err(e) => eprintln!("the game's icons of a thing's figures could not be read; its rows have none: {e:#}"),
                }
                match trp_data::screen::figure_icons(install) {
                    Ok(tables) => {
                        for (number, table) in tables.into_iter().enumerate() {
                            for (name, (file, id)) in table {
                                let name = if number == 2 { trp_sim::health::BuffKind::of_name(&name).name().to_owned() } else { name };
                                if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                    look.figure_icons[number].insert(name, picture);
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("the game's icons of medicine's figures could not be read; their rows have none: {e:#}"),
                }
                // The pictures the OVERALL tab's scripts put on it: the
                // level's rank, the side's (the BEAR's, the side of the
                // character's starting kit, `PROVISIONAL`) and the
                // prestige's (none: `_noSprite`).
                if let Some(overall) = &look.overall {
                    let [bear, _, none] = overall.side_sprites.clone();
                    let rank = trp_data::screen::resource_sprite(install, OVERALL_RANK);
                    for (name, sprite) in [("rank", rank), ("side", bear), ("prestige", none)] {
                        let Some((file, id)) = sprite else { continue };
                        if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                            look.overall_pictures.insert(name, picture);
                        }
                    }
                }
                // The tabs' fills and the crate panel's stripes cut to their
                // masks, from the pictures' pixels.
                let mut decoded = HashMap::new();
                if let Some(bar) = &look.tab_bar {
                    look.tab_fills = masked_fills(&bar.panel, &look.tabs_drawn, &look.textures, &mut decoded);
                }
                if let Some(stash) = &look.stash {
                    look.stash_fills = masked_fills(&stash.panel, &look.stash_drawn, &look.textures, &mut decoded);
                }
                // Each container slot's empty picture (`SearchableSlotView.
                // Show`: none for the pockets).
                for slot in ["TacticalVest", "Backpack", "SecuredContainer"] {
                    let Some((file, id)) = trp_data::screen::resource_sprite(install, &format!("Slots/Equipment/{slot}")) else { continue };
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.container_backs.insert(slot.to_owned(), picture);
                    }
                }
                // A part's marks (`ItemViewFactory.LoadModClassIconSprite`,
                // `LoadModIconSprite`): its class's and its kind's.
                let marks = MOD_CLASSES.iter().map(|(_, name)| (format!("type_{name}"), format!("Mod Types/icon_type_mod_{name}")));
                let kinds = MOD_KINDS.iter().map(|(_, name)| (format!("mod_{name}"), format!("Mod Tactical Types/icon_mod_{name}")));
                // And a plate's armour class's, which a plate that may be
                // taken out shows instead (`ItemViewStats.
                // GetArmorPlateClassIconName`).
                let plates = (1..=6).map(|class| (format!("plate_{class}"), format!("Mod Types/icon_type_mod_armor_plate_{class}")));
                for (key, path) in marks.chain(kinds).chain(plates) {
                    let Some((file, id)) = trp_data::screen::resource_sprite(install, &path) else { continue };
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.mod_pictures.insert(key, picture);
                    }
                }
                // The sprites the stance panel's script puts on it.
                if let Some(views) = &look.battle {
                    let wanted: Vec<(i32, i64)> = views.fill_sprites.iter().flatten().chain(&views.noise_sprites).copied().collect();
                    for (outside, id) in wanted {
                        let Some(name) = views.stance.file_of(outside) else { continue };
                        if let Some(picture) = reader.sprite(&name, id, &mut look.textures) {
                            look.battle_sprites.insert((outside, id), picture);
                        }
                    }
                    // The interaction panel's pointer: the hand, the hand
                    // refused, the sense's dot (`UIPointer`).
                    if let Some(action) = &views.action {
                        for (outside, id) in [action.hover_sprite, action.unavailable_sprite, action.sense_sprite].into_iter().flatten() {
                            let Some(name) = action.panel.file_of(outside) else { continue };
                            if let Some(picture) = reader.sprite(&name, id, &mut look.textures) {
                                look.battle_sprites.insert((outside, id), picture);
                            }
                        }
                    }
                }
                // The icons of the menu's lines (`InteractionButtonsContainer`:
                // `Characteristics/Icons/<line>`).
                for line in trp_sim::item_menu::Line::ALL {
                    let Some((file, id)) = trp_data::screen::resource_sprite(install, &format!("Characteristics/Icons/{}", line.name())) else { continue };
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.menu_icons.insert(line.name(), picture);
                    }
                }
                // The wish list's: Remove's as the menu's, the groups' from
                // the static icons, and the outlined ones of a view.
                if let Some((file, id)) = trp_data::screen::resource_sprite(install, "Characteristics/Icons/RemoveFromWishlist") {
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.menu_icons.insert("RemoveFromWishlist", picture);
                    }
                }
                match trp_data::screen::wishlist_icons(install) {
                    Ok([plain, outlined]) => {
                        for (name, (file, id)) in plain {
                            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                look.menu_icons.insert(name, picture);
                                look.wish_icons.insert(name, picture);
                            }
                        }
                        for (name, (file, id)) in outlined {
                            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                look.wish_outlines.insert(name, picture);
                            }
                        }
                    }
                    Err(e) => eprintln!("the game's icons of the wish list could not be read; its lines and marks have none: {e:#}"),
                }
                // The repair window, as its scripts leave it for a kit: its
                // warnings' templates copied as they are needed, its loader,
                // its repair's progress, the rank and standing (a trader's),
                // the slider shown by its script.
                match trp_data::screen::repair_window(install) {
                    Ok(panel) => {
                        let scripted: Vec<usize> = REPAIR_SCRIPTED.iter().filter_map(|path| panel.find(path)).collect();
                        let turned: Vec<usize> = REPAIR_TEMPLATES.iter().filter_map(|path| panel.find(path)).collect();
                        let by_script: Vec<usize> = scripted.iter().chain(&turned).copied().collect();
                        look.repair_drawn.shown = shown_of(&panel.nodes, &turned, &[], &by_script);
                        look.repair_drawn.pictures = reader.panel(&panel, &look.repair_drawn.shown, file, &mut look.textures);
                        look.repair_figures = trp_data::screen::repair_figures(&panel).unwrap_or_else(|e| {
                            eprintln!("the repair window's scripts could not be read; its kit's icons and colours are the prefab's: {e:#}");
                            trp_data::screen::RepairFigures::default()
                        });
                        let figures = look.repair_figures.clone();
                        look.repair_icons = [figures.in_stash, figures.to_pay].map(|sprite| sprite.and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures)));
                        look.repair = Some(panel);
                    }
                    Err(e) => eprintln!("the game's repair window could not be read; the app's is drawn: {e:#}"),
                }
                // The health's effects' icons (`StaticIcons.EffectIcons`).
                match trp_data::screen::effect_icons(install) {
                    Ok(icons) => {
                        for (name, (file, id)) in icons {
                            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                                look.effect_pictures.insert(name, picture);
                            }
                        }
                    }
                    Err(e) => eprintln!("the game's icons of the health's effects could not be read; the effects are written in words: {e:#}"),
                }
                if !reader.missing.is_empty() {
                    eprintln!("the inventory's pictures of {} could not be read; they are left out", reader.missing.join(", "));
                }
                // The fonts its texts are drawn in: each font asset's own
                // font, kept whole in the build's file beside it.
                let items = look.items.as_ref().map_or([None, None], |items| [items.slot.as_ref(), items.grid.as_ref()]);
                let containers = look.containers.as_ref().map(|views| &views.panel);
                let tabs = look.tab_bar.as_ref().map(|bar| &bar.panel);
                let stash = look.stash.as_ref().map(|stash| &stash.panel);
                let quick = look.quick.as_ref().map(|quick| &quick.panel);
                let back = look.tab_bar.as_ref().and_then(|bar| bar.back.as_ref());
                let battle = look.battle.as_ref().map_or([None, None], |views| [Some(&views.stance), Some(&views.health)]);
                let battle_quick = look.battle_quick.as_ref().map(|quick| &quick.panel);
                let info = look.info.as_ref();
                let menu = look.menu.as_ref();
                let tag_window = look.tag_window.as_ref();
                let submenu = look.submenu.as_ref();
                let overall = look.overall.as_ref().map(|overall| &overall.panel);
                let overall_line = look.overall.as_ref().and_then(|overall| overall.line.as_ref());
                let edit_build = look.edit_build.as_ref().map_or([None, None, None, None], |eb| [Some(&eb.scene.panel), Some(&eb.slot), Some(&eb.empty), eb.part.as_ref().map(|(part, _)| part)]);
                let info_rows = look.info_rows.as_ref().map_or([None, None, None, None], |rows| [Some(&rows.bar.0), Some(&rows.compact.0), Some(&rows.dropdown.0), rows.mod_slot.as_ref().map(|(panel, _)| panel)]);
                let action = look.battle.as_ref().and_then(|views| views.action.as_ref()).map(|action| &action.panel);
                let grid_window = look.grid_window.as_ref().map(|window| &window.panel);
                let repair_window = look.repair.as_ref();
                match trp_data::screen::version_label(install) {
                    Ok(version) => look.version = Some(version),
                    Err(e) => eprintln!("the game's line of its version could not be read; none is drawn: {e:#}"),
                }
                match trp_data::screen::simple_tooltip(install) {
                    Ok(tooltip) => look.tooltip = Some(tooltip),
                    Err(e) => eprintln!("the game's tooltip could not be read; the app's look is used for it: {e:#}"),
                }
                let version = look.version.as_ref().map(|(label, _)| label).or(look.tooltip.as_ref());
                let tooltip = look.tooltip.as_ref();
                let skill_panels: Vec<&trp_data::screen::ScenePanel> = look.skills.as_ref().map(|skills| skills.panels()).unwrap_or_default();
                let mut assets: std::collections::BTreeSet<(String, i64)> = [Some(&panel.panel), panel.strip.as_ref(), panel.foot.as_ref(), panel.gear_panel.as_ref(), items[0], items[1], containers, tabs, stash, quick, back, version, tooltip, battle[0], battle[1], battle_quick, info, menu, tag_window, submenu, overall, overall_line, edit_build[0], edit_build[1], edit_build[2], edit_build[3], info_rows[0], info_rows[1], info_rows[2], info_rows[3], action, grid_window, repair_window]
                    .into_iter()
                    .flatten()
                    .chain(skill_panels.iter().copied())
                    .flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| Some((scene.file_of(text.font.0)?, text.font.1)))))
                    .collect();
                // The materials the texts name for themselves, where they do.
                let text_materials: std::collections::BTreeSet<(String, i64)> = [Some(&panel.panel), panel.strip.as_ref(), panel.foot.as_ref(), panel.gear_panel.as_ref(), items[0], items[1], containers, tabs, stash, quick, back, version, tooltip, battle[0], battle[1], battle_quick, info, menu, tag_window, submenu, overall, overall_line, edit_build[0], edit_build[1], edit_build[2], edit_build[3], info_rows[0], info_rows[1], info_rows[2], info_rows[3], action, grid_window, repair_window]
                    .into_iter()
                    .flatten()
                    .chain(skill_panels.iter().copied())
                    .flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| text.material).and_then(|(file, id)| Some((scene.file_of(file)?, id)))))
                    .collect();
                // And each one's bold typeface, where it has one.
                for (name, id) in assets.clone() {
                    if let Some(bold) = trp_data::screen::TmpFont::read(install, &name, id).ok().and_then(|font| font.bold) {
                        look.bold_fonts.insert((name, id), bold.clone());
                        assets.insert(bold);
                    }
                }
                for (name, id) in &assets {
                    if let Some(effect) = TextEffect::of(install, name, *id) {
                        look.text_effects.insert((name.clone(), *id), effect);
                    }
                }
                let mut kept_fonts: HashMap<String, Vec<(String, Vec<u8>)>> = HashMap::new();
                let mut tmp_built: HashMap<(String, i64), trp_data::tmp_text::TmpFace> = HashMap::new();
                for (name, id) in assets {
                    if !kept_fonts.contains_key(&name) {
                        let read = trp_data::screen::build_fonts(install, &name).unwrap_or_else(|e| {
                            eprintln!("the game's fonts could not be read from {name}; the app's is used: {e:#}");
                            Vec::new()
                        });
                        kept_fonts.insert(name.clone(), read);
                    }
                    let asset = trp_data::screen::script_name(install, &name, id).unwrap_or_default();
                    let by_face = || trp_data::screen::TmpFont::read(install, &name, id).ok().and_then(|face| trp_data::screen::font_by_face(&face.family, &face.style, &kept_fonts[&name]));
                    let font_file = trp_data::screen::font_for(&asset, &kept_fonts[&name]).or_else(by_face).map(|(_, font)| font.clone());
                    match &font_file {
                        Some(font) => look.font_data.push(((name.clone(), id), font.clone())),
                        None => eprintln!("no font of the game's is had for its font asset {asset:?}; the app's is used"),
                    }
                    // The asset as the game draws from it: its glyphs, its
                    // atlas, its font's kerning, its own material.
                    let face = trp_data::screen::TmpFont::read(install, &name, id).and_then(|font| {
                        let (atlas_file, atlas_id) = anyhow::Context::context(font.atlases.first().cloned(), "the font asset has no atlas")?;
                        let atlas = pictures.texture(&atlas_file, atlas_id)?;
                        Ok(trp_data::tmp_text::TmpFace::new(font, &atlas, font_file.as_deref()))
                    });
                    match face {
                        Ok(face) => {
                            tmp_built.insert((name.clone(), id), face);
                        }
                        Err(e) => eprintln!("the font asset {asset:?} could not be read to draw from; its texts are drawn the app's way: {e:#}"),
                    }
                    if let Ok(material) = trp_data::screen::font_material(install, &name, id) {
                        look.tmp_default_material.insert((name.clone(), id), material);
                    }
                }
                // Each asset with its bold typeface's face, where it has one.
                let bolds: std::collections::BTreeSet<(String, i64)> = tmp_built.values().filter_map(|face| face.font.bold.clone()).collect();
                for key in bolds {
                    if let Some(face) = tmp_built.remove(&key) {
                        look.tmp_faces.insert(key, std::sync::Arc::new(face));
                    }
                }
                for (key, mut face) in tmp_built {
                    face.bold = face.font.bold.as_ref().and_then(|bold| look.tmp_faces.get(bold)).cloned();
                    look.tmp_faces.insert(key, std::sync::Arc::new(face));
                }
                // What the tag window's texts and its grid of colours ask of
                // its layout (TextMeshPro's preferred size; the grid's one
                // row), which the layout here does not ask of them itself.
                if let Some(mut panel) = look.tag_window.take() {
                    let asks = |panel: &trp_data::screen::ScenePanel, index: usize, words: &str| -> Option<[f32; 2]> {
                        let text = panel.nodes[index].text.as_ref()?;
                        let font = &look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?.font;
                        let width = font.width(words, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
                        Some([width, tmp_line(font, text.size) + text.margin[1] + text.margin[3]])
                    };
                    let element = |preferred: [f32; 2]| trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred, flexible: [-1.0, -1.0] };
                    if let Some(colors) = panel.find(TAG_COLORS_PANEL) {
                        let width = panel.nodes[colors].element.map_or(360.0, |element| element.preferred[0]);
                        panel.nodes[colors].element = Some(trp_data::screen::Element { preferred: [width, TAG_CELL], ..element([width, TAG_CELL]) });
                    }
                    let counter = format!("{0}/{0}", look.tag_limit);
                    if let Some((index, size)) = panel.find(TAG_COUNT).and_then(|index| Some((index, asks(&panel, index, &counter)?))) {
                        panel.nodes[index].element = Some(element(size));
                    }
                    if let Some((index, size)) = panel.find(TAG_SAVE_SIZE).and_then(|index| Some((index, asks(&panel, index, "SAVE")?))) {
                        panel.nodes[index].element = Some(element(size));
                    }
                    look.tag_window = Some(panel);
                }
                // The materials: the assets' own and the texts' own.
                let wanted: std::collections::BTreeSet<(String, i64)> = look.tmp_default_material.values().cloned().chain(text_materials).collect();
                for (file, id) in wanted {
                    match trp_data::screen::material_properties(install, &file, id) {
                        Ok(props) => {
                            look.tmp_materials.insert((file, id), trp_data::tmp_text::TmpMaterial::of(&props));
                        }
                        Err(e) => eprintln!("a text's material could not be read; texts drawn with it are drawn the app's way: {e:#}"),
                    }
                }
                // The scene's clips that the panels' animators play.
                let shared: Vec<String> = panel.panel.externals.iter().map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).filter(|name| name.starts_with("sharedassets")).collect();
                match trp_data::screen::scene_clips(install, &mut pictures, &shared) {
                    Ok(clips) => {
                        use trp_data::unity::anim::{path_hash, Clip};
                        let curve = |clip: &Clip, path: &str, property: &str| clip.others.iter().find(|b| b.path == path_hash(path) && b.attribute == path_hash(property)).map(|b| b.curve);
                        for (_, _, clip) in &clips {
                            if let (Some(alpha), true) = (curve(clip, "BackGlowImage", "m_Color.a"), clip.name == "Normal") {
                                let mut values = Vec::new();
                                clip.sample(0.0, &mut values);
                                look.glow_alpha = values.get(alpha).copied();
                            }
                            if let (Some(alpha), true) = (curve(clip, "BackGlowImage", "m_Color.a"), clip.name == "Highlighted") {
                                look.glow_lit = Some((clip.clone(), alpha));
                            }
                            if let (Some(height), Some(alpha), true) = (curve(clip, "", "m_AnchoredPosition.y"), curve(clip, "", "m_Color.a"), clip.name == "BuffPaneActive") {
                                look.bob = Some((clip.clone(), height, alpha));
                            }
                        }
                    }
                    Err(e) => eprintln!("the health panel's animations could not be read; its glows show and its arrows stand still: {e:#}"),
                }
            }
            Err(e) => eprintln!("the health panel's pictures could not be read; it is drawn without them: {e:#}"),
        }
        look.panel = Some(panel);
        look
    }

    /// The panels' own texts that are keys to the install's words: the
    /// health panel's views' headers, and the slots' names, which the slot
    /// views' headers say (`SlotViewHeader.Show`).
    pub fn keys(&self) -> Vec<String> {
        let Some(game) = &self.panel else { return Vec::new() };
        let headers = game.views.parts.iter().filter_map(|view| game.panel.find(&format!("{}/Header/Text", game.panel.nodes[view.view].path))).filter_map(|text| game.panel.nodes[text].text.as_ref().map(|text| text.text.clone()));
        // And the containers' slots', which their rows' headers say.
        let containers = ["TacticalVest", "Pockets", "Backpack", "SecuredContainer"].map(str::to_owned);
        // And what the panel for a container opened calls what it holds.
        let mut keys: Vec<String> = headers.chain(game.slot_views.iter().map(|(slot, _)| slot.to_string())).chain(containers).chain(["LOOT".to_owned()]).collect();
        // And the OVERALL tab's texts the install's words fill.
        keys.extend(self.overall.iter().flat_map(|overall| overall.localized.iter().map(|(_, key, _)| key.clone())));
        // And the SKILLS tab's.
        keys.extend(self.skills.iter().flat_map(|skills| skills.page.localized.iter().map(|(_, key, _)| key.clone())));
        // And the edit build screen's, its slots' and its empty view's.
        if let Some(eb) = &self.edit_build {
            for panel in [&eb.scene.panel, &eb.slot, &eb.empty] {
                keys.extend((0..panel.nodes.len()).filter_map(|index| trp_data::screen::localized(panel, index)).map(|(key, _)| key));
            }
        }
        keys
    }

    /// The edit build screen as the game draws it, where it was read.
    pub(crate) fn edit_build(&self) -> Option<&EditBuildLook> {
        self.edit_build.as_ref()
    }

    /// One of the game's pictures as drawn by hand, in that colour.
    pub(crate) fn sprite_of(&self, picture: Picture, color: Color) -> Option<Sprite> {
        let image = self.images.get(picture.texture)?.clone()?;
        Some(Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color })
    }

    /// A part's kind's mark (`Mod Tactical Types/icon_mod_<kind>`) by its
    /// kind's name.
    pub(crate) fn mod_mark(&self, kind: &str) -> Option<Picture> {
        self.mod_pictures.get(&format!("mod_{kind}")).copied()
    }

    /// What a panel's pictures and texts ask of the layout groups that lay
    /// them out, put on them where no `LayoutElement` of theirs says
    /// (`Image.preferredWidth`: a simple or filled picture its sprite's
    /// size, a sliced or tiled one its border's; TextMeshPro's preferred
    /// size: its words' width in its font asset and a line's height, with
    /// its margins), so that the panel's groups and fitters lay out as the
    /// game's: the layout here asks nothing of either by itself. `words`:
    /// what the texts say where not what the scene has; an object with a
    /// group of its own asks what its group does.
    pub(crate) fn ask_by_content(&self, panel: &mut trp_data::screen::ScenePanel, drawn: &Drawn, words: &HashMap<usize, String>) {
        self.ask_by_content_under(panel, drawn, words, None);
    }

    /// The same, only for what lies under one object of the panel (itself
    /// among it), where one is named.
    pub(crate) fn ask_by_content_under(&self, panel: &mut trp_data::screen::ScenePanel, drawn: &Drawn, words: &HashMap<usize, String>, top: Option<usize>) {
        let under = |panel: &trp_data::screen::ScenePanel, index: usize| {
            let Some(top) = top else { return true };
            let mut at = Some(index);
            while let Some(here) = at {
                if here == top {
                    return true;
                }
                at = panel.nodes[here].parent;
            }
            false
        };
        for index in 0..panel.nodes.len() {
            if panel.nodes[index].group.is_some() || !under(panel, index) {
                continue;
            }
            let mut asked: Option<[f32; 2]> = None;
            if let (Some(image), Some(Some(picture))) = (panel.nodes[index].image, drawn.pictures.get(index)) {
                let per = image.pixels_per_unit.max(1e-3);
                let [l, b, r, t] = picture.border;
                asked = Some(match image.kind {
                    1 | 2 => [(l + r) / per, (b + t) / per],
                    _ => [picture.size[0] / per, picture.size[1] / per],
                });
            }
            if let Some(text) = panel.nodes[index].text.as_ref() {
                let said = words.get(&index).cloned().unwrap_or_else(|| text.text.clone());
                let said = if text.style & 16 != 0 { said.to_uppercase() } else { said };
                if let Some(face) = panel.file_of(text.font.0).and_then(|file| self.tmp_faces.get(&(file, text.font.1))) {
                    let width = face.font.width(&said, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
                    let height = tmp_line(&face.font, text.size) + text.margin[1] + text.margin[3];
                    asked = Some(asked.map_or([width, height], |own| [own[0].max(width), own[1].max(height)]));
                }
            }
            let Some(asked) = asked else { continue };
            let element = panel.nodes[index].element.get_or_insert(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
            for axis in 0..2 {
                if element.preferred[axis] < 0.0 {
                    element.preferred[axis] = asked[axis];
                }
            }
        }
    }
}

// ==== The edit build and OVERALL panels, the picture reader, what is shown ====

/// The edit build screen's panels from the install: its scene, a slot's
/// view, the empty view its script names, a part's view.
fn edit_build_panels(install: &trp_data::Install) -> anyhow::Result<(trp_data::edit_build::EditBuildScene, trp_data::screen::ScenePanel, trp_data::screen::ScenePanel, trp_data::screen::ScenePanel)> {
    use anyhow::Context;
    let scene = trp_data::edit_build::EditBuildScene::load(install)?;
    let paths = trp_data::screen::resource_paths(install)?;
    let prefab = |wanted: &str| -> anyhow::Result<trp_data::screen::ScenePanel> {
        let (_, file, id) = paths.iter().find(|(path, ..)| path == wanted).with_context(|| format!("no resource is at {wanted}"))?;
        let scene = trp_data::screen::Scene::load_prefab(install, file, *id)?;
        let mut panel = trp_data::screen::ScenePanel::of(&scene, 0);
        panel.file = file.clone();
        Ok(panel)
    };
    let slot = prefab("prefabs/ugui/modding_screen_slot")?;
    let part = prefab("prefabs/ugui/layouts/modding_selectable_layout")?;
    // The empty view: the prefab its template pointer's script is on.
    let fields = slot.script(0, "ModdingScreenSlotView").context("the slot's view has no script")?;
    let pointer = fields.get(EB_EMPTY_FIELD..EB_EMPTY_FIELD + 12).context("the slot's view's script is short")?;
    let (outside, id) = (i32::from_le_bytes(pointer[..4].try_into()?), i64::from_le_bytes(pointer[4..].try_into()?));
    let file = slot.file_of(outside).context("the empty view's file is not had")?;
    let empty = trp_data::screen::Scene::load_prefab_of_script(install, &file, id)?;
    let mut empty = trp_data::screen::ScenePanel::of(&empty, 0);
    empty.file = file.clone();
    Ok((scene, slot, empty, part))
}

/// The OVERALL tab as the scene has it, as it shows in a raid: its clothing
/// panel on where the options' `CLOTHING` is (the game's only out of one,
/// `ShowClothingPanel`: the user's departure), its lists' scroll areas as
/// high as they show of their lines (at most `_maxVisibleHeight`: the
/// install's suits are many more than that holds, `DropDownBox.Update`),
/// the level's icon kept to its shape (`SetLevelIcon`), the dropdowns'
/// words and their lines' put within their margins.
fn overall_panel(install: &trp_data::Install, file: &str, clothing: bool) -> Option<trp_data::screen::OverallPanel> {
    let mut overall = match trp_data::screen::OverallPanel::load(install, file) {
        Ok(overall) => overall,
        Err(e) => {
            eprintln!("the game's OVERALL tab could not be read; none is drawn: {e:#}");
            return None;
        }
    };
    let most_seen = overall.dropdown_most_seen.unwrap_or(120.0);
    let panel = &mut overall.panel;
    if let Some(clothing_panel) = panel.find(OVERALL_CLOTHING).filter(|_| clothing) {
        panel.nodes[clothing_panel].active = true;
    }
    for selector in OVERALL_SELECTORS {
        if let Some(area) = panel.find(&format!("{selector}/Open Panel/Scroll Area")) {
            let element = panel.nodes[area].element.get_or_insert(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
            element.min[1] = most_seen;
        }
    }
    if let Some(icon) = panel.find("LeftSide/CharacterPanel/Level Panel/Level Icon") {
        if let Some(image) = panel.nodes[icon].image.as_mut() {
            image.preserve_aspect = true;
        }
    }
    let boxes: Vec<usize> = OVERALL_SELECTORS.iter().filter_map(|selector| panel.find(&format!("{selector}/Text"))).collect();
    inset_by_margins(panel, &boxes);
    if let Some(line) = overall.line.as_mut() {
        let texts: Vec<usize> = (0..line.nodes.len()).filter(|&index| line.nodes[index].text.is_some()).collect();
        inset_by_margins(line, &texts);
    }
    Some(overall)
}

/// Texts of a panel kept off their rectangles' edges by their margins (left,
/// top, right, bottom: TextMeshPro's), put into their rectangles instead,
/// about their pivots: the panels' drawing lays a text in its rectangle as
/// it is.
pub(crate) fn inset_by_margins(panel: &mut trp_data::screen::ScenePanel, texts: &[usize]) {
    for &index in texts {
        let Some(margin) = panel.nodes[index].text.as_ref().map(|text| text.margin) else { continue };
        let [left, top, right, bottom] = margin;
        let layout = &mut panel.nodes[index].layout;
        let pivot = layout.pivot;
        layout.offset = [layout.offset[0] + left - pivot[0] * (left + right), layout.offset[1] + bottom - pivot[1] * (top + bottom)];
        layout.size_delta = [layout.size_delta[0] - left - right, layout.size_delta[1] - top - bottom];
        if let Some(text) = panel.nodes[index].text.as_mut() {
            text.margin = [0.0; 4];
        }
    }
}

/// What reads the game's pictures for its panels, each texture once.
pub(crate) struct PictureReader<'a> {
    pictures: &'a mut trp_data::screen::ScenePictures,
    textures: HashMap<(String, i64), usize>,
    missing: Vec<String>,
}

impl PictureReader<'_> {
    /// A sprite of that file and number as a picture, its texture among
    /// those kept (`kept`) once.
    pub(crate) fn sprite(&mut self, name: &str, id: i64, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Option<Picture> {
        let cut = self.pictures.sprite(name, id).ok().filter(|cut| !cut.turned)?;
        let texture = match self.textures.get(&cut.texture) {
            Some(&texture) => texture,
            None => {
                let texture = self.pictures.texture(&cut.texture.0, cut.texture.1).ok()?;
                kept.push(Some(texture));
                self.textures.insert(cut.texture.clone(), kept.len() - 1);
                kept.len() - 1
            }
        };
        let [x, y, w, h] = cut.rect;
        Some(Picture { texture, cut: Rect::new(x, y, x + w, y + h), covers: cut.covers(), border: cut.border, size: [cut.whole[2], cut.whole[3]] })
    }

    /// The pictures of a panel's objects that show, by their places.
    pub(crate) fn panel(&mut self, scene: &trp_data::screen::ScenePanel, shown: &[Shown], file: &str, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Vec<Option<Picture>> {
        let mut out = vec![None; scene.nodes.len()];
        for (index, node) in scene.nodes.iter().enumerate() {
            if shown[index] == Shown::Never {
                continue;
            }
            let Some((outside, id)) = node.image.and_then(|image| image.sprite) else { continue };
            let name = scene.file_of(outside).or_else(|| (outside == 0).then(|| file.to_owned()));
            out[index] = name.and_then(|name| self.sprite(&name, id, kept));
            if out[index].is_none() {
                self.missing.push(node.path.clone());
            }
        }
        out
    }
}

/// Which of the health panel's objects are drawn in a raid (`Shown`): as
/// `shown_of` has them, with the views and their effects' panels turned on
/// as the panel shows (`BodyPartView.Show`, `EffectsPanel.Show`); never the
/// legend or the damage panels, which are for the damage of a raid gone by
/// (`InventoryScreenHealthPanel.Show`, given none in a raid).
fn health_shown(panel: &trp_data::screen::HealthPanel) -> Vec<Shown> {
    let views = &panel.views;
    let turned_on: Vec<usize> = views.parts.iter().flat_map(|view| [Some(view.view), view.effects]).flatten().collect();
    let never: Vec<usize> = views.legend.into_iter().chain(views.parts.iter().filter_map(|view| view.damage)).collect();
    let mut scripted: Vec<usize> = views.parts.iter().filter_map(|view| view.broken).collect();
    for (_, figure) in &views.parameters {
        if let Some(f) = figure {
            scripted.extend([f.ok_glow, f.glow, f.up, f.down, f.rate].into_iter().flatten());
        }
    }
    shown_of(&panel.panel.nodes, &turned_on, &never, &scripted)
}

/// Which of a panel's objects are drawn: those on as the scene has them or
/// turned on as the panel shows, with all they hang from (the panel's top
/// is shown by its tab); never those of `never` or under them; as their
/// script has it, those of `scripted`.
pub(crate) fn shown_of(nodes: &[trp_data::screen::PanelNode], turned_on: &[usize], never: &[usize], scripted: &[usize]) -> Vec<Shown> {
    let on = |index: usize| nodes[index].active || turned_on.contains(&index);
    (0..nodes.len())
        .map(|index| {
            let mut above = nodes[index].parent;
            while let Some(at) = above {
                if never.contains(&at) || (nodes[at].parent.is_some() && !on(at)) {
                    return Shown::Never;
                }
                above = nodes[at].parent;
            }
            if never.contains(&index) {
                Shown::Never
            } else if scripted.contains(&index) {
                Shown::ByScript
            } else if on(index) || nodes[index].parent.is_none() {
                Shown::Always
            } else {
                Shown::Never
            }
        })
        .collect()
}

impl HealthLook {
    /// A text's font: the game's, where it is had, else the app's.
    fn font(&self, panel: &trp_data::screen::ScenePanel, font: (i32, i64)) -> Handle<Font> {
        panel.file_of(font.0).and_then(|file| self.fonts.get(&(file, font.1)).cloned()).unwrap_or_default()
    }

    /// A text's font as drawn in its style: bold in its font's bold
    /// typeface where it has one (as TextMeshPro takes it), else its own.
    /// What a text's font asset draws under its letters, if anything.
    pub fn text_effect(&self, panel: &trp_data::screen::ScenePanel, font: (i32, i64)) -> Option<TextEffect> {
        panel.file_of(font.0).and_then(|file| self.text_effects.get(&(file, font.1)).copied())
    }

    /// A text's font asset and material as the game draws it, where both
    /// are had (`TARKOV UI`): the asset by its file and number, the
    /// material the text names for itself, else the asset's own.
    pub fn tmp_of(&self, panel: &trp_data::screen::ScenePanel, text: &trp_data::screen::TextLook) -> Option<((String, i64), (String, i64))> {
        let asset = (panel.file_of(text.font.0)?, text.font.1);
        let material = text.material.and_then(|(file, id)| Some((panel.file_of(file)?, id))).filter(|key| self.tmp_materials.contains_key(key)).or_else(|| self.tmp_default_material.get(&asset).cloned())?;
        (self.tmp_faces.contains_key(&asset) && self.tmp_materials.contains_key(&material)).then_some((asset, material))
    }

    pub fn styled_font(&self, panel: &trp_data::screen::ScenePanel, font: (i32, i64), style: u32) -> Handle<Font> {
        let bold = (style & 1 != 0).then(|| panel.file_of(font.0).and_then(|file| self.bold_fonts.get(&(file, font.1)))).flatten();
        match bold.and_then(|bold| self.fonts.get(bold)) {
            Some(handle) => handle.clone(),
            None => self.font(panel, font),
        }
    }
}

// ==== TextMeshPro's texts: effects, copies, drawing ====

/// What a font asset's material draws under its letters (TextMeshPro's
/// distance-field shader): a dark rim all round them, from its outline
/// (`_OutlineWidth`) and an underlay laid unmoved under them and dilated
/// (`UNDERLAY_ON`, `_UnderlayDilate`; Bender Outline: an outline of 0.1 and
/// an underlay of 0.6, black), or a drop shadow, an underlay moved
/// (`_UnderlayOffsetX`, `_UnderlayOffsetY`; Bender Shadowed: 0.3 right and
/// down, black at 0.33). Each as wide or as far, in pixels, for a letter of
/// one, the atlas sampled at the asset's point size: an underlay moves by
/// its offset times its scale ratio (`_ScaleRatioC`) times the gradient's
/// scale (`_GradientScale`) of the atlas's pixels; an outline and an
/// underlay grow by half their width times their ratio (`_ScaleRatioA`,
/// `_ScaleRatioC`) in the distance field's measure, whose whole spans twice
/// the atlas's padding (`_GradientScale` less one; 0.6 of Bender Outline
/// is about a pixel at a size of 11). The face's own dilation
/// (`_FaceDilate`) widens the letters as far, and the shader's soft edge
/// fades the rim over its ramp (`PROVISIONAL`: drawn as copies of the
/// words moved round, a fainter ring for the fade; the field's measure,
/// TextMeshPro's generator not read).
#[derive(Clone, Copy, Debug, Default)]
pub struct TextEffect {
    /// The rim's colour and how far out from the letters its edge lies
    /// (where it is half drawn), for a letter of one.
    pub rim: Option<(Color, f32)>,
    pub shadow: Option<(Color, [f32; 2])>,
    /// How much the letters themselves are widened each way, for a letter
    /// of one (`_FaceDilate`: bolder than the font's own).
    pub face: f32,
    /// How wide the rim's edge fades, in pixels (the shader's ramp, the
    /// same at every size).
    pub soft: f32,
}

impl TextEffect {
    /// Drawn at a share of its alpha, as its words are.
    pub fn faded(self, alpha: f32) -> Self {
        let fade = |color: Color| color.with_alpha(color.alpha() * alpha);
        Self { rim: self.rim.map(|(color, width)| (fade(color), width)), shadow: self.shadow.map(|(color, at)| (fade(color), at)), ..self }
    }

    fn of(install: &trp_data::Install, file: &str, id: i64) -> Option<Self> {
        let font = trp_data::screen::TmpFont::read(install, file, id).ok()?;
        let (material_file, material) = trp_data::screen::font_material(install, file, id).ok()?;
        let props = trp_data::screen::material_properties(install, &material_file, material).ok()?;
        let float = |name: &str| props.floats.get(name).copied().unwrap_or(0.0);
        let color = |name: &str| props.colors.get(name).copied().map_or(Color::BLACK, rgba);
        // An underlay is moved by so many of the atlas's pixels: its
        // offset times its ratio times the gradient's scale (the shader's
        // `_UnderlayOffset * _ScaleRatioC * _GradientScale / _TextureWidth`
        // of the texture's width).
        let per = float("_GradientScale") / font.point_size.max(1.0);
        let underlay = props.keywords.iter().any(|keyword| keyword == "UNDERLAY_ON");
        let offset = [float("_UnderlayOffsetX"), -float("_UnderlayOffsetY")].map(|o| o * float("_ScaleRatioC") * per);
        let moved = offset.iter().any(|o| o.abs() > 1e-4);
        // An outline and a dilation are in the distance field's own
        // measure, whose 0 to 1 spans twice the atlas's padding
        // (`_GradientScale` less one) in its pixels, the edge at a half.
        let padding = (float("_GradientScale") - 1.0).max(0.0);
        let spread = 2.0 * padding / font.point_size.max(1.0);
        // The letters widened by the face's dilation (the shader's
        // `weight`: `_FaceDilate * _ScaleRatioA * 0.5`), the rim's edge as
        // far again out as its own width (the underlay's bias: `weight` and
        // `_UnderlayDilate * _ScaleRatioC * 0.5`; an outline's,
        // `_OutlineWidth * _ScaleRatioA * 0.5`); both edges fade over the
        // shader's ramp, a distance of one over its scale (`_GradientScale`
        // times the screen's pixels to the atlas's), which comes to twice
        // the padding over the gradient's scale in pixels at any size.
        let weight = float("_FaceDilate") * float("_ScaleRatioA") * 0.5;
        let rim = [float("_OutlineWidth") * float("_ScaleRatioA"), if underlay && !moved { float("_UnderlayDilate") * float("_ScaleRatioC") } else { 0.0 }];
        let rim = rim[0].max(rim[1]) * 0.5;
        let effect = Self {
            rim: (rim > 1e-4).then(|| (if underlay && !moved { color("_UnderlayColor") } else { color("_OutlineColor") }, (weight + rim) * spread)),
            shadow: (underlay && moved).then(|| (color("_UnderlayColor"), offset)),
            face: weight * spread,
            soft: if float("_GradientScale") > 0.0 { 2.0 * padding / float("_GradientScale") } else { 0.0 },
        };
        (effect.rim.is_some() || effect.shadow.is_some() || effect.face > 1e-4).then_some(effect)
    }
}

/// A copy of words drawn under them as their font's rim or shadow.
#[derive(Component)]
pub struct EffectCopy;

/// Words drawn as the game draws them (`trp_data::tmp_text`): their font
/// asset and material (each by its file and number), their runs (sizes in
/// the window's units, colours as the screen's numbers) and how they are
/// laid out (a width to break lines at in the window's units). Drawn into
/// an image of their own by `draw_tmp_texts` as they are made and as they
/// change.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TmpText {
    pub asset: (String, i64),
    pub material: (String, i64),
    pub runs: Vec<trp_data::tmp_text::TmpRun>,
    pub layout: trp_data::tmp_text::TmpLayout,
}

/// A colour as the screen's numbers (gamma), for a text drawn as the
/// game's.
pub fn screen_numbers(color: Color) -> [f32; 4] {
    let c = color.to_srgba();
    [c.red, c.green, c.blue, c.alpha]
}

/// The box a text drawn as the game's is aligned by, which its image
/// hangs from.
#[derive(Component)]
pub struct TmpBox;

/// Words drawn as the game draws them: a box laid out as `node` holding a
/// box as large as TextMeshPro aligns the words by (as wide as their
/// advances, as high as the font's ascender to its descender), and from
/// it their image, what the shader draws round them outside it. `mark`
/// puts the caller's markers on the image. Drawn once its image is made
/// (`draw_tmp_texts`), on the same frame.
pub fn tmp_words(parent: &mut ChildSpawnerCommands, node: Node, text: TmpText, mark: &mut dyn FnMut(&mut EntityCommands)) {
    parent.spawn(node).with_children(|frame| {
        let empty = Node { width: Val::Px(0.0), height: Val::Px(0.0), ..default() };
        frame.spawn((TmpBox, empty.clone())).with_children(|aligned| {
            let hung = Node { position_type: PositionType::Absolute, ..empty };
            let mut words = aligned.spawn((text, hung, ImageNode { color: Color::WHITE, ..default() }));
            mark(&mut words);
        });
    });
}

/// The images of the texts drawn as the game draws them, each made as its
/// words are first drawn or change and kept by what it is (the same words
/// alike drawn again take the same image), at the window's own pixels:
/// the image as large as it comes, its words' box where the layout puts
/// the node, by margins as wide as what lies outside it.
#[allow(clippy::type_complexity)]
fn draw_tmp_texts(
    look: Res<HealthLook>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut texts: Query<(&TmpText, &mut ImageNode, &mut Node, &ChildOf), Changed<TmpText>>,
    mut boxes: Query<&mut Node, (With<TmpBox>, Without<TmpText>)>,
    mut made: Local<HashMap<String, (Handle<Image>, [f32; 2], [f32; 4])>>,
) {
    if texts.is_empty() {
        return;
    }
    let _clock = crate::render_timing::Clock::new("texts drawn as the game's");
    let scale = window.single().map_or(1.0, |window| window.scale_factor());
    // Kept while the screens draw the same texts; let go when it has grown
    // large (each image is kept as long as a text shows it).
    if made.len() > 4096 {
        made.clear();
    }
    for (text, mut image, mut node, parent) in &mut texts {
        let key = format!("{scale}|{text:?}");
        let (handle, size, layout) = match made.get(&key) {
            Some(kept) => kept.clone(),
            None => {
                let (Some(face), Some(material)) = (look.tmp_faces.get(&text.asset), look.tmp_materials.get(&text.material)) else { continue };
                let runs: Vec<trp_data::tmp_text::TmpRun> = text.runs.iter().map(|run| trp_data::tmp_text::TmpRun { size: run.size * scale, ..run.clone() }).collect();
                let layout = trp_data::tmp_text::TmpLayout { em: text.layout.em * scale, wrap: text.layout.wrap.map(|width| width * scale), ..text.layout };
                let drawn = trp_data::tmp_text::draw(face, material, &runs, &layout);
                let mut picture = Image::new(
                    bevy::render::render_resource::Extent3d { width: drawn.width as u32, height: drawn.height as u32, depth_or_array_layers: 1 },
                    bevy::render::render_resource::TextureDimension::D2,
                    drawn.rgba,
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::RENDER_WORLD,
                );
                picture.sampler = bevy::image::ImageSampler::nearest();
                let kept = (images.add(picture), [drawn.width as f32 / scale, drawn.height as f32 / scale], drawn.layout.map(|v| v / scale));
                made.insert(key, kept.clone());
                kept
            }
        };
        let [lx, ly, lw, lh] = layout;
        let [w, h] = size;
        image.image = handle;
        (node.left, node.top, node.width, node.height) = (Val::Px(-lx), Val::Px(-ly), Val::Px(w), Val::Px(h));
        if let Ok(mut aligned) = boxes.get_mut(parent.parent()) {
            (aligned.width, aligned.height) = (Val::Px(lw), Val::Px(lh));
        }
    }
}

/// Words with what their font asset draws under them: a box laid out as
/// `node` holding, in one cell of a grid, copies of the words in the
/// effect's colour moved twelve ways round for the rim (a whole ring and a
/// fainter one outside it, for its soft edge) or moved for the shadow,
/// copies in their own colour moved four ways by the face's dilation, and
/// the words themselves over them. `runs` are the words
/// and each one's font and colour; `size` the letters' height in pixels the
/// effect is measured by. `mark` puts the caller's markers on each text
/// (`None`) and each run (its place), and says whether it is a copy.
pub fn effect_words(parent: &mut ChildSpawnerCommands, node: Node, runs: &[(String, TextFont, Color)], effect: Option<TextEffect>, size: f32, mark: &mut dyn FnMut(&mut EntityCommands, Option<usize>, bool)) {
    let Some((_, first_font, first_color)) = runs.first() else { return };
    // Words in a box of a set width are broken into lines at it (a
    // window's description), each copy as wide; others run on one line.
    let wrapped = matches!(node.width, Val::Px(_));
    let layout = if wrapped { TextLayout::new_with_linebreak(LineBreak::WordBoundary) } else { TextLayout::new_with_no_wrap() };
    // Each copy: its colour (none: the words' own) and where it is moved.
    let mut copies: Vec<(Option<Color>, Vec2)> = Vec::new();
    // Twelve ways round, so that the rim is round.
    let round = |r: f32| (0..12).map(move |k| Vec2::from_angle(k as f32 * std::f32::consts::TAU / 12.0) * r);
    if let Some((color, width)) = effect.and_then(|effect| effect.rim) {
        // The rim's edge fades over the shader's ramp: whole to a quarter
        // of it inside its edge, then a fainter ring a quarter of it out.
        let (edge, soft) = (width * size, effect.map_or(0.0, |effect| effect.soft));
        if soft > 0.5 {
            copies.extend(round(edge + soft / 4.0).map(|at| (Some(color.with_alpha(color.alpha() * 0.4)), at)));
        }
        copies.extend(round((edge - soft / 4.0).max(0.5)).map(|at| (Some(color), at)));
    }
    if let Some((color, [x, y])) = effect.and_then(|effect| effect.shadow) {
        copies.push((Some(color), Vec2::new(x, y) * size));
    }
    // The letters widened: the words again in their own colours, moved
    // that far each way.
    let face = effect.map_or(0.0, |effect| effect.face * size);
    if face >= 0.15 {
        copies.extend([Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y].map(|way| (None, way * face)));
    }
    let cell = || Node { grid_row: GridPlacement::start(1), grid_column: GridPlacement::start(1), width: if wrapped { Val::Percent(100.0) } else { Val::Auto }, ..default() };
    parent.spawn(Node { display: Display::Grid, ..node }).with_children(|grid| {
        for (color, at) in copies.iter().copied().chain(std::iter::once((None, Vec2::ZERO))) {
            let copy = color.is_some();
            let moved = UiTransform { translation: Val2::px(at.x, at.y), ..UiTransform::IDENTITY };
            let mut text = grid.spawn((cell(), Text::new(""), first_font.clone(), TextColor(color.unwrap_or(*first_color)), layout, moved));
            if copy {
                text.insert(EffectCopy);
            }
            mark(&mut text, None, copy);
            text.with_children(|words| {
                for (at, (said, font, run_color)) in runs.iter().enumerate() {
                    let mut span = words.spawn((TextSpan::new(said.clone()), font.clone(), TextColor(color.unwrap_or(*run_color))));
                    if copy {
                        span.insert(EffectCopy);
                    }
                    mark(&mut span, Some(at), copy);
                }
            });
        }
    });
}

/// The weight a text is laid out at by its style (TextMeshPro's
/// `FontStyles.Bold`): Bevy finds a face by its family and the weight
/// asked, so a bold typeface of the same family as the regular one is only
/// had by asking for it bold.
pub fn weight_of(style: u32) -> FontWeight {
    if style & 1 != 0 { FontWeight::BOLD } else { FontWeight::NORMAL }
}

// ==== The item view's, the spinner's, the wish's and the tag's looks ====

/// What the game's own view of a thing looks like (`TARKOV UI`), in a slot
/// (`SlotItemView`, `slot_layout`) or in a grid (`GridItemView`,
/// `grid_layout`): its prefab, where its caption and its value lie in it
/// (`Info Panel/Name` or `Info Panel/Caption`, and the bottom right
/// group's `Value`, by their names), their fonts, its border's colour
/// where its border is on, and the label of a thing bound to a key
/// (`BindPanel`: its picture, and its key's words).
pub struct ItemViewLook {
    pub panel: trp_data::screen::ScenePanel,
    pub caption: Option<usize>,
    pub value: Option<usize>,
    pub caption_font: Handle<Font>,
    pub value_font: Handle<Font>,
    pub border: Option<Color>,
    pub bind: Option<BindLook>,
    /// Where a weapon's round is written (`SetInscription`: in a slot the
    /// top right `Info Panel/Info Label`, under the caption; in a grid the
    /// bottom left `Inscription`), and its font.
    pub inscription: Option<usize>,
    pub inscription_font: Handle<Font>,
    /// What the caption's, the value's and the round's fonts draw under
    /// them (the item views' Bender Outline: a dark rim).
    pub caption_effect: Option<TextEffect>,
    pub value_effect: Option<TextEffect>,
    pub inscription_effect: Option<TextEffect>,
    /// The caption's, the round's and the value's font asset and material
    /// as the game draws them (`TmpText`), where had.
    pub caption_tmp: Option<((String, i64), (String, i64))>,
    pub inscription_tmp: Option<((String, i64), (String, i64))>,
    pub value_tmp: Option<((String, i64), (String, i64))>,
    /// The caption's font asset, which measures what of it fits (its
    /// overflow cuts what does not: `m_overflowMode`).
    pub caption_face: Option<std::sync::Arc<trp_data::tmp_text::TmpFace>>,
    /// A weapon's mastering in a slot (`SlotItemView.UpdateInfo`): the
    /// mark (`MasteringIcon/Image`) and its level's words' size, colour
    /// and font (`Level`).
    pub mastering: Option<(Sprite, f32, Color, Handle<Font>)>,
    /// A weapon's or a knife's durability in a slot (`DurabilitySlider`):
    /// its track (`Background`, sliced) and its fill (`Fill`).
    pub durability: Option<(Sprite, Sprite)>,
    /// A part's marks (`ItemViewStats.SetItemIcon`): its class's ground by
    /// `type_<class>` (`Mod Types/icon_type_mod_<class>`) and its kind's
    /// mark by `mod_<kind>` (`Mod Tactical Types/icon_mod_<kind>`).
    pub mods: HashMap<String, Sprite>,
    /// A tag's plate (`TagPanel`, `_tagColor`) and its name (`TagName`),
    /// with the name's and the caption's font assets as TextMeshPro has
    /// them, which size the plate (`ResizeTag`).
    pub tag: Option<TagPlateLook>,
    /// A thing on the wish list's marks (`WishlistGridView`).
    pub wish: Option<WishLook>,
    /// The load component over a magazine's view while it is worked.
    pub spin: Option<SpinLook>,
    /// Its loader, shown while its picture is still to come (`ItemView.
    /// _iconLoader`: `Loader`, kept its own size in the view's middle),
    /// and that size.
    pub loader: Option<(Sprite, [f32; 2])>,
}

/// The load component (`ItemViewLoadAmmoComponent`; inventory spec, "The
/// load component") as `ItemViewLook::spin` has it: its panel, laid out
/// over a view, and its pieces, each its object and its picture: the
/// spinner's back and fill, the loader, the stop cross.
pub struct SpinLook {
    pub panel: trp_data::screen::ScenePanel,
    pub back: Option<(usize, SpinPicture)>,
    pub fill: Option<(usize, SpinPicture)>,
    pub loader: Option<(usize, SpinPicture)>,
    pub stop: Option<(usize, SpinPicture)>,
}

/// A piece's picture: its texture, its cut and the share of the piece's
/// rectangle the cut covers (a trimmed sprite).
#[derive(Clone)]
pub struct SpinPicture {
    pub image: Handle<Image>,
    pub cut: Rect,
    covers: [f32; 4],
}

impl SpinLook {
    /// A piece's picture's rectangle over a view of that size, in units.
    pub fn rect(&self, index: usize, picture: &SpinPicture, view: [f32; 2]) -> [f32; 4] {
        let [x, y, w, h] = self.panel.placed(index, view).rect;
        let c = picture.covers;
        [x + w * c[0], y + h * (1.0 - c[3]), w * (c[2] - c[0]), h * (c[3] - c[1])]
    }
}

/// A view's marks of the wish list as `ItemViewLook::wish` has them: the
/// glow over it and its border's edges, each its object and its picture;
/// its icon's object, the column it is the first of
/// (`BottomLeftLayoutGroup`), and what the column asks for a part's marks
/// (`ModIconContainer`) and, where the weapon's round is written in it, for
/// that line (TextMeshPro's height of it); and the groups' outlined icons.
pub struct WishLook {
    pub glow: Option<(usize, Sprite)>,
    pub edges: Vec<(usize, Sprite)>,
    pub icon: usize,
    pub column: usize,
    pub mod_height: f32,
    pub inscription_height: Option<f32>,
    pub icons: HashMap<&'static str, Sprite>,
}

/// A view's tag plate as `ItemViewLook::tag` has it.
pub struct TagPlateLook {
    pub plate: usize,
    pub name: usize,
    pub name_tmp: Option<((String, i64), (String, i64))>,
    pub name_font: Handle<Font>,
    pub name_face: Option<std::sync::Arc<trp_data::tmp_text::TmpFace>>,
    pub caption_face: Option<std::sync::Arc<trp_data::tmp_text::TmpFace>>,
}

impl TagPlateLook {
    /// The plate's width in units and whether its name shows
    /// (`GridItemView.ResizeTag`): where the view's width less the
    /// caption's drawn width and 2 leaves less than 40 the plate is the
    /// view's width with no name, else the name's width and 12, from 40 to
    /// that room.
    pub fn width(&self, panel: &trp_data::screen::ScenePanel, caption: Option<usize>, caption_words: &str, name: &str, view_width: f32) -> (f32, bool) {
        let drawn = |face: &Option<std::sync::Arc<trp_data::tmp_text::TmpFace>>, index: Option<usize>, words: &str| {
            let text = index.and_then(|index| panel.nodes[index].text.as_ref());
            face.as_ref().zip(text).map_or(0.0, |(face, text)| face.font.width(words, text.size, text.spacing, text.style & 1 != 0))
        };
        let room = view_width - drawn(&self.caption_face, caption, caption_words) - 2.0;
        if room < 40.0 {
            return (view_width, false);
        }
        ((drawn(&self.name_face, Some(self.name), name) + 12.0).clamp(40.0, room), true)
    }
}

/// A part's class and the name of its ground's picture
/// (`ItemViewFactory.GetModClass`, `GetModClassIconName`), by the
/// database's names of the game's classes, in the game's order of asking.
pub const MOD_CLASSES: [(&str, &str); 5] = [("FunctionalMod", "generic"), ("GearMod", "gear"), ("MasterMod", "vital"), ("AuxiliaryMod", "gear"), ("ArmorPlate", "armor_plate")];

/// A part's kind and the name of its mark's picture (`GetModSubclass`,
/// `GetModIconName`), the same way: the first the part is of wins.
pub const MOD_KINDS: [(&str, &str); 21] = [
    ("Bipod", "bipod"),
    ("IronSight", "ironsight"),
    ("Launcher", "launcher"),
    ("LightLaser", "lightlaser"),
    ("Mount", "mount"),
    ("Muzzle", "muzzle"),
    ("RailCovers", "railcovers"),
    ("Sights", "sight"),
    ("TacticalCombo", "tactical"),
    ("Foregrip", "tactical"),
    ("Magazine", "magazine"),
    ("Gasblock", "gasblock"),
    ("Handguard", "handguard"),
    ("Receiver", "reciever"),
    ("PistolGrip", "pistol_grip"),
    ("Flashlight", "lightlaser"),
    ("AuxiliaryMod", "aux"),
    ("Charge", "charge"),
    ("Stock", "stock"),
    ("Barrel", "barrel"),
    // (A plate, or a built-in insert: `ArmorPlate`, the game's last.)
    ("ArmorPlate", "armor_plate"),
];

// ==== Sprites, and the inspect window: places, parts, buttons, mastering, rows ====

/// One of the game's pictures as the inventory draws it by hand: its
/// texture's cut at its sprite's own size in units, how far in a sliced
/// one keeps its edges whole (left, bottom, right, top), and its colour.
#[derive(Clone)]
pub struct Sprite {
    pub image: Handle<Image>,
    pub cut: Rect,
    pub size: [f32; 2],
    pub border: [f32; 4],
    pub color: Color,
}

/// Draws a picture into a node of that place and size, in pixels: sliced
/// where it keeps edges, else stretched, its lowest row first as the engine
/// keeps it and drawn turned up; `s` pixels to a unit.
pub fn draw_sprite(parent: &mut ChildSpawnerCommands, sprite: &Sprite, rect: [f32; 4], s: f32) {
    let [x, y, w, h] = rect;
    let node = Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(w), height: Val::Px(h), ..default() };
    if sprite.border.iter().any(|&edge| edge > 0.0) {
        let slices = Slices { image: sprite.image.clone(), cut: sprite.cut, border: sprite.border, per: s, fill_center: true, color: sprite.color, flip_x: false, tiled: false };
        parent.spawn((node, EXACT)).with_children(|frame| nine_slice(frame, &slices, [w, h], usize::MAX));
    } else {
        parent.spawn((node, EXACT, ImageNode { image: sprite.image.clone(), color: sprite.color, rect: Some(sprite.cut), flip_y: true, ..default() }));
    }
}

/// A picture's piece laid out where the game puts it, to a fraction of a
/// pixel, not rounded to the screen's pixels: rounded one by one, pieces
/// that meet in the game's units fell a pixel apart or over one another
/// at a scale of a third (a slot's header, the middle tiled between two
/// ends, a pixel short of its right end at 2560 by 1440), where the game
/// draws them edge to edge. Texts keep their rounding (sharp).
pub(crate) const EXACT: bevy::ui::LayoutConfig = bevy::ui::LayoutConfig { use_rounding: false };

/// What the window that shows a thing writes of it
/// (`ItemSpecificationPanel`, into `ItemInfoWindowLabels`): its name in
/// the caption (`ItemName`), its kind as the handbook files it
/// (`_handbookNode.Category`, " > " between), its weight with the unit
/// (`GetTruncatedWeightString`, three places) and its description.
#[derive(Default)]
pub struct InfoWords {
    /// Its tag, where it has one named (`InfoWindow.UpdateTag`).
    pub tag: Option<(String, u32)>,
    /// Its kind's group on the wish list, where it is on it
    /// (`ItemSpecificationPanel._wishlistIcon`).
    pub wish: Option<&'static str>,
    pub caption: String,
    pub kind: String,
    pub weight: String,
    pub description: String,
}

/// The objects of the window that shows a thing its script turns off for
/// a thing of the range (`HealthLook::read`); its weight's figure, its
/// caption bar, its cross, its preview and its description's box.
const INFO_OFF: [&str; 0] = [];
/// Its panel of a thing's parts' slots (`_modsPanel`), on for a thing with
/// slots, its grid of slots and its scrollbar; a weapon's mastering row,
/// its value, its badge under the preview and the preview's corner beside
/// it (`_masteringBg`), on for a weapon; its buttons' grid and its three
/// templates (`SimpleSpecifiedContextButtons`: Discard's own, the wish
/// list's own, the rest).
const INFO_MODS: &str = "Inner/Contents/DescriptionPanel/ModsPanel";
const INFO_MODS_CONTENT: &str = "Inner/Contents/DescriptionPanel/ModsPanel/Padding/Content";
const INFO_MODS_BAR: &str = "Inner/Contents/DescriptionPanel/ModsPanel/Scrollbar";
const INFO_MASTERING: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel/Mastering";
const INFO_MASTERING_VALUE: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel/Mastering/Value";
const INFO_MASTERING_BADGE: &str = "Inner/Contents/Preview Panel/MasteringAndQuestPanel/MasteringPanel";
const INFO_CORNER: &str = "Inner/Contents/Preview Panel/bottom_left";
const INFO_BUTTONS: &str = "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer";
const INFO_BUTTON_TEMPLATES: [&str; 3] = [
    "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer/Button Template",
    "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer/Wishlist Template",
    "Inner/Contents/InteractionButtonsPanel/InteractionButtonsContainer/Dispose Template",
];
/// The window's stretching places (`StretchButtons`' `StretchArea`s), each
/// with its sides (`EStretchAreaType`: top 1, bottom 2, left 4, right 8).
const INFO_STRETCH: [(&str, u8); 8] = [
    ("StretchButtons/TopStretch", 1),
    ("StretchButtons/BottomStretch", 2),
    ("StretchButtons/LeftStretch", 4),
    ("StretchButtons/RightStretch", 8),
    ("StretchButtons/TopLeftStretch", 5),
    ("StretchButtons/TopRightStretch", 9),
    ("StretchButtons/BottomLeftStretch", 6),
    ("StretchButtons/BottomRightStretch", 10),
];
/// A slot's pieces its script shows as the slot is full or empty.
const MOD_SLOT_PIECES: [&str; 6] = ["CanvasGroup/Empty Border", "CanvasGroup/Full Border", "CanvasGroup/Background", "CanvasGroup/Background Image", "CanvasGroup/Text", "CanvasGroup/filters"];
/// Its loader (shown until its model is in), its column of bars and the
/// weapon's durability row at its top, its compact grid.
const INFO_LOADER: &str = "Inner/Contents/Preview Panel/Loader";
const INFO_DURABILITY: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel/Durability";
const INFO_BARS: &str = "Inner/Contents/DescriptionPanel/CharacteristicsPanel";
const INFO_COMPACT: &str = "Inner/Contents/DescriptionPanel/CompactPanel";
/// Its mark of the wish list, which its script shows for a thing on it.
const INFO_WISH: &str = "Inner/Contents/Preview Panel/MasteringAndQuestPanel/WishlistIcon";
/// Its tag's plate and name, which its script shows for a thing tagged.
const INFO_TAG: &str = "Inner/Caption Panel/TagPanel";
const INFO_TAG_TEXT: &str = "Inner/Caption Panel/TagPanel/TagText";
const INFO_MASS: &str = "Inner/Caption Panel/MassText";
const INFO_CAPTION: &str = "Inner/Caption Panel";
const INFO_CLOSE: &str = "Inner/Caption Panel/Close Button";
const INFO_PREVIEW: &str = "Inner/Contents/Preview Panel/Camera Image";
const INFO_SCROLL: &str = "Inner/Contents/DescriptionPanel/DescriptionPanel/Scroll Area";
const INFO_DESCRIPTION: &str = "Inner/Contents/DescriptionPanel/DescriptionPanel/Scroll Area/Contents/Description";

/// The window that shows a thing as the game lays it out (`TARKOV UI`):
/// its size, and where in it its caption bar (by which it is moved) and
/// its cross lie, in units from its top left corner.
pub struct InfoPlaces {
    pub size: [f32; 2],
    pub caption: [f32; 4],
    pub close: [f32; 4],
}

pub fn info_places(look: &HealthLook) -> Option<InfoPlaces> {
    let info = look.info.as_ref()?;
    let size = info_size(info);
    let rect = |path: &str| info.find(path).map(|index| info.rect(index, size));
    Some(InfoPlaces { size, caption: rect(INFO_CAPTION)?, close: rect(INFO_CLOSE)? })
}

/// The window's own size, as it lies in a screen (its anchors at a point).
fn info_size(info: &trp_data::screen::ScenePanel) -> [f32; 2] {
    let [_, _, w, h] = info.placed_in(0, [1920.0, 1080.0]).rect;
    [w, h]
}

/// The window that shows a thing laid out for it (`TARKOV UI`): the game's
/// window with what its script writes (`InfoWords`), a weapon's durability
/// row, a copy of a row template for each of its figures (inspect-window
/// spec, "Where the figures go"), its loader while its model is made, all
/// laid out by the game's layout; the window as tall as what it holds asks
/// and at least its own height (its `ContentSizeFitter` and
/// `LayoutElement`).
pub struct InfoBuilt {
    panel: trp_data::screen::ScenePanel,
    drawn: Drawn,
    texts: HashMap<usize, String>,
    set: HashMap<usize, bool>,
    colors: HashMap<usize, Color>,
    sprites: HashMap<usize, Picture>,
    groups: HashMap<usize, f32>,
    /// The kinds' marks of an empty slot's filters, each where it lies.
    icons: Vec<([f32; 4], Picture)>,
    pub size: [f32; 2],
    /// Where each slot of the parts' panel lies, and each button, in the
    /// order given; each stretching place with its sides; in units from
    /// the window's top left corner.
    pub slots: Vec<[f32; 4]>,
    pub buttons: Vec<[f32; 4]>,
    /// Beside `buttons`, each one's ground (`_backgroundImage`): its node,
    /// its colours at rest and under the mouse, and its group's alpha.
    pub grounds: Vec<Option<(usize, [Color; 2], f32)>>,
    pub stretch: Vec<(u8, [f32; 4])>,
    /// Each dropdown row's arrow (`_dropdownImage`), with the row's place
    /// among the figures' rows.
    pub tips: Vec<([f32; 4], usize)>,
    /// The description's scroll view, where its text is taller than its
    /// area (else its bar is hidden: `AutoHide`).
    pub scroll: Option<InfoScroll>,
}

/// The description's scroll view as the window is built
/// (`DescriptionPanel/Scroll Area`'s `ScrollRectNoDrag`; inspect-window
/// spec, "The description"): where its area, its bar and its bar's sliding
/// area lie, in units from the window's top left; its handle (its node,
/// layout and colour) and how the bar runs and is tinted; how tall the text
/// is and how much of it shows; the units a notch of the wheel moves it.
#[derive(Clone)]
pub struct InfoScroll {
    pub area: [f32; 4],
    pub bar: [f32; 4],
    pub sliding: [f32; 4],
    handle: usize,
    handle_layout: trp_data::screen::Layout,
    bottom_up: bool,
    pub tints: Option<trp_data::screen::Tints>,
    pub color: Color,
    pub whole: f32,
    pub shown: f32,
    pub sensitivity: f32,
    pub vertical: bool,
}

impl InfoScroll {
    /// How far it scrolls, units.
    pub fn most(&self) -> f32 {
        (self.whole - self.shown).max(0.0)
    }

    /// The handle's share of its bar (`Scrollbar.size`).
    pub fn share(&self) -> f32 {
        if self.whole > 0.0 { (self.shown / self.whole).clamp(0.0, 1.0) } else { 1.0 }
    }

    /// The bar's value with the text moved `at` units up, and back.
    pub fn value_of(&self, at: f32) -> f32 {
        let most = self.most();
        if most > 0.0 { 1.0 - at / most } else { 1.0 }
    }
    pub fn at_of(&self, value: f32) -> f32 {
        (1.0 - value.clamp(0.0, 1.0)) * self.most()
    }

    /// Whether the bar runs from the bottom up.
    pub fn bottom_up(&self) -> bool {
        self.bottom_up
    }

    /// Where the handle lies with the text moved `at` units up.
    pub fn handle_at(&self, at: f32) -> [f32; 4] {
        trp_data::screen::handle_on(self.sliding, self.handle_layout, self.bottom_up, self.value_of(at), self.share())
    }
}

/// What of the description moves as it is scrolled, and the bar's handle
/// with its own colour, by the window's opening (`Win::opened`).
#[derive(Component)]
pub(crate) struct InfoScrollContent(pub(crate) u64);
#[derive(Component)]
pub(crate) struct InfoScrollHandle(pub(crate) u64, pub(crate) Color);

/// The description's scroll view (`ScrollRectNoDrag`, vertical, its bar
/// hidden while the text fits): the text's height as its font lays it out
/// at the area's width less its margins, with its margins above and below
/// (`Contents`' `ContentSizeFitter` at its preferred height), against the
/// area's height; none where it fits by a hundredth, or where the text is
/// not one of the game's fonts (`PROVISIONAL`: then taken to fit).
fn info_scroll(look: &HealthLook, panel: &trp_data::screen::ScenePanel, size: [f32; 2], description: &str) -> Option<InfoScroll> {
    let area = panel.find(INFO_SCROLL)?;
    let scrolling = trp_data::screen::Scrolling::of(panel, area, "ScrollRectNoDrag")?;
    let bar = scrolling.bar?;
    let text = panel.nodes[panel.find(INFO_DESCRIPTION)?].text.as_ref()?;
    let [_, _, w, h] = panel.rect(area, size);
    let (asset, _) = look.tmp_of(panel, text)?;
    let face = look.tmp_faces.get(&asset)?;
    let run = trp_data::tmp_text::TmpRun { text: description.to_owned(), size: text.size, color: [1.0; 4], bold: text.style & 1 != 0 };
    let layout = trp_data::tmp_text::TmpLayout { em: text.size, spacing: text.spacing, kerning: text.kerning, wrap: Some(w - text.margin[0] - text.margin[2]), align: text.horizontal & 7 };
    let [_, tall] = trp_data::tmp_text::measure(face, &[run], &layout);
    let whole = tall + text.margin[1] + text.margin[3];
    if whole <= h + 0.01 {
        return None;
    }
    let (handle, sliding) = (bar.handle?, bar.sliding?);
    let color = panel.nodes[handle].image.map_or(Color::WHITE, |image| rgba(image.color));
    Some(InfoScroll {
        area: panel.rect(area, size),
        bar: panel.rect(bar.object, size),
        sliding: panel.rect(sliding, size),
        handle,
        handle_layout: panel.nodes[handle].layout,
        bottom_up: bar.bottom_up,
        tints: bar.tints.filter(|_| bar.tinted == bar.handle),
        color,
        whole,
        shown: h,
        sensitivity: scrolling.sensitivity,
        vertical: scrolling.vertical,
    })
}

/// A slot of the parts' panel as the window shows it (`ModSlotView.Show`,
/// `SetLocked`; inspect-window spec, "The parts' panel").
pub struct InfoSlot {
    /// Its name in the install's words, upper case
    /// (`slot.Name.Localized().ToUpper()`), shown while it is empty.
    pub name: String,
    /// Its picture's name (`Slots/<name>`, lower case), and whether it is
    /// an armour's (`armor_plate`'s picture where its own has none).
    pub picture: String,
    pub armour: bool,
    pub full: bool,
    /// Locked (`ModLock`: the slot at half its alpha, nothing taken from it).
    pub locked: bool,
    /// The marks of the kinds of part it takes (`SetFilters`), shown while
    /// it is empty.
    pub marks: Vec<&'static str>,
}

/// A button of the window's grid of them (`InteractionButtonsContainer`):
/// its line's name (its template and icon go by it), its caption, whether
/// it can be done now, whether it opens lines of its own.
pub struct InfoButtonLook {
    pub name: &'static str,
    pub caption: String,
    pub enabled: bool,
    pub arrow: bool,
}

/// A weapon's mastering as the window shows it: its level (0 to 2), its
/// points into it and the points the level takes.
pub struct InfoMastering {
    pub level: u32,
    pub points: f32,
    pub most: f32,
}

/// What the window holds besides its figures (inspect-window spec): its
/// parts' slots, its buttons, a weapon's mastering, and the size it is
/// stretched to (its `LayoutElement`'s preferred size, in units; none as
/// made, 670 by 500).
#[derive(Default)]
pub struct InfoParts {
    pub slots: Vec<InfoSlot>,
    pub buttons: Vec<InfoButtonLook>,
    pub mastering: Option<InfoMastering>,
    pub stretched: Option<[f32; 2]>,
}

impl InfoBuilt {
    /// Where its caption bar and its cross lie.
    pub fn places(&self) -> Option<InfoPlaces> {
        let rect = |path: &str| self.panel.find(path).map(|index| self.panel.rect(index, self.size));
        Some(InfoPlaces { size: self.size, caption: rect(INFO_CAPTION)?, close: rect(INFO_CLOSE)? })
    }

    /// Where its model is drawn (`Camera Image`), in units from its top
    /// left corner.
    pub fn preview(&self) -> Option<[f32; 4]> {
        self.panel.find(INFO_PREVIEW).map(|index| self.panel.rect(index, self.size))
    }
}

/// The window laid out for a thing with those figures (none: its frame
/// alone), its loader shown while `loading`.
pub fn info_built(look: &HealthLook, words: &InfoWords, figures: Option<&trp_sim::item_attributes::Figures>, parts: &InfoParts, loading: bool) -> Option<InfoBuilt> {
    let info = look.info.as_ref()?;
    let mut panel = info.clone();
    let mut drawn = Drawn { shown: look.info_drawn.shown.clone(), pictures: look.info_drawn.pictures.clone() };
    // Stretched (`StretchArea.OnDrag`): its preferred size the one dragged
    // to, which it is never smaller than what it holds asks; first, as the
    // grids within it lay out by its width.
    if let Some(stretched) = parts.stretched {
        let element = panel.nodes[0].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[0].element = Some(trp_data::screen::Element { preferred: stretched, ..element });
    }
    // The weight's figure sized to its words (its `ContentSizeFitter`),
    // its icon at its left.
    if let (Some(mass), Some(font)) = (panel.find(INFO_MASS), look.info_mass_font.as_ref()) {
        if let Some(text) = panel.nodes[mass].text.clone() {
            let width = font.width(&words.weight, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
            panel.nodes[mass].layout.size_delta[0] = width;
        }
    }
    let mut texts = HashMap::new();
    for (path, said_now) in [("Inner/Caption Panel/Caption", &words.caption), ("Inner/Caption Panel/Item Type", &words.kind), (INFO_MASS, &words.weight)] {
        if let Some(index) = panel.find(path) {
            texts.insert(index, said_now.clone());
        }
    }
    // Its tag's plate, coloured and sized to its name (`UpdateTag`,
    // `ResizeTag`: the name's drawn width and 12, at least 40).
    let mut set = HashMap::new();
    let mut colors = HashMap::new();
    if let (Some((name, color)), Some(plate), Some(text)) = (&words.tag, panel.find(INFO_TAG), panel.find(INFO_TAG_TEXT)) {
        set.insert(plate, true);
        colors.insert(plate, tag_colour(*color));
        texts.insert(text, name.clone());
        let drawn = panel.nodes[text].text.as_ref().and_then(|look_of| Some((look_of, look.tmp_faces.get(&(panel.file_of(look_of.font.0)?, look_of.font.1))?))).map_or(0.0, |(text, face)| face.font.width(name, text.size, text.spacing, text.style & 1 != 0));
        panel.nodes[plate].layout.size_delta[0] = (drawn + 12.0).max(40.0);
    } else if let Some(text) = panel.find(INFO_TAG_TEXT) {
        // With no tag its text goes with its plate (the pieces are drawn
        // apart, and the plate's being off does not hide it).
        set.insert(text, false);
    }
    // Its mark of the wish list: the group's icon (`_wishlistIcon`).
    let mut sprites = HashMap::new();
    if let (Some(group), Some(icon)) = (words.wish, panel.find(INFO_WISH)) {
        if let Some(picture) = look.wish_icons.get(group) {
            set.insert(icon, true);
            panel.nodes[icon].active = true;
            // Its row lays it out at its sprite's own size (an `Image`'s
            // preferred size, which the layout here does not ask of it).
            panel.nodes[icon].element = Some(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: picture.size, flexible: [-1.0, -1.0] });
            sprites.insert(icon, *picture);
        }
    }
    // Its loader while the model is made (`SetupItemPreview`'s first
    // call), and the picture it is drawn into after.
    if let Some(loader) = panel.find(INFO_LOADER) {
        set.insert(loader, loading);
    }
    let mut tip_nodes = Vec::new();
    if let Some(figures) = figures {
        tip_nodes = info_rows(look, &mut panel, &mut drawn, &mut texts, &mut set, &mut colors, &mut sprites, figures);
    }
    let mut groups = HashMap::new();
    let mut marks = PartsMarks { texts: &mut texts, set: &mut set, colors: &mut colors, sprites: &mut sprites, groups: &mut groups };
    info_mastering(look, &mut panel, &mut marks, parts.mastering.as_ref());
    let (button_tops, grounds): (Vec<usize>, Vec<Option<(usize, [Color; 2], f32)>>) = info_buttons(look, &mut panel, &mut drawn, &mut marks, &parts.buttons).into_iter().unzip();
    let (slot_tops, filters) = info_slots(look, &mut panel, &mut drawn, &mut marks, &parts.slots);
    let size = fitted_size(&panel);
    let rect = |index: usize| panel.rect(index, size);
    let slots = slot_tops.iter().map(|&top| rect(top)).collect();
    let buttons = button_tops.iter().map(|&top| rect(top)).collect();
    let stretch = INFO_STRETCH.iter().filter_map(|(path, sides)| panel.find(path).map(|index| (*sides, rect(index)))).collect();
    let tips = tip_nodes.iter().map(|&(node, row)| (rect(node), row)).collect();
    // The description's bar, hidden while its text fits.
    let scroll = info_scroll(look, &panel, size, &words.description);
    if scroll.is_none() {
        let bar = panel.find(INFO_SCROLL).and_then(|area| trp_data::screen::Scrolling::of(&panel, area, "ScrollRectNoDrag")).and_then(|scrolling| scrolling.bar);
        if let Some(bar) = bar {
            set.insert(bar.object, false);
        }
    }
    // The filters' marks: in their row at the slot's foot (a row from the
    // lower left, 2 apart), each at its own size (`SetNativeSize`).
    let mut icons = Vec::new();
    for (row, pictures) in filters {
        let [x, y, _, h] = rect(row);
        let spacing = panel.nodes[row].group.map_or(2.0, |group| group.spacing);
        let mut left = x;
        for picture in pictures {
            icons.push(([left, y + h - picture.size[1], picture.size[0], picture.size[1]], picture));
            left += picture.size[0] + spacing;
        }
    }
    Some(InfoBuilt { panel, drawn, texts, set, colors, sprites, groups, icons, size, slots, buttons, grounds, stretch, tips, scroll })
}

/// A row's tooltip as the game's draw (`SimpleTooltip` for a list of
/// words, `MultiLineTooltip` for lines): in the simple tooltip's look (its
/// font, size and colours, on black within its frame's colour), a line
/// each, an armour class's mark before one that has it at its own size
/// (or as much room), the words in their colours (their colour tags), a
/// break as a line of the frame's colour; its bottom left corner at
/// `corner` (pixels), `s` pixels to a unit. Its padding and line spacing
/// the app's (`ESTIMATE`: the tooltips' prefabs are not read).
pub fn draw_tip(commands: &mut Commands, look: &HealthLook, lines: &[trp_sim::item_attributes::TipLine], corner: Vec2, screen: Vec2, s: f32) -> Entity {
    let (font, size, color, frame) = look.tooltip_words();
    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(corner.x.max(0.0)),
        bottom: Val::Px((screen.y - corner.y).max(0.0)),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(3.0 * s),
        padding: UiRect { left: Val::Px(6.0 * s), right: Val::Px(8.0 * s), top: Val::Px(4.0 * s), bottom: Val::Px(5.0 * s) },
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    };
    commands
        .spawn((node, BackgroundColor(Color::BLACK), BorderColor::all(frame), GlobalZIndex(40)))
        .with_children(|tip| {
            for line in lines {
                if line.divider {
                    tip.spawn((Node { height: Val::Px(1.0), width: Val::Percent(100.0), ..default() }, BackgroundColor(frame)));
                    continue;
                }
                tip.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(4.0 * s), ..default() }).with_children(|row| {
                    let mark = line.class.and_then(|class| look.class_marks.get(&class)).and_then(|picture| look.sprite_of(*picture, Color::WHITE));
                    match mark {
                        Some(mark) => {
                            row.spawn((Node { width: Val::Px(mark.size[0] * s), height: Val::Px(mark.size[1] * s), ..default() }, ImageNode { image: mark.image.clone(), rect: Some(mark.cut), flip_y: true, ..default() }));
                        }
                        None if line.indent || line.class.is_some() => {
                            row.spawn(Node { width: Val::Px(20.0 * s), ..default() });
                        }
                        None => {}
                    }
                    let spans = rich_spans(&line.text).unwrap_or_else(|| vec![(line.text.clone(), None)]);
                    row.spawn((Text::default(), TextFont { font: font.clone(), font_size: size * s, ..default() }, TextColor(color), TextLayout::new_with_no_wrap())).with_children(|text| {
                        for (said, tint) in spans {
                            let tinted = tint.map_or(color, |c| Color::srgba(c[0], c[1], c[2], c[3]));
                            text.spawn((TextSpan::new(said.trim_end().to_owned()), TextFont { font: font.clone(), font_size: size * s, ..default() }, TextColor(tinted)));
                        }
                    });
                });
            }
        })
        .id()
}

/// What a part of the window puts on its pieces, besides their places.
struct PartsMarks<'a> {
    texts: &'a mut HashMap<usize, String>,
    set: &'a mut HashMap<usize, bool>,
    colors: &'a mut HashMap<usize, Color>,
    sprites: &'a mut HashMap<usize, Picture>,
    groups: &'a mut HashMap<usize, f32>,
}

/// An element's preferred size set, its least and its share of what is
/// left kept.
pub(crate) fn prefer_size(panel: &mut trp_data::screen::ScenePanel, index: usize, size: [f32; 2]) {
    let element = panel.nodes[index].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[index].element = Some(trp_data::screen::Element { preferred: size, ..element });
}

/// Words cut to what of them fits a width (units) as TextMeshPro cuts a
/// line that does not break (`m_overflowMode`): truncated (3), the letters
/// past the width left out; with an ellipsis (1), as many as leave room
/// for "…", and it; any other mode, or words that fit, as they are. Its
/// margins come off the width.
pub(crate) fn fitted_words(font: &trp_data::screen::TmpFont, text: &trp_data::screen::TextLook, words: &str, width: f32) -> String {
    let room = width - text.margin[0] - text.margin[2];
    let bold = text.style & 1 != 0;
    let said = if text.style & 16 != 0 { words.to_uppercase() } else { words.to_owned() };
    // TextMeshPro tests a line against its room at its visible letters
    // only, so a trailing space never cuts it.
    let wide = |words: &str| font.width(words.trim_end(), text.size, text.spacing, bold);
    if text.wraps || !matches!(text.overflow, 1 | 3) || wide(&said) <= room {
        return said;
    }
    let tail = if text.overflow == 1 { "\u{2026}" } else { "" };
    let mut kept: Vec<char> = said.chars().collect();
    while !kept.is_empty() && wide(&format!("{}{tail}", kept.iter().collect::<String>().trim_end())) > room {
        kept.pop();
    }
    format!("{}{tail}", kept.iter().collect::<String>())
}

/// A text's size as its font draws it, its margins with it.
pub(crate) fn text_size(look: &HealthLook, panel: &trp_data::screen::ScenePanel, index: usize, said: &str) -> Option<[f32; 2]> {
    let text = panel.nodes[index].text.as_ref()?;
    let face = look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?;
    Some([face.font.width(said, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2], face.font.height(text.size) + text.margin[1] + text.margin[3]])
}

/// A copy of one of the panel's own objects and all under it put last
/// under `holder`, the original left undrawn; the place of the copy's top.
pub(crate) fn copy_within(panel: &mut trp_data::screen::ScenePanel, drawn: &mut Drawn, top: usize, holder: usize) -> usize {
    let under = |index: usize, nodes: &[trp_data::screen::PanelNode]| {
        let mut at = Some(index);
        while let Some(here) = at {
            if here == top {
                return true;
            }
            at = nodes[here].parent;
        }
        false
    };
    let copied: Vec<usize> = (0..panel.nodes.len()).filter(|&index| under(index, &panel.nodes)).collect();
    let first = panel.nodes.len();
    let new_of = |old: usize| copied.iter().position(|&i| i == old).map(|at| first + at);
    let holder_path = panel.nodes[holder].path.clone();
    for &old in &copied {
        let mut node = panel.nodes[old].clone();
        node.parent = if old == top { Some(holder) } else { node.parent.and_then(new_of) };
        node.active |= old == top;
        node.path = format!("{holder_path}/#{first}/{}", node.path);
        panel.nodes.push(node);
        drawn.shown.push(drawn.shown.get(old).copied().unwrap_or(Shown::Never));
        drawn.pictures.push(drawn.pictures.get(old).copied().flatten());
    }
    first
}

/// A weapon's mastering (`ItemSpecificationPanel.method_1`,
/// `MasteringCharacteristicPanel.SetValues`): its row named
/// "WEAPON MASTERING (0N)" with "points/points the level takes" and its
/// fill to their share; its badge under the preview with "0N", and the
/// preview's corner beside it; none of it for anything else.
fn info_mastering(look: &HealthLook, panel: &mut trp_data::screen::ScenePanel, marks: &mut PartsMarks, mastering: Option<&InfoMastering>) {
    for path in [INFO_MASTERING, INFO_MASTERING_BADGE, INFO_CORNER] {
        if let Some(index) = panel.find(path) {
            marks.set.insert(index, mastering.is_some());
            panel.nodes[index].active |= mastering.is_some();
        }
    }
    let Some(mastering) = mastering else { return };
    let level = format!("0{}", mastering.level + 1);
    let at = |panel: &trp_data::screen::ScenePanel, end: &str| panel.find(&format!("{INFO_MASTERING}/{end}"));
    if let Some(name) = at(panel, "Name") {
        marks.texts.insert(name, format!("WEAPON MASTERING ({level})"));
    }
    if let Some(value) = at(panel, "Value") {
        marks.texts.insert(value, format!("{}/{}", trimmed_figure(mastering.points), mastering.most.round()));
    }
    let share = inverse_lerp(0.0, mastering.most, mastering.points);
    if let Some(full) = at(panel, "bars/Full") {
        panel.nodes[full].layout.anchor_max[0] = share;
    }
    if let Some(delta) = at(panel, "bars/Delta") {
        panel.nodes[delta].layout.anchor_min[0] = share;
        panel.nodes[delta].layout.anchor_max[0] = share;
    }
    // The badge: its icon and its level at their own sizes in its row.
    let badge = format!("{INFO_MASTERING_BADGE}/MasteringText");
    if let Some(text) = panel.find(&badge) {
        let size = text_size(look, panel, text, &level).unwrap_or([20.0, 15.0]);
        prefer_size(panel, text, size);
        marks.texts.insert(text, level);
    }
    if let Some(icon) = panel.find(&format!("{INFO_MASTERING_BADGE}/MasteringIcon")) {
        let size = panel.nodes[icon].image.and_then(|_| Some(panel.nodes[icon].layout.size_delta)).filter(|size| size[0] > 0.0).unwrap_or([19.0, 25.0]);
        prefer_size(panel, icon, size);
    }
}

/// A figure as C# writes a float (`ToString()`): no decimals where it is
/// whole.
pub(crate) fn trimmed_figure(value: f32) -> String {
    let written = format!("{value}");
    written.strip_suffix(".0").map(str::to_owned).unwrap_or(written)
}

/// The window's buttons (`InteractionButtonsContainer.Show`, the thing's
/// lines but Inspect): a copy of a template for each (Discard's own, the
/// wish list's own, the rest's), its caption at its font's width, its icon,
/// its arrow where it opens lines of its own, at 0.3 where it cannot be
/// done; in the container's grid (`GridLayoutGroup`: three columns, filled
/// row by row from the top left, the block in its middle), the container
/// as large as the grid asks. The places of the copies' tops, each with its
/// ground's node, colours and alpha (`light_info_buttons`).
#[allow(clippy::type_complexity)]
fn info_buttons(look: &HealthLook, panel: &mut trp_data::screen::ScenePanel, drawn: &mut Drawn, marks: &mut PartsMarks, buttons: &[InfoButtonLook]) -> Vec<(usize, Option<(usize, [Color; 2], f32)>)> {
    let Some(holder) = panel.find(INFO_BUTTONS) else { return Vec::new() };
    let templates: Vec<Option<usize>> = INFO_BUTTON_TEMPLATES.iter().map(|path| panel.find(path)).collect();
    let mut tops = Vec::new();
    for button in buttons {
        let template = match button.name {
            "Discard" => templates[2],
            "AddToWishlist" => templates[1],
            _ => templates[0],
        };
        let Some(template) = template else { continue };
        let first = copy_within(panel, drawn, template, holder);
        let named = |panel: &trp_data::screen::ScenePanel, name: &str| (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == name);
        if let Some(text) = named(panel, "Text") {
            let caption = button.caption.to_uppercase();
            let width = text_size(look, panel, text, &caption).map_or(0.0, |size| size[0]);
            let flexible = panel.nodes[text].element.map_or([1.0, -1.0], |element| element.flexible);
            panel.nodes[text].element = Some(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [width, -1.0], flexible });
            marks.texts.insert(text, caption);
        }
        if let (Some(icon), Some(picture)) = (named(panel, "Icon"), look.menu_icons.get(button.name)) {
            marks.sprites.insert(icon, *picture);
        }
        if let Some(arrow) = named(panel, "SubmenuArrow") {
            if button.arrow {
                let size = drawn.pictures.get(arrow).copied().flatten().map_or([9.0, 11.0], |picture| picture.size);
                panel.nodes[arrow].element = Some(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: size, flexible: [-1.0, -1.0] });
            } else {
                marks.set.insert(arrow, false);
            }
        }
        if !button.enabled {
            marks.groups.insert(first, 0.3);
        }
        let icon_holder = named(panel, "IconPlaceholder").and_then(|index| panel.nodes[index].parent);
        let ground = (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Background" && panel.nodes[index].parent == icon_holder);
        let line_colors = panel.script(first, "ContextMenuButton").and_then(menu_line_colors);
        if let (Some(ground), Some([normal, _])) = (ground, line_colors) {
            marks.colors.insert(ground, normal);
        }
        tops.push((first, ground.zip(line_colors).map(|(ground, colors)| (ground, colors, if button.enabled { 1.0 } else { 0.3 }))));
    }
    // The templates themselves are not drawn.
    for template in templates.into_iter().flatten() {
        marks.set.insert(template, false);
    }
    let Some(grid) = panel.grid(holder) else { return tops };
    let columns = grid.columns.unwrap_or(3).max(1) as usize;
    let count = tops.len();
    let rows = count.div_ceil(columns);
    let [cell_w, cell_h] = grid.cell;
    let [space_w, space_h] = grid.spacing;
    let width = grid.padding[0] + grid.padding[1] + columns as f32 * (cell_w + space_w) - space_w;
    let height = if rows == 0 { 0.0 } else { grid.padding[2] + grid.padding[3] + rows as f32 * (cell_h + space_h) - space_h };
    let shown_columns = columns.min(count.max(1));
    let block = shown_columns as f32 * (cell_w + space_w) - space_w;
    let start = grid.padding[0] + (width - grid.padding[0] - grid.padding[1] - block) * (grid.alignment % 3) as f32 * 0.5;
    for (n, &(top, _)) in tops.iter().enumerate() {
        let layout = &mut panel.nodes[top].layout;
        layout.anchor_min = [0.0, 1.0];
        layout.anchor_max = [0.0, 1.0];
        layout.pivot = [0.0, 1.0];
        layout.offset = [start + (n % columns) as f32 * (cell_w + space_w), -(grid.padding[2] + (n / columns) as f32 * (cell_h + space_h))];
        layout.size_delta = [cell_w, cell_h];
    }
    let element = panel.nodes[holder].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[holder].element = Some(trp_data::screen::Element { min: [width, height], preferred: [width, height], ..element });
    tops
}

/// The parts' panel (`CreateModSlots`): on for a thing with slots, a copy
/// of the slot's view for each (`ModSlotView`), full or empty as its slot
/// is (`SetSlotGraphics`: the full border, or the empty border, the ground,
/// the slot's picture at its own size, its name and its filters' marks),
/// in the grid (`GridLayoutGroup`: 64 square, 3 apart, as many to a row as
/// fit, the block in the middle of the top), the grid as tall as its rows;
/// the window at least as wide as six slots, or a third of them, ask with
/// the paddings about them. The places of the copies' tops, and each empty
/// slot's filters' row with its marks' pictures.
#[allow(clippy::type_complexity)]
fn info_slots(look: &HealthLook, panel: &mut trp_data::screen::ScenePanel, drawn: &mut Drawn, marks: &mut PartsMarks, slots: &[InfoSlot]) -> (Vec<usize>, Vec<(usize, Vec<Picture>)>) {
    let (Some(mods), Some(content)) = (panel.find(INFO_MODS), panel.find(INFO_MODS_CONTENT)) else { return (Vec::new(), Vec::new()) };
    let template = look.info_rows.as_ref().and_then(|rows| rows.mod_slot.as_ref());
    let (false, Some(template)) = (slots.is_empty(), template) else {
        marks.set.insert(mods, false);
        return (Vec::new(), Vec::new());
    };
    panel.nodes[mods].active = true;
    marks.set.insert(mods, true);
    // (Its scrollbar is never wanted: the rows take as many as fit.)
    if let Some(bar) = panel.find(INFO_MODS_BAR) {
        marks.set.insert(bar, false);
    }
    let mut tops = Vec::new();
    let mut filters = Vec::new();
    for slot in slots {
        let top = graft_row(panel, drawn, template, content);
        let at = |end: &str| template.0.find(end).map(|index| top + index);
        for (end, on) in [("CanvasGroup/Empty Border", !slot.full), ("CanvasGroup/Full Border", slot.full), ("CanvasGroup/Background", !slot.full), ("CanvasGroup/Text", !slot.full), ("CanvasGroup/filters", !slot.full)] {
            if let Some(index) = at(end) {
                marks.set.insert(index, on);
            }
        }
        if let Some(text) = at("CanvasGroup/Text") {
            // What of it fits the slot (its text's ellipsis: "…" where a
            // name is wider than the slot, `ModSlotView._slotName`).
            let fitted = template.0.find("CanvasGroup/Text").and_then(|index| {
                let look_of = template.0.nodes[index].text.as_ref()?;
                let face = look.tmp_faces.get(&(template.0.file_of(look_of.font.0)?, look_of.font.1))?;
                Some(fitted_words(&face.font, look_of, &slot.name, template.0.rect(index, [64.0, 64.0])[2]))
            });
            marks.texts.insert(text, fitted.unwrap_or_else(|| slot.name.clone()));
        }
        // Its picture, at its own size in the middle (`SetSlotBackImage`).
        if let Some(back) = at("CanvasGroup/Background Image") {
            let picture = look.slot_pictures.get(&slot.picture).or_else(|| slot.armour.then(|| look.slot_pictures.get("armor_plate")).flatten()).copied();
            match picture.filter(|_| !slot.full) {
                Some(picture) => {
                    marks.set.insert(back, true);
                    marks.sprites.insert(back, picture);
                    panel.nodes[back].layout.size_delta = picture.size;
                }
                None => {
                    marks.set.insert(back, false);
                }
            }
        }
        if let (false, Some(row)) = (slot.full, at("CanvasGroup/filters")) {
            let pictures: Vec<Picture> = slot.marks.iter().filter_map(|mark| look.mod_pictures.get(&format!("mod_{mark}")).copied()).collect();
            if !pictures.is_empty() {
                filters.push((row, pictures));
            }
        }
        tops.push(top);
    }
    let size = fitted_size(panel);
    let Some(grid) = panel.grid(content) else { return (tops, filters) };
    let width = panel.rect(content, size)[2];
    let [cell_w, cell_h] = grid.cell;
    let [space_w, space_h] = grid.spacing;
    let inner = width - grid.padding[0] - grid.padding[1];
    let columns = (((inner + space_w) / (cell_w + space_w)).floor() as usize).max(1);
    let count = tops.len();
    let shown_columns = columns.min(count);
    let block = shown_columns as f32 * (cell_w + space_w) - space_w;
    let start = grid.padding[0] + (inner - block) * (grid.alignment % 3) as f32 * 0.5;
    for (n, &top) in tops.iter().enumerate() {
        let layout = &mut panel.nodes[top].layout;
        layout.anchor_min = [0.0, 1.0];
        layout.anchor_max = [0.0, 1.0];
        layout.pivot = [0.0, 1.0];
        layout.offset = [start + (n % columns) as f32 * (cell_w + space_w), -(grid.padding[2] + (n / columns) as f32 * (cell_h + space_h))];
        layout.size_delta = [cell_w, cell_h];
    }
    let rows = count.div_ceil(columns);
    let height = grid.padding[2] + grid.padding[3] + rows as f32 * (cell_h + space_h) - space_h;
    let element = panel.nodes[content].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[content].element = Some(trp_data::screen::Element { min: [element.min[0], height], preferred: [element.preferred[0], height], ..element });
    // The window's least width (`_layoutElement.minWidth`): the paddings
    // of the groups from the grid up to the window, and six slots or a
    // third of them, whichever is more.
    let mut padding = grid.padding[0] + grid.padding[1];
    let mut above = panel.nodes[content].parent;
    while let Some(index) = above {
        padding += panel.nodes[index].group.map_or(0.0, |group| group.padding[0] + group.padding[1]);
        above = panel.nodes[index].parent;
    }
    let least_columns = count.div_ceil(3).max(6) as f32;
    let least = padding + cell_w * least_columns + space_w * (least_columns - 1.0);
    let root = panel.nodes[0].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    panel.nodes[0].element = Some(trp_data::screen::Element { min: [least, root.min[1]], ..root });
    (tops, filters)
}

/// A panel's top's size as its fitter makes it (`ContentSizeFitter`:
/// least 1, preferred 2, by axis), else as it is kept.
fn fitted_size(panel: &trp_data::screen::ScenePanel) -> [f32; 2] {
    let mut size = info_size(panel);
    if let Some(fitter) = panel.script(0, "ContentSizeFitter") {
        for (axis, kept) in size.iter_mut().enumerate() {
            let mode = fitter.get(4 * axis..4 * axis + 4).map_or(0, |word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
            let (least, preferred, _) = panel.asks(0, axis);
            match mode {
                1 => *kept = least,
                2 => *kept = preferred,
                _ => {}
            }
        }
    }
    size
}

/// Mathf.InverseLerp: where a figure lies between two, held within 0 and
/// 1 (0 where they are the same).
fn inverse_lerp(from: f32, to: f32, value: f32) -> f32 {
    if from == to { 0.0 } else { ((value - from) / (to - from)).clamp(0.0, 1.0) }
}

/// A copy of one of the window's row templates put last under `holder`,
/// its objects after the panel's own, numbered apart by their place; the
/// place of the copy's top.
fn graft_row(panel: &mut trp_data::screen::ScenePanel, drawn: &mut Drawn, (source, source_drawn): &(trp_data::screen::ScenePanel, Drawn), holder: usize) -> usize {
    let first = panel.nodes.len();
    let under = panel.nodes[holder].path.clone();
    for (old, node) in source.nodes.iter().enumerate() {
        let mut node = node.clone();
        // Its texts' fonts and materials and its picture count in the
        // source's table of files: put them in the panel's (a slot's
        // template is `resources.assets`', the window `sharedassets49`'s).
        if let Some(text) = node.text.as_mut() {
            text.font.0 = rebase_file(panel, source, text.font.0);
            if let Some(material) = text.material.as_mut() {
                material.0 = rebase_file(panel, source, material.0);
            }
        }
        if let Some(sprite) = node.image.as_mut().and_then(|image| image.sprite.as_mut()) {
            sprite.0 = rebase_file(panel, source, sprite.0);
        }
        node.path = format!("{under}/#{first}/{}", node.path);
        node.parent = Some(node.parent.map_or(holder, |parent| first + parent));
        node.active |= old == 0;
        panel.nodes.push(node);
        drawn.shown.push(source_drawn.shown.get(old).copied().unwrap_or(Shown::Never));
        drawn.pictures.push(source_drawn.pictures.get(old).copied().flatten());
    }
    first
}

/// A file's number in `source`'s table of files (0 its own, else one past
/// its place among its outside files) as `panel` counts it, the file added
/// to `panel`'s outside files where it has it not.
fn rebase_file(panel: &mut trp_data::screen::ScenePanel, source: &trp_data::screen::ScenePanel, index: i32) -> i32 {
    let Some(name) = source.file_of(index) else { return index };
    if name == panel.file {
        return 0;
    }
    if let Some(at) = panel.externals.iter().position(|outside| outside.rsplit('/').next() == Some(name.as_str())) {
        return at as i32 + 1;
    }
    let full = if index == 0 { source.file.clone() } else { source.externals.get(index as usize - 1).cloned().unwrap_or(name) };
    panel.externals.push(full);
    panel.externals.len() as i32
}

/// The thing's figures put into the window (`ItemSpecificationPanel.
/// RecreateAttributeBars`, `CompactCharacteristicPanel.Show` and
/// `SetValues`, `CharacteristicPanel.SetValues`, `DurabilityPanel.
/// SetValues`, `ProcessCompactCells`): a weapon's durability row; a bar row
/// in the column of bars for each bar, a compact or a dropdown row in the
/// compact grid for the rest, each with its icon at its own place (none
/// where it has none), its name and its value at the widths their font
/// draws them, coloured by its sign where it is so shown; each bar's fill
/// and change; the grid in two columns, or one where a row's words would
/// not fit its cell, filled down each column first.
#[allow(clippy::too_many_arguments)]
fn info_rows(look: &HealthLook, panel: &mut trp_data::screen::ScenePanel, drawn: &mut Drawn, texts: &mut HashMap<usize, String>, set: &mut HashMap<usize, bool>, colors: &mut HashMap<usize, Color>, sprites: &mut HashMap<usize, Picture>, figures: &trp_sim::item_attributes::Figures) -> Vec<(usize, usize)> {
    let mut tips = Vec::new();
    use trp_sim::item_attributes::Show;
    // A text's size as its font draws it, its margins with it
    // (`TMP_Text.preferredWidth` and `preferredHeight`), which the row's
    // layout gives it.
    let size_of = |panel: &trp_data::screen::ScenePanel, index: usize, said: &str| -> Option<[f32; 2]> {
        let text = panel.nodes[index].text.as_ref()?;
        let face = look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?;
        Some([face.font.width(said, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2], face.font.height(text.size) + text.margin[1] + text.margin[3]])
    };
    let prefer = |panel: &mut trp_data::screen::ScenePanel, index: usize, size: [f32; 2]| {
        let element = panel.nodes[index].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[index].element = Some(trp_data::screen::Element { preferred: size, ..element });
    };
    let off = |panel: &mut trp_data::screen::ScenePanel, drawn: &mut Drawn, index: usize| {
        panel.nodes[index].active = false;
        drawn.shown[index] = Shown::Never;
    };
    // The weapon's durability row: "present/greatest (the template's)",
    // the greatest red under 10; its fill and border in the low colour at
    // 15% of the template's or under; its change to the greatest's share.
    // (With none, the row and all under it are not drawn: its pieces are
    // drawn apart, and its being off does not hide them.)
    if let (None, Some(row)) = (figures.durability, panel.find(INFO_DURABILITY)) {
        let path = format!("{}/", panel.nodes[row].path);
        for index in 0..panel.nodes.len() {
            if index == row || panel.nodes[index].path.starts_with(&path) {
                drawn.shown[index] = Shown::Never;
            }
        }
    }
    if let (Some(durability), Some(row)) = (figures.durability, panel.find(INFO_DURABILITY)) {
        panel.nodes[row].active = true;
        set.insert(row, true);
        let path = panel.nodes[row].path.clone();
        let at = |panel: &trp_data::screen::ScenePanel, end: &str| panel.find(&format!("{path}/{end}"));
        let round = |value: f32| ((value as f64 * 10.0).round() / 10.0) as f32;
        let tint = if durability.most < 10.0 { "#FF0000" } else { "#FFFFFF" };
        let said = format!("{}/<color={tint}>{}</color> ({})", round(durability.now), round(durability.most), durability.peak.round());
        if let Some(value) = at(panel, "Value") {
            texts.insert(value, said);
        }
        if let Some(name) = at(panel, "Name") {
            texts.insert(name, figures.durability_name.clone());
        }
        let share = durability.now / durability.peak.max(1e-6);
        let look_of = trp_data::screen::row_look(&panel.subtree(row), "DurabilityPanel").and_then(|look| look.bar);
        if let Some(bars) = look_of {
            let color = rgba(if share > 0.15 { bars.current } else { bars.low });
            for end in ["bars/Full", "bars/Border"] {
                if let Some(index) = at(panel, end) {
                    colors.insert(index, color);
                }
            }
        }
        if let Some(full) = at(panel, "bars/Full") {
            panel.nodes[full].layout.anchor_max[0] = share;
        }
        if let Some(delta) = at(panel, "bars/Delta") {
            panel.nodes[delta].layout.anchor_max[0] = inverse_lerp(0.0, durability.peak, durability.most);
        }
        if let Some(compare) = at(panel, "bars/CompareDelta") {
            off(panel, drawn, compare);
        }
    }
    let Some(rows) = look.info_rows.as_ref() else { return tips };
    let (Some(bars), Some(grid)) = (panel.find(INFO_BARS), panel.find(INFO_COMPACT)) else { return tips };
    // The compact rows' widths of their words, for the columns.
    let mut compact_widths = Vec::new();
    let mut cells = Vec::new();
    for (number, row) in figures.rows.iter().enumerate() {
        let (template, row_look) = match row.show {
            Show::Bar { .. } => (&rows.bar, &rows.bar_look),
            Show::Compact => (&rows.compact, &rows.compact_look),
            Show::Dropdown => (&rows.dropdown, &rows.compact_look),
        };
        let holder = if matches!(row.show, Show::Bar { .. }) { bars } else { grid };
        let top = graft_row(panel, drawn, template, holder);
        let at = |end: &str| template.0.find(end).map(|index| top + index);
        if let (Show::Dropdown, Some(hover), false) = (&row.show, at("Hover"), row.tip.is_empty()) {
            tips.push((hover, number));
        }
        let mut words_width = 0.0;
        if let Some(name) = at("Name") {
            let size = size_of(panel, name, &row.name).unwrap_or_default();
            prefer(panel, name, size);
            words_width += size[0];
            texts.insert(name, row.name.clone());
        }
        if let Some(value) = at("Value") {
            match &row.value {
                // (`Colored`: "+" before a positive one; "-" before a
                // negative one that has none; each its colour.)
                Some(said) => {
                    let mut said = said.clone();
                    if let Some(base) = row.colored {
                        let (good, bad) = (rgba(row_look.increasing), rgba(row_look.decreasing));
                        if base < 0.0 {
                            colors.insert(value, if row.less_is_good { good } else { bad });
                            if !said.contains('-') {
                                said.insert(0, '-');
                            }
                        } else if base > 0.0 {
                            colors.insert(value, if row.less_is_good { bad } else { good });
                            said.insert(0, '+');
                        }
                    }
                    // (Measured as drawn: a colour tag takes no room.)
                    let drawn_words = rich_spans(&said).map_or_else(|| said.clone(), |spans| spans.into_iter().map(|(words, _)| words).collect());
                    let size = size_of(panel, value, &drawn_words).unwrap_or_default();
                    prefer(panel, value, size);
                    if !said.is_empty() {
                        words_width += size[0] + 5.0;
                    }
                    panel.nodes[value].active = true;
                    texts.insert(value, said);
                }
                _ => off(panel, drawn, value),
            }
        }
        // Its icon: the attribute's, else none (`Icon.SetActive`).
        if let Some(icon) = at("Icon") {
            use trp_sim::item_attributes::Table;
            let table = match row.table {
                Table::Attribute => &look.attribute_icons,
                Table::Health => &look.figure_icons[0],
                Table::Damage => &look.figure_icons[1],
                Table::Buff => &look.figure_icons[2],
            };
            match table.get(row.id) {
                Some(picture) => {
                    sprites.insert(icon, *picture);
                }
                None => off(panel, drawn, icon),
            }
        }
        if let Show::Bar { range, base, delta } = row.show {
            let change = base * delta;
            let place = |value: f32| inverse_lerp(range[0], range[1], value);
            let (total, own) = (place(base + change), place(base));
            if let Some(full) = at("bars/Full") {
                panel.nodes[full].layout.anchor_max[0] = total.min(own);
            }
            if let Some(index) = at("bars/Delta") {
                panel.nodes[index].layout.anchor_min[0] = total.min(own);
                panel.nodes[index].layout.anchor_max[0] = total.max(own);
                let inverted = range[0] > range[1];
                let lowers = (change < 0.0 && !inverted) || (change > 0.0 && inverted);
                if let Some(picture) = if lowers { rows.sub } else { rows.add } {
                    sprites.insert(index, picture);
                }
                if let Some(bar) = &row_look.bar {
                    let better = if row.less_is_good { change <= 0.0 } else { change > 0.0 };
                    colors.insert(index, rgba(if better { bar.add } else { bar.sub }));
                }
            }
        } else {
            if row.show == Show::Compact {
                compact_widths.push(words_width);
            }
            cells.push((row.show == Show::Dropdown, top));
        }
    }
    // (The dropdown rows are made after all the compact rows, and follow
    // them in the grid.)
    cells.sort_by_key(|(dropdown, _)| *dropdown);
    let cells: Vec<usize> = cells.into_iter().map(|(_, top)| top).collect();
    // The grid: its columns (`ProcessCompactCells`: two, unless 30 and a
    // compact row's words are wider than a cell of two), each cell as wide
    // as its columns share the grid (`FlexibleGridLayoutGroup`), the rows
    // filled down each column first (`GridLayoutGroup`, the vertical axis
    // first, from the top left), the grid as tall as its rows.
    let Some(figures_of) = panel.grid(grid) else { return tips };
    let width = panel.rect(grid, fitted_size(panel))[2];
    let two = (width - figures_of.spacing[0] - figures_of.padding[0] - figures_of.padding[1]) / 2.0;
    let columns = if compact_widths.iter().any(|&words| 30.0 + words > two) { 1 } else { figures_of.columns.unwrap_or(2).max(1) as usize };
    let cell_width = (width - (columns as f32 - 1.0) * figures_of.spacing[0] - figures_of.padding[0] - figures_of.padding[1]) / columns as f32;
    let count = cells.len();
    let down = if count > columns { count.div_ceil(columns) } else { 1 };
    for (n, &top) in cells.iter().enumerate() {
        let (x, y) = if figures_of.start_axis == 1 { (n / down, n % down) } else { (n % columns, n / columns) };
        let layout = &mut panel.nodes[top].layout;
        layout.anchor_min = [0.0, 1.0];
        layout.anchor_max = [0.0, 1.0];
        layout.pivot = [0.0, 1.0];
        layout.offset = [figures_of.padding[0] + (cell_width + figures_of.spacing[0]) * x as f32, -(figures_of.padding[2] + (figures_of.cell[1] + figures_of.spacing[1]) * y as f32)];
        layout.size_delta = [cell_width, figures_of.cell[1]];
    }
    let rows_down = count.div_ceil(columns);
    let height = figures_of.padding[2] + figures_of.padding[3] + (figures_of.cell[1] + figures_of.spacing[1]) * rows_down as f32 - figures_of.spacing[1];
    if height > 0.0 {
        let element = panel.nodes[grid].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[grid].element = Some(trp_data::screen::Element { min: [element.min[0], height], preferred: [element.preferred[0], height], ..element });
    }
    tips
}


/// The edit build screen's characteristics panel laid out (`EbStatsLook`
/// filled by `eb_stats_built`): what `draw_eb_stats` draws, its size, the
/// places of its switcher and its INFO button (each folds or unfolds it),
/// and each bar's comparison piece with its colour, which pulses.
pub(crate) struct EbStatsBuilt {
    panel: trp_data::screen::ScenePanel,
    drawn: Drawn,
    texts: HashMap<usize, String>,
    set: HashMap<usize, bool>,
    colors: HashMap<usize, Color>,
    sprites: HashMap<usize, Picture>,
    pub(crate) size: [f32; 2],
    pub(crate) switches: Vec<[f32; 4]>,
    pub(crate) pulse: Vec<(usize, Color)>,
}

/// The base the characteristics panel's pieces are numbered from.
pub(crate) const EB_STATS: usize = 23 << 20;

/// The comparison's alpha at a time (`CharacteristicPanel.Update`: 0.2 and
/// a ping-pong of 0.4, gone through at 0.8 a second: from 0.2 to 0.6 in
/// half a second and back in the next).
pub(crate) fn compare_alpha(seconds: f32) -> f32 {
    let travelled = seconds / 0.5 * 0.4;
    let phase = travelled % 0.8;
    0.2 + if phase < 0.4 { phase } else { 0.8 - phase }
}

/// One bar of the panel (`CharacteristicPanel.SetValues`): its fill to the
/// lesser of the figure with and without its change, the change to the
/// other; against the figure compared with (the build as the screen was
/// given it, or with the part the mouse rests on in its slot), the
/// difference drawn in the comparison's piece in the adding or the
/// subtracting colour, and the change's difference written after the value
/// in brackets.
#[allow(clippy::too_many_arguments)]
fn eb_bar(panel: &mut trp_data::screen::ScenePanel, at: &dyn Fn(&trp_data::screen::ScenePanel, &str) -> Option<usize>, (range, base, delta, less_is_good): ([f32; 2], f32, f32, bool), compare: Option<(f32, f32)>, row_look: &trp_data::screen::RowLook, (add, sub): (Option<Picture>, Option<Picture>), (colors, sprites, set): (&mut HashMap<usize, Color>, &mut HashMap<usize, Picture>, &mut HashMap<usize, bool>), pulse: &mut Vec<(usize, Color)>) -> Option<String> {
    let place = |value: f32| inverse_lerp(range[0], range[1], value);
    let inverted = range[0] > range[1];
    let (own_change, (cbase, cdelta)) = (base * delta, compare.unwrap_or((base, delta)));
    let total = base + own_change;
    let (compare_change, compare_total) = (cbase * cdelta, cbase + cbase * cdelta);
    let difference = compare_total - total;
    let (n8, n9, n11, b) = (place(total), place(base), place(compare_total), place(cbase));
    let (n10, x) = (n8.min(n9), n8.max(n9));
    let (b2, n12) = (n11.min(b), (n11 - n8).abs());
    let (full, delta_piece, compare_piece) = (at(panel, "bars/Full"), at(panel, "bars/Delta"), at(panel, "bars/CompareDelta"));
    if let Some(piece) = delta_piece {
        let lowers = (own_change < 0.0 && !inverted) || (own_change > 0.0 && inverted);
        if let Some(picture) = if lowers { sub } else { add } {
            sprites.insert(piece, picture);
        }
        if let Some(bar) = &row_look.bar {
            let better = if less_is_good { own_change <= 0.0 } else { own_change > 0.0 };
            colors.insert(piece, rgba(if better { bar.add } else { bar.sub }));
        }
    }
    let anchor = |panel: &mut trp_data::screen::ScenePanel, piece: Option<usize>, from: Option<f32>, to: f32| {
        if let Some(piece) = piece {
            if let Some(from) = from {
                panel.nodes[piece].layout.anchor_min[0] = from;
            }
            panel.nodes[piece].layout.anchor_max[0] = to;
        }
    };
    if difference.abs() > f32::EPSILON && compare_piece.is_some() {
        let piece = compare_piece.unwrap_or_default();
        set.insert(piece, true);
        if let Some(picture) = add {
            sprites.insert(piece, picture);
        }
        if let Some(bar) = &row_look.bar {
            let better = if less_is_good { difference <= 0.0 } else { difference > 0.0 };
            let color = rgba(if better { bar.add } else { bar.sub });
            colors.insert(piece, color.with_alpha(compare_alpha(0.0)));
            pulse.push((piece, color));
        }
        if difference > 0.0 {
            anchor(panel, full, None, n10.min(b2));
            anchor(panel, delta_piece, Some(n10.min(b2)), n8.min(n11));
            anchor(panel, compare_piece, Some(n8.min(n11)), n8.max(n11));
        } else {
            anchor(panel, full, None, n10.min(b2));
            if compare_total < base {
                anchor(panel, compare_piece, Some(n10.min(b2)), n10.min(b2) + n12);
                anchor(panel, delta_piece, Some(n10.min(b2) + n12), n8.max(n11));
            } else {
                anchor(panel, delta_piece, Some(n10.min(b2)), n8.max(n11) - n12);
                anchor(panel, compare_piece, Some(n8.max(n11) - n12), n8.max(n11));
            }
        }
    } else {
        if let Some(piece) = compare_piece {
            set.insert(piece, false);
        }
        anchor(panel, full, None, n10);
        anchor(panel, delta_piece, Some(n10), x);
    }
    let more = compare_change - own_change;
    (more.abs() > f32::EPSILON).then(|| format!("({}{})", if more > 0.0 { "+" } else { "" }, more.round() as i64))
}

/// The edit build screen's characteristics panel for a build's figures,
/// against those of the build it is compared with (`CharacteristicsPanel.
/// InstantiateItemFields`): its durability row (compared with nothing), its
/// weight (over 0 to 15 kg, less better, compared), then in one column each
/// bar and each compact row in the figures' order; folded to its INFO
/// button, or not (`SetState`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn eb_stats_built(look: &HealthLook, figures: &trp_sim::item_attributes::Figures, compare: &trp_sim::item_attributes::Figures, (weight, compare_weight): (f32, f32), weight_name: &str, expanded: bool) -> Option<EbStatsBuilt> {
    use trp_sim::item_attributes::{Show, Table};
    let stats = look.edit_build.as_ref()?.stats.as_ref()?;
    let mut panel = stats.panel.clone();
    let mut drawn = stats.drawn.clone();
    let (mut texts, mut set, mut colors, mut sprites, mut pulse) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new(), Vec::new());
    let (add, sub) = look.info_rows.as_ref().map_or((None, None), |rows| (rows.add, rows.sub));
    for (path, on) in [("CharacteristicsExpanded", expanded), ("InfoButton", !expanded)] {
        if let Some(index) = panel.find(path) {
            set.insert(index, on);
            panel.nodes[index].active = on;
        }
    }
    let size_of = |panel: &trp_data::screen::ScenePanel, index: usize, said: &str| -> Option<[f32; 2]> {
        let text = panel.nodes[index].text.as_ref()?;
        let face = look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?;
        Some([face.font.width(said, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2], face.font.height(text.size) + text.margin[1] + text.margin[3]])
    };
    let prefer = |panel: &mut trp_data::screen::ScenePanel, index: usize, size: [f32; 2]| {
        let element = panel.nodes[index].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[index].element = Some(trp_data::screen::Element { preferred: size, ..element });
    };
    let within = |top: String| move |panel: &trp_data::screen::ScenePanel, end: &str| panel.find(&format!("{top}/{end}"));
    // The durability row, as the inspect window's.
    match (figures.durability, panel.find("CharacteristicsExpanded/Durability")) {
        (Some(durability), Some(row)) => {
            let at = within(panel.nodes[row].path.clone());
            let round = |value: f32| ((value as f64 * 10.0).round() / 10.0) as f32;
            let tint = if durability.most < 10.0 { "#FF0000" } else { "#FFFFFF" };
            if let Some(value) = at(&panel, "Value") {
                texts.insert(value, format!("{}/<color={tint}>{}</color> ({})", round(durability.now), round(durability.most), durability.peak.round()));
            }
            if let Some(name) = at(&panel, "Name") {
                texts.insert(name, figures.durability_name.clone());
            }
            let share = durability.now / durability.peak.max(1e-6);
            if let Some(bars) = trp_data::screen::row_look(&panel.subtree(row), "DurabilityPanel").and_then(|look| look.bar) {
                let color = rgba(if share > 0.15 { bars.current } else { bars.low });
                for end in ["bars/Full", "bars/Border"] {
                    if let Some(index) = at(&panel, end) {
                        colors.insert(index, color);
                    }
                }
            }
            if let Some(full) = at(&panel, "bars/Full") {
                panel.nodes[full].layout.anchor_max[0] = share;
            }
            if let Some(delta) = at(&panel, "bars/Delta") {
                panel.nodes[delta].layout.anchor_max[0] = inverse_lerp(0.0, durability.peak, durability.most);
            }
            if let Some(compare) = at(&panel, "bars/CompareDelta") {
                set.insert(compare, false);
            }
        }
        (None, Some(row)) => {
            set.insert(row, false);
            panel.nodes[row].active = false;
        }
        _ => {}
    }
    // The weight (`_charWeight`): to three places.
    if let Some(row) = panel.find("CharacteristicsExpanded/Weight") {
        let at = within(panel.nodes[row].path.clone());
        let mut said = format!("{weight:.3}");
        if let Some(name) = at(&panel, "Name") {
            texts.insert(name, weight_name.to_owned());
        }
        // (Its change is none, so nothing is written after it.)
        eb_bar(&mut panel, &at, ([0.0, 15.0], weight, 0.0, true), Some((compare_weight, 0.0)), &stats.weight_look, (add, sub), (&mut colors, &mut sprites, &mut set), &mut pulse);
        if let Some(value) = at(&panel, "Value") {
            texts.insert(value, std::mem::take(&mut said));
        }
    }
    // The weight's icon, its attribute's (`Weight`).
    if let (Some(icon), Some(picture)) = (panel.find("CharacteristicsExpanded/Weight/Icon"), look.attribute_icons.get("Weight")) {
        sprites.insert(icon, *picture);
    }
    // The rows, in the container's grid (`_charPanelContainer`, a
    // `FlexibleGridLayoutGroup`): each a cell, in the figures' order.
    let Some(holder) = panel.find("CharacteristicsExpanded/CharsPanel") else { return None };
    let mut cells = Vec::new();
    for row in figures.rows.iter() {
        let bar = matches!(row.show, Show::Bar { .. });
        let (template, row_look) = if bar { (&stats.bar, &stats.bar_look) } else { (&stats.compact, &stats.compact_look) };
        let top = graft_row(&mut panel, &mut drawn, template, holder);
        cells.push(top);
        let at = |panel: &trp_data::screen::ScenePanel, end: &str| template.0.find(end).map(|index| top + index).filter(|index| *index < panel.nodes.len());
        if let Some(name) = at(&panel, "Name") {
            let size = size_of(&panel, name, &row.name).unwrap_or_default();
            prefer(&mut panel, name, size);
            texts.insert(name, row.name.clone());
        }
        let mut said = row.value.clone().unwrap_or_default();
        if let Some(base) = row.colored {
            let (good, bad) = (rgba(row_look.increasing), rgba(row_look.decreasing));
            if let Some(value) = at(&panel, "Value") {
                if base < 0.0 {
                    colors.insert(value, if row.less_is_good { good } else { bad });
                    if !said.contains('-') {
                        said.insert(0, '-');
                    }
                } else if base > 0.0 {
                    colors.insert(value, if row.less_is_good { bad } else { good });
                    said.insert(0, '+');
                }
            }
        }
        if let Show::Bar { range, base, delta } = row.show {
            let compared = compare.rows.iter().find(|other| other.id == row.id).and_then(|other| match other.show {
                Show::Bar { base, delta, .. } => Some((base, delta)),
                _ => None,
            });
            if let Some(more) = eb_bar(&mut panel, &at, (range, base, delta, row.less_is_good), compared, row_look, (add, sub), (&mut colors, &mut sprites, &mut set), &mut pulse) {
                said.push_str(&more);
            }
        }
        if let Some(value) = at(&panel, "Value") {
            let drawn_words = rich_spans(&said).map_or_else(|| said.clone(), |spans| spans.into_iter().map(|(words, _)| words).collect());
            let size = size_of(&panel, value, &drawn_words).unwrap_or_default();
            prefer(&mut panel, value, size);
            panel.nodes[value].active = true;
            texts.insert(value, said);
        }
        if let Some(icon) = at(&panel, "Icon") {
            let table = match row.table {
                Table::Attribute => &look.attribute_icons,
                Table::Health => &look.figure_icons[0],
                Table::Damage => &look.figure_icons[1],
                Table::Buff => &look.figure_icons[2],
            };
            match table.get(row.id) {
                Some(picture) => {
                    sprites.insert(icon, *picture);
                }
                None => {
                    set.insert(icon, false);
                }
            }
        }
    }
    // The grid's cells: its columns (its fixed count, one where it has
    // none), each as wide as they share it, filled down each column first
    // where its axis is the vertical one; the grid as tall as its rows.
    if let Some(grid) = panel.grid(holder) {
        let width = panel.rect(holder, fitted_size(&panel))[2].max(grid.cell[0]);
        let columns = grid.columns.unwrap_or(1).max(1) as usize;
        let cell_width = (width - (columns as f32 - 1.0) * grid.spacing[0] - grid.padding[0] - grid.padding[1]) / columns as f32;
        let cell_height = if grid.cell[1] > 0.0 { grid.cell[1] } else { 23.0 };
        let down = cells.len().div_ceil(columns).max(1);
        for (n, &top) in cells.iter().enumerate() {
            let (x, y) = if grid.start_axis == 1 { (n / down, n % down) } else { (n % columns, n / columns) };
            let layout = &mut panel.nodes[top].layout;
            layout.anchor_min = [0.0, 1.0];
            layout.anchor_max = [0.0, 1.0];
            layout.pivot = [0.0, 1.0];
            layout.offset = [grid.padding[0] + (cell_width + grid.spacing[0]) * x as f32, -(grid.padding[2] + (cell_height + grid.spacing[1]) * y as f32)];
            layout.size_delta = [cell_width, cell_height];
        }
        let rows_down = cells.len().div_ceil(columns);
        let height = if rows_down == 0 { 0.0 } else { grid.padding[2] + grid.padding[3] + (cell_height + grid.spacing[1]) * rows_down as f32 - grid.spacing[1] };
        let element = panel.nodes[holder].element.unwrap_or(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[holder].element = Some(trp_data::screen::Element { min: [element.min[0], height], preferred: [element.preferred[0], height], ..element });
    }
    let size = fitted_size(&panel);
    let switches = ["CharacteristicsExpanded/Switcher", "InfoButton"].iter().zip([expanded, !expanded]).filter(|(_, on)| *on).filter_map(|(path, _)| panel.find(path).map(|index| panel.rect(index, size))).collect();
    Some(EbStatsBuilt { panel, drawn, texts, set, colors, sprites, size, switches, pulse })
}

/// The characteristics panel as `eb_stats_built` laid it out, its top left
/// at `origin` (units).
pub(crate) fn draw_eb_stats(layer: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &EbStatsBuilt, origin: [f32; 2], s: f32) {
    let marks = Marks { words: Some(&built.texts), set: Some(&built.set), colors: Some(&built.colors), sprites: Some(&built.sprites), ..Marks::default() };
    draw_nodes(layer, look, &built.panel, &built.drawn, EB_STATS, marks, said, Frame::Is(built.size), origin, s);
}

/// The window's own preferred size as made (its `LayoutElement`'s, 670 by
/// 500), which stretching it starts from.
pub(crate) fn info_preferred(look: &HealthLook) -> Option<[f32; 2]> {
    look.info.as_ref()?.nodes.first()?.element.map(|element| element.preferred).filter(|size| size[0] > 0.0 && size[1] > 0.0)
}

/// The piece the `index`th window that shows a thing draws its loader as,
/// which `info_preview` turns while it shows.
pub(crate) fn info_loader_piece(look: &HealthLook, index: usize) -> Option<usize> {
    look.info.as_ref()?.find(INFO_LOADER).map(|loader| INFO + (index << 12) + loader)
}

// ==== The inspect window drawn, and the menu of a right click ====

/// The window that shows a thing (`InfoWindow`) as `info_built` laid it
/// out, in a frame at its top left corner, `s` pixels to a unit, the
/// `index`th window open. Its model is drawn by the caller into its
/// preview's room (`InfoBuilt::preview`); its description in its box, cut
/// to it (`Mask`; the game's scrolls, not built).
pub fn draw_info_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, words: &InfoWords, built: &InfoBuilt, index: usize, s: f32, (opened, scrolled): (u64, f32)) {
    let InfoBuilt { panel, drawn, texts, set, colors, sprites, groups, icons, size, .. } = built;
    let size = *size;
    // (The description is drawn below, and a bar's handle where it moves.)
    let skip: Vec<usize> = panel.find(INFO_DESCRIPTION).into_iter().chain(built.scroll.as_ref().map(|scroll| scroll.handle)).collect();
    let base = INFO + (index << 12);
    draw_nodes(column, look, panel, drawn, base, Marks { words: Some(texts), skip: &skip, set: Some(set), colors: Some(colors), sprites: Some(sprites), groups: Some(groups), ..Marks::default() }, said, Frame::Is(size), [0.0, 0.0], s);
    let place = |x: f32, y: f32, w: f32, h: f32| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
    // An empty slot's filters' marks (`filter_icon`, white: its prefab's
    // colour not read).
    for ([x, y, w, h], picture) in icons {
        if let Some(sprite) = look.sprite_of(*picture, Color::WHITE) {
            draw_sprite(column, &sprite, [x * s, y * s, w * s, h * s], s);
        }
    }
    // The description's bar's handle, where it is scrolled to
    // (`inventory_view::scroll_info_texts` moves it).
    if let Some(scroll) = &built.scroll {
        let [x, y, w, h] = scroll.handle_at(scrolled);
        column.spawn((place(x, y, w, h), BackgroundColor(scroll.color), InfoScrollHandle(opened, scroll.color)));
    }
    let moved = if built.scroll.is_some() { -scrolled * s } else { 0.0 };
    if let (Some(scroll), Some(description)) = (panel.find(INFO_SCROLL), panel.find(INFO_DESCRIPTION)) {
        let Some(text) = panel.nodes[description].text.clone() else { return };
        let [x, y, w, h] = panel.rect(scroll, size);
        let font = TextFont { font: look.styled_font(&panel, text.font, text.style), font_size: text.size * s, weight: weight_of(text.style), ..default() };
        let effect = look.text_effect(&panel, text.font);
        let width = (w - text.margin[0] - text.margin[2]) * s;
        let tmp = look.tmp_of(&panel, &text);
        // In its area, clipped to it, moved up as far as it is scrolled.
        let moving = (Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(moved), width: Val::Percent(100.0), ..default() }, InfoScrollContent(opened));
        column.spawn(Node { overflow: Overflow::clip(), ..place(x, y, w, h) }).with_children(|frame| {
            frame.spawn(moving).with_children(|clip| {
                let room = Node { margin: UiRect::new(Val::Px(text.margin[0] * s), Val::ZERO, Val::Px(text.margin[1] * s), Val::ZERO), ..default() };
                match tmp {
                    // As the game draws it, its lines broken at the box's width.
                    Some((asset, material)) => {
                        let run = trp_data::tmp_text::TmpRun { text: words.description.clone(), size: text.size * s, color: screen_numbers(rgba(text.color)), bold: text.style & 1 != 0 };
                        let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: Some(width), align: text.horizontal & 7 };
                        tmp_words(clip, room, TmpText { asset, material, runs: vec![run], layout }, &mut |_| {});
                    }
                    None => effect_words(clip, Node { width: Val::Px(width), ..room }, &[(words.description.clone(), font, rgba(text.color))], effect, text.size * s, &mut |_, _, _| {}),
                }
            });
        });
    }
}

/// The windows of containers' grids, each window's past the one before it.
const GRID_WINDOW: usize = 29 << 20;

/// The game's window of a container's grids laid out for one container
/// (`GridWindow`; inventory spec, "A container's window"): its panel with
/// the caption's words asking the caption's row for their width, its size
/// in units, each grid's place in it, the caption's words as they fit,
/// the caption's bar (what moves it) and the cross, and whether it is the
/// priority window.
pub(crate) struct GridWindowBuilt {
    panel: trp_data::screen::ScenePanel,
    pub(crate) size: [f32; 2],
    pub(crate) grids: Vec<[f32; 4]>,
    caption: String,
    pub(crate) bar: [f32; 4],
    pub(crate) close: [f32; 4],
    picked: bool,
}

/// The window of a container's grids for a container of that name, layout
/// and grids (`GridWindow.Show`): the container's own layout where it has
/// one with as many grids (`TemplatedGridsView`), else the default row
/// (`GeneratedGridsView`), at the window's padding (5 left, 26 down); the
/// window as large as its padding and the grids ask (`ContentSizeFitter`,
/// preferred), at least its `LayoutElement`'s least (128 by 64). The
/// caption is the short name as the locale has it, not in capitals (its
/// text has no capitals' style), cut with an ellipsis where its row is
/// narrower (`TextOverflowModes.Ellipsis`). No slots are drawn: neither
/// view the window makes has any in a raid.
pub(crate) fn grid_window_built(look: &HealthLook, name: &str, layout: &str, cells: &[[u32; 2]], picked: bool) -> Option<GridWindowBuilt> {
    let window = look.grid_window.as_ref()?;
    let mut panel = window.panel.clone();
    let rects = container_grids(look, layout, cells);
    let content = match look.layouts.get(layout).filter(|(_, own)| own.len() >= cells.len()) {
        Some((own, _)) => *own,
        None => {
            let row = look.containers.as_ref().and_then(|views| views.grids_row).unwrap_or_default();
            let right = rects.iter().fold(0.0f32, |most, [x, _, w, _]| most.max(x + w));
            let bottom = rects.iter().fold(0.0f32, |most, [_, y, _, h]| most.max(y + h));
            [right + row.padding[1], bottom + row.padding[3]]
        }
    };
    let pad = panel.nodes[0].group.map_or([5.0, 4.0, 26.0, 4.0], |group| group.padding);
    let least = panel.nodes[0].element.map_or([128.0, 64.0], |element| element.min);
    let size = [(pad[0] + content[0] + pad[1]).max(least[0]), (pad[2] + content[1] + pad[3]).max(least[1])];
    let grids = rects.iter().map(|[x, y, w, h]| [pad[0] + x, pad[2] + y, *w, *h]).collect();
    // The caption asks its row for its words' own size; what it gets of the
    // row it shows, cut to fit.
    if let Some(asked) = text_size(look, &panel, window.caption, name) {
        prefer_size(&mut panel, window.caption, asked);
    }
    let room = panel.placed(window.caption, size).rect[2];
    let caption = match panel.nodes[window.caption].text.as_ref().and_then(|text| Some((text, look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?))) {
        Some((text, face)) => fitted_words(&face.font, &trp_data::screen::TextLook { wraps: false, overflow: 1, ..text.clone() }, name, room),
        None => name.to_owned(),
    };
    let bar = panel.placed(window.caption_panel, size).rect;
    let close = panel.placed(window.close, size).rect;
    Some(GridWindowBuilt { panel, size, grids, caption, bar, close, picked })
}

/// The window of a container's grids drawn, the `index`th window, at its
/// top left corner: its frame, shadow and caption's row as the prefab has
/// them, its icon and caption's ground in the colours of a priority window
/// or not (`SetSelectedAsTargetVisual`), and each grid's view; the things
/// in the grids are the caller's.
pub(crate) fn draw_grid_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &GridWindowBuilt, index: usize, s: f32) {
    let Some(window) = look.grid_window.as_ref() else { return };
    let words: HashMap<usize, String> = [(window.caption, built.caption.clone())].into();
    let pick = usize::from(built.picked);
    let colors: HashMap<usize, Color> = [(window.icon, rgba(window.icon_colors[pick])), (window.caption_panel, rgba(window.header_colors[pick]))].into();
    let marks = Marks { words: Some(&words), colors: Some(&colors), ..Marks::default() };
    draw_nodes(column, look, &built.panel, &look.grid_window_drawn, GRID_WINDOW + (index << 12), marks, said, Frame::Is(built.size), [0.0, 0.0], s);
    for [x, y, w, h] in &built.grids {
        draw_grid_view(column, look, said, [*x, *y], [*w, *h], s);
    }
}

impl HealthLook {
    /// The game's simple tooltip's font, size, colour of its words and of
    /// its frame, where it was read; the app's otherwise.
    pub fn tooltip_words(&self) -> (Handle<Font>, f32, Color, Color) {
        let text = self.tooltip.as_ref().and_then(|panel| panel.nodes.iter().find_map(|node| node.text.as_ref()).map(|text| (panel, text)));
        let (font, size, color) = text.map_or((Handle::default(), 14.0, Color::srgb(0.651, 0.690, 0.710)), |(panel, text)| (self.font(panel, text.font), text.size, rgba(text.color)));
        let frame = self.tooltip.as_ref().and_then(|panel| panel.find("Background/Border")).and_then(|index| self.tooltip.as_ref()?.nodes[index].image.map(|image| rgba(image.color))).unwrap_or(Color::srgb(0.322, 0.349, 0.353));
        (font, size, color, frame)
    }
}

/// A line of the menu of a right click, for `menu_layout`: the line's
/// name (its template and icon go by it), its caption, whether it can be
/// done now, whether the mouse is on it, and whether it has lines of its
/// own (an arrow).
pub struct MenuLineLook<'a> {
    pub name: &'static str,
    pub caption: &'a str,
    pub enabled: bool,
    pub hovered: bool,
    pub arrow: bool,
}

/// The menu of a right click laid out as the game's (`TARKOV UI`): the
/// game's menu with a copy of a template for each line put in what holds
/// the lines, laid out by the game's layout; what of it is drawn and how;
/// its size in units, and each line's rectangle in units from its top
/// left.
pub struct MenuBuilt {
    panel: trp_data::screen::ScenePanel,
    drawn: Drawn,
    words: HashMap<usize, String>,
    pictures: HashMap<usize, Picture>,
    groups: HashMap<usize, f32>,
    colors: HashMap<usize, Color>,
    set: HashMap<usize, bool>,
    pub size: [f32; 2],
    pub lines: Vec<[f32; 4]>,
}

/// A line's two colours (`SimpleContextMenuButton._defaultBackgroundColor`,
/// `_selectedBackgroundColor`), from its script's fields: after the four
/// pointers the button and the image take, at 88 and 104.
fn menu_line_colors(fields: &[u8]) -> Option<[Color; 2]> {
    let float = |at: usize| fields.get(at..at + 4).map(|word| f32::from_le_bytes(word.try_into().unwrap_or_default()));
    let color = |at: usize| Some(Color::srgba(float(at)?, float(at + 4)?, float(at + 8)?, float(at + 12)?));
    Some([color(88)?, color(104)?])
}

/// The menu laid out (`InteractionButtonsContainer.CreateContextButton`):
/// each line a copy of its template (Discard's own and the wish list's own,
/// `SimpleSpecifiedContextButtons`), its caption sized by the caption's
/// font, its icon at its own size, its arrow only where it has lines of
/// its own (`_subMenuArrow.enabled`), at 0.3 where it cannot be done
/// (`SetUnlockStatus`), and its ground the selected colour under the mouse.
/// None where the game's menu was not read.
pub fn menu_layout(look: &HealthLook, lines: &[MenuLineLook]) -> Option<MenuBuilt> {
    layout_menu(look, look.menu.as_ref()?, &look.menu_drawn, lines)
}

/// The menu of a line's own lines laid out the same way, from its own
/// copy of the templates (`SubMenuTemplate`).
pub fn submenu_layout(look: &HealthLook, lines: &[MenuLineLook]) -> Option<MenuBuilt> {
    layout_menu(look, look.submenu.as_ref()?, &look.submenu_drawn, lines)
}

fn layout_menu(look: &HealthLook, source: &trp_data::screen::ScenePanel, source_drawn: &Drawn, lines: &[MenuLineLook]) -> Option<MenuBuilt> {
    let holder = source.find(MENU_LINES)?;
    let mut panel = source.clone();
    let mut drawn = Drawn { shown: source_drawn.shown.clone(), pictures: source_drawn.pictures.clone() };
    // The templates themselves are not drawn.
    let under = |index: usize, top: usize| {
        let mut at = Some(index);
        while let Some(i) = at {
            if i == top {
                return true;
            }
            at = source.nodes[i].parent;
        }
        false
    };
    let templates: Vec<usize> = MENU_TEMPLATES.iter().filter_map(|path| source.find(path)).collect();
    for index in 0..source.nodes.len() {
        if templates.iter().any(|&top| under(index, top)) {
            drawn.shown[index] = Shown::Never;
        }
    }
    let (mut words, mut pictures, mut groups, mut colors, mut set) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
    let mut tops = Vec::new();
    for line in lines {
        // (Both menus' `SimpleSpecifiedContextButtons`: the wish list's
        // line and its groups' in the wish list's template, Discard and
        // Remove from Wishlist in Discard's.)
        let path = match line.name {
            "Discard" | "RemoveFromWishlist" => MENU_TEMPLATES[2],
            "AddToWishlist" | "Quests" | "Hideout" | "Trading" | "Equipment" | "Other" => MENU_TEMPLATES[1],
            _ => MENU_TEMPLATES[0],
        };
        let Some(top) = source.find(path) else { continue };
        let copied: Vec<usize> = (0..source.nodes.len()).filter(|&index| under(index, top)).collect();
        let first = panel.nodes.len();
        let new_of = |old: usize| copied.iter().position(|&i| i == old).map(|at| first + at);
        for &old in &copied {
            let mut node = source.nodes[old].clone();
            node.parent = if old == top { Some(holder) } else { node.parent.and_then(new_of) };
            node.active |= old == top;
            panel.nodes.push(node);
            drawn.shown.push(source_drawn.shown.get(old).copied().unwrap_or(Shown::Never));
            drawn.pictures.push(source_drawn.pictures.get(old).copied().flatten());
        }
        let named = |panel: &trp_data::screen::ScenePanel, name: &str| (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == name);
        let copy_top = first;
        if let Some(text_at) = named(&panel, "Text") {
            let caption = line.caption.to_uppercase();
            if let (Some(text), Some(font)) = (panel.nodes[text_at].text.clone(), look.menu_font.as_ref()) {
                let width = font.width(&caption, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
                let flexible = panel.nodes[text_at].element.map_or([1.0, -1.0], |element| element.flexible);
                panel.nodes[text_at].element = Some(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [width, -1.0], flexible });
            }
            words.insert(text_at, caption);
        }
        if let (Some(icon), Some(picture)) = (named(&panel, "Icon"), look.menu_icons.get(line.name)) {
            pictures.insert(icon, *picture);
        }
        if let Some(arrow) = named(&panel, "SubmenuArrow") {
            if line.arrow {
                let size = drawn.pictures.get(arrow).copied().flatten().map_or([9.0, 11.0], |picture| picture.size);
                panel.nodes[arrow].element = Some(trp_data::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: size, flexible: [-1.0, -1.0] });
            } else {
                set.insert(arrow, false);
            }
        }
        if !line.enabled {
            groups.insert(copy_top, 0.3);
        }
        // The ground the line's colours go on: the one beside its icon.
        let icon_holder = named(&panel, "IconPlaceholder").and_then(|index| panel.nodes[index].parent);
        let ground = (first..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Background" && panel.nodes[index].parent == icon_holder);
        let line_colors = panel.script(copy_top, "ContextMenuButton").and_then(menu_line_colors);
        if let (Some(ground), Some([normal, selected])) = (ground, line_colors) {
            colors.insert(ground, if line.hovered { selected } else { normal });
        }
        tops.push(copy_top);
    }
    let size = [panel.asks(0, 0).1, panel.asks(0, 1).1];
    let rects = tops.iter().map(|&top| panel.rect(top, size)).collect();
    Some(MenuBuilt { panel, drawn, words, pictures, groups, colors, set, size, lines: rects })
}

/// Draws the menu as `menu_layout` laid it out, its top left at `origin`
/// in units, `s` pixels to a unit.
/// (`sub`: the menu of a line's own lines, numbered apart from the first.)
pub fn draw_menu(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &MenuBuilt, origin: [f32; 2], s: f32, sub: bool) {
    // A line's icon is only its sprite swapped (`ContextMenuButton.Show`):
    // drawn in the icon's own 16 units square, kept to its shape.
    let marks = Marks { words: Some(&built.words), sprites: Some(&built.pictures), groups: Some(&built.groups), colors: Some(&built.colors), set: Some(&built.set), ..Marks::default() };
    let base = if sub { MENU + (1 << 19) } else { MENU };
    draw_nodes(column, look, &built.panel, &built.drawn, base, marks, said, Frame::Is(built.size), origin, s);
}

// ==== Binds, item grounds and looks, the pictures, the version line ====

/// The label of a thing bound to a key: its picture (the cut of its
/// texture) at its sprite's own size, in units, and its key's words.
pub struct BindLook {
    pub image: Handle<Image>,
    pub cut: Rect,
    pub size: [f32; 2],
    pub text_size: f32,
    pub color: Color,
    pub font: Handle<Font>,
}

/// The ground of a thing's view as the game colours it
/// (`ItemView.UpdateStaticInfo`): the database's colour of its kind
/// (`TaxonomyColorExtension.ColorMapping`) at 0.302.
pub fn item_ground(color: &str) -> Color {
    let [r, g, b] = match color {
        "blue" => [28, 65, 86],
        "yellow" => [104, 102, 40],
        "green" => [21, 45, 0],
        "red" => [109, 36, 24],
        "black" => [0, 0, 0],
        "grey" => [29, 29, 29],
        "violet" => [76, 42, 85],
        "orange" => [60, 25, 0],
        _ => [127, 127, 127],
    };
    Color::srgba_u8(r, g, b, 77)
}

/// A view's ground under the mouse (`ItemView.DefaultSelectedColor`).
pub const ITEM_SELECTED: Color = Color::srgba(1.0, 1.0, 1.0, 77.0 / 255.0);

/// The words a stack's count is in (`GridItemView.GetStackColor`):
/// rounds', money's and the rest's, by the database's names of the kind's
/// classes.
pub fn stack_color(class_names: &[String]) -> Color {
    let has = |name: &str| class_names.iter().any(|class| class == name);
    if has("Ammo") {
        Color::srgb_u8(0xda, 0xda, 0xbc)
    } else if has("Money") {
        Color::srgb_u8(0x8d, 0xb5, 0x00)
    } else {
        Color::srgb_u8(0xb6, 0xc1, 0xc7)
    }
}

/// Each item view's look made from its prefab, with its pictures and fonts.
fn item_looks(look: &HealthLook) -> [Option<std::sync::Arc<ItemViewLook>>; 2] {
    let Some(items) = &look.items else { return [None, None] };
    [
        items.slot.as_ref().map(|panel| std::sync::Arc::new(item_look(look, panel, &look.slot_item))),
        items.grid.as_ref().map(|panel| std::sync::Arc::new(item_look(look, panel, &look.grid_item))),
    ]
}

/// A thing's view as the inventory draws it, from its prefab as a panel
/// and its pictures (`slot_layout`, `grid_layout`, the edit build screen's
/// `modding_selectable_layout`).
fn item_look(look: &HealthLook, panel: &trp_data::screen::ScenePanel, drawn: &Drawn) -> ItemViewLook {
    {
        let caption = panel.find("Info Panel/Name").or_else(|| panel.find("Info Panel/Caption"));
        let value = (0..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Value" && panel.nodes[index].text.is_some());
        let font = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).map(|text| look.styled_font(panel, text.font, text.style)).unwrap_or_default();
        let border = panel.find("Border").filter(|&index| panel.nodes[index].active).and_then(|index| panel.nodes[index].image.map(|image| rgba(image.color)));
        let bind = (0..panel.nodes.len()).find(|&index| panel.nodes[index].scripts.iter().any(|(class, _)| class == "BindPanel")).and_then(|index| {
            let picture = drawn.pictures.get(index).copied().flatten()?;
            let image = look.images.get(picture.texture)?.clone()?;
            let text = panel.nodes.iter().skip(index + 1).find(|node| node.parent == Some(index) && node.text.is_some())?.text.clone()?;
            let size = [picture.cut.width(), picture.cut.height()];
            Some(BindLook { image, cut: picture.cut, size, text_size: text.size, color: rgba(text.color), font: look.font(panel, text.font) })
        });
        let sprite = |picture: &Picture, color: Color| -> Option<Sprite> {
            let image = look.images.get(picture.texture)?.clone()?;
            Some(Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color })
        };
        let drawn_sprite = |index: usize| -> Option<Sprite> {
            let picture = drawn.pictures.get(index).copied().flatten()?;
            sprite(&picture, panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color)))
        };
        let ends = |end: &str| (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end));
        let inscription = panel.find("Info Panel/Info Label").or_else(|| (0..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Inscription" && panel.nodes[index].text.is_some()));
        let mastering = ends("MasteringIcon/Image").zip(ends("MasteringIcon/Level")).and_then(|(icon, level)| {
            let text = panel.nodes[level].text.as_ref()?;
            Some((drawn_sprite(icon)?, text.size, rgba(text.color), look.font(panel, text.font)))
        });
        let durability = ends("DurabilityContaimer/Background").zip(ends("DurabilityContaimer/Fill Area/Fill")).and_then(|(track, fill)| Some((drawn_sprite(track)?, drawn_sprite(fill)?)));
        let mods = look.mod_pictures.iter().filter_map(|(key, picture)| Some((key.clone(), sprite(picture, Color::WHITE)?))).collect();
        let effect = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.text_effect(panel, text.font));
        let tmp = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.tmp_of(panel, text));
        let face = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1)).cloned());
        let tag = panel.find("TagPanel").zip(panel.find("TagPanel/TagName")).map(|(plate, name)| TagPlateLook {
            plate,
            name,
            name_tmp: tmp(Some(name)),
            name_font: font(Some(name)),
            name_face: face(Some(name)),
            caption_face: face(caption),
        });
        // The wish list's marks, and its column's other members' heights.
        let wish = ends("WishlistIcon").map(|icon| {
            let column = panel.nodes[icon].parent.unwrap_or(icon);
            let edges = (0..panel.nodes.len())
                .filter(|&index| panel.nodes[index].parent.is_some_and(|parent| panel.nodes[parent].name.starts_with("WishlistBorder")))
                .filter_map(|index| Some((index, drawn_sprite(index)?)))
                .collect();
            let mod_height = ends("ModIconContainer").map_or(0.0, |holder| panel.asks(holder, 1).1);
            let inscription_height = inscription
                .filter(|&index| panel.nodes[index].parent == Some(column))
                .and_then(|index| Some((panel.nodes[index].text.as_ref()?.size, face(Some(index))?)))
                .map(|(size, face)| face.font.height(size));
            let icons = look.wish_outlines.iter().filter_map(|(name, picture)| Some((*name, sprite(picture, Color::WHITE)?))).collect();
            WishLook { glow: ends("WishlistGlow").and_then(|glow| Some((glow, drawn_sprite(glow)?))), edges, icon, column, mod_height, inscription_height, icons }
        });
        let spin = look.spin.as_ref().map(|panel| {
            let piece = |end: &str| -> Option<(usize, SpinPicture)> {
                let index = (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end))?;
                let picture = look.spin_drawn.pictures.get(index).copied().flatten()?;
                let image = look.images.get(picture.texture)?.clone()?;
                Some((index, SpinPicture { image, cut: picture.cut, covers: picture.covers }))
            };
            SpinLook { panel: panel.clone(), back: piece("Spinner Background"), fill: piece("Spinner Fill"), loader: piece("LoadMagazineLoader"), stop: piece("CloseImage") }
        });
        ItemViewLook {
            spin,
            wish,
            tag,
            caption_tmp: tmp(caption),
            caption_face: face(caption),
            inscription_tmp: tmp(inscription),
            value_tmp: tmp(value),
            panel: panel.clone(),
            caption,
            value,
            caption_font: font(caption),
            value_font: font(value),
            border,
            bind,
            inscription,
            inscription_font: font(inscription),
            caption_effect: effect(caption),
            value_effect: effect(value),
            inscription_effect: effect(inscription),
            mastering,
            durability,
            mods,
            loader: panel.find("Loader").and_then(|index| Some((drawn_sprite(index)?, panel.nodes[index].layout.size_delta))),
        }
    }
}

/// The panels' textures made images, and their fonts fonts, once, at the
/// start.
fn make_pictures(mut look: ResMut<HealthLook>, mut images: ResMut<Assets<Image>>, mut fonts: ResMut<Assets<Font>>) {
    for (asset, data) in std::mem::take(&mut look.font_data) {
        match Font::try_from_bytes(data) {
            Ok(font) => {
                let handle = fonts.add(font);
                look.fonts.insert(asset, handle);
            }
            Err(e) => eprintln!("a font of the game's could not be made; the app's is used: {e}"),
        }
    }
    let textures = std::mem::take(&mut look.textures);
    look.images = textures
        .into_iter()
        .map(|texture| {
            let texture = texture?;
            match crate::render_data::build_image(&texture, crate::render_data::Role::Color) {
                // Sampled as the engine samples it: a texture set to point
                // filtering at its nearest pixel (the frames of a pixel or
                // two, whose middles are clear); and clamped at its edges
                // where it is set so (`TextureWrapMode.Clamp`), as the
                // interface's mostly are: repeated, a picture's edge took
                // in its far side's (a faint line along the top of the left
                // panel's foot, its bottom row's light).
                Ok(mut image) => {
                    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
                    let wrap = |clamp: bool| if clamp { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
                    let filter = if texture.point { ImageFilterMode::Nearest } else { ImageFilterMode::Linear };
                    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: wrap(texture.clamp[0]),
                        address_mode_v: wrap(texture.clamp[1]),
                        mag_filter: filter,
                        min_filter: filter,
                        mipmap_filter: ImageFilterMode::Linear,
                        ..default()
                    });
                    Some(images.add(image))
                }
                Err(e) => {
                    eprintln!("the health panel's picture {} could not be made: {e:#}", texture.name);
                    None
                }
            }
        })
        .collect();
    let looks = item_looks(&look);
    look.item_looks = looks;
    // The edit build screen's view of a part.
    let part_look = look.edit_build.as_ref().and_then(|eb| eb.part.as_ref()).map(|(panel, drawn)| std::sync::Arc::new(item_look(&look, panel, drawn)));
    if let Some(eb) = look.edit_build.as_mut() {
        eb.part_look = part_look;
    }
}

/// The game's line of its version at the screen's foot.
#[derive(Component)]
struct VersionLine(u32);

/// Draws the game's line of its version (`TARKOV UI`) along the screen's
/// foot, over every screen as the game's preloader draws it: its text's
/// look and place as the scene has them (Bender at 16, its colour at a
/// third, 12 in from the left, in a band 31.5 high just over the foot), the
/// words the app's own (`TRP` and its version, where the game's say its
/// build and SPT's its own: the app's choice). Made afresh
/// as the window's height changes.
fn show_version(mut commands: Commands, look: Res<HealthLook>, window: Query<&Window, With<bevy::window::PrimaryWindow>>, shown: Query<(Entity, &VersionLine)>) {
    let Some((label, below)) = look.version.as_ref().filter(|_| look.tarkov_ui) else { return };
    let Ok(window) = window.single() else { return };
    let height = window.height().round() as u32;
    if height == 0 || shown.iter().any(|(_, line)| line.0 == height) || look.fonts.is_empty() {
        return;
    }
    for (entity, _) in &shown {
        commands.entity(entity).despawn();
    }
    let Some(text) = label.nodes.first().and_then(|node| node.text.as_ref()) else { return };
    let s = height as f32 / 1080.0;
    let band = label.nodes[0].layout.size_delta[1];
    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        bottom: Val::Px(below * s),
        width: Val::Percent(100.0),
        height: Val::Px(band * s),
        padding: UiRect { left: Val::Px(text.margin[0] * s), ..default() },
        align_items: AlignItems::Center,
        ..default()
    };
    let font = TextFont { font: look.styled_font(label, text.font, text.style), font_size: text.size * s, weight: weight_of(text.style), ..default() };
    let words = format!("TRP {}", env!("TRP_VERSION"));
    commands.spawn((VersionLine(height), node, GlobalZIndex(30))).with_children(|line| {
        line.spawn((Text::new(words), font, TextColor(rgba(text.color)), TextLayout::new_with_no_wrap()));
    });
}

/// The game's views of a thing handed to the inventory, which draws every
/// thing by them with `TARKOV UI` on.
fn share_item_looks(look: Res<HealthLook>, mut carried: ResMut<crate::inventory_view::Carried>) {
    carried.item_looks = look.item_looks.clone();
    carried.info_size = info_places(&look).map(|places| Vec2::from(places.size));
    carried.game_grid_window = look.grid_window.is_some();
    carried.tag_size = tag_places(&look).map(|places| Vec2::from(places.size));
    carried.tag_limit = if look.tag_window.is_some() { look.tag_limit } else { TAG_LIMIT };
}

/// The colours a tag may be (`EditTagWindow.GetColor`: the game's own
/// seven, a number past them the first).
pub const TAG_COLOURS: [[f32; 3]; 7] = [[0.4, 0.0, 1.0 / 51.0], [0.4, 0.227_450_98, 3.0 / 85.0], [24.0 / 85.0, 0.4, 1.0 / 17.0], [1.0 / 85.0, 0.4, 28.0 / 85.0], [0.168_627_46, 0.0, 20.0 / 51.0], [0.4, 0.0, 0.227_450_98], [0.4, 0.4, 0.4]];
const TAG_COLORS: usize = TAG_COLOURS.len();

// ==== Tags, the panel's pieces, the drop's glow, the effects' icons and tooltips ====

/// A tag's colour by its number (`GetColor`).
pub fn tag_colour(color: u32) -> Color {
    let [r, g, b] = TAG_COLOURS[if (color as usize) < TAG_COLORS { color as usize } else { 0 }];
    Color::srgb(r, g, b)
}

/// The most letters a tag has where the window's field could not be read
/// (as the install's had it on 2026-10-17).
pub const TAG_LIMIT: u32 = 16;
/// A colour cell's side in the grid (`GridLayoutGroup.m_CellSize`, read
/// 2026-10-17).
const TAG_CELL: f32 = 25.0;
const TAG_CAPTION: &str = "Inner/Caption Panel/Caption";
const TAG_CAPTION_PANEL: &str = "Inner/Caption Panel";
const TAG_CLOSE: &str = "Inner/Caption Panel/Close Button";
const TAG_COLORS_PANEL: &str = "Inner/Contents/ColorsAndCount/Colors";
const TAG_COUNT: &str = "Inner/Contents/ColorsAndCount/Text";
const TAG_INPUT: &str = "Inner/Contents/TagInput";
const TAG_TEXT: &str = "Inner/Contents/TagInput/Text Area/Text";
const TAG_PLACEHOLDER: &str = "Inner/Contents/TagInput/Text Area/Placeholder";
const TAG_SAVE: &str = "Inner/Contents/SaveButton";
const TAG_SAVE_SIZE: &str = "Inner/Contents/SaveButton/SizeLabel";
const TAG_SAVE_LABEL: &str = "Inner/Contents/SaveButton/SizeLabel/Label";

/// The prefab's `n`th colour cell.
fn tag_cell(n: usize) -> String {
    if n == 0 { format!("{TAG_COLORS_PANEL}/ColorCell") } else { format!("{TAG_COLORS_PANEL}/ColorCell ({n})") }
}

/// A line's height as TextMeshPro asks it of a layout: the face's ascent
/// to its descent at that size.
fn tmp_line(font: &trp_data::screen::TmpFont, size: f32) -> f32 {
    (font.ascent - font.descent) * size / font.point_size.max(1.0) * font.scale
}

/// The window that edits a tag as the game lays it out (`TARKOV UI`): its
/// size, and where in it its caption bar and cross, its field, its colour
/// cells and its button lie, in units from its top left corner.
pub struct TagPlaces {
    pub size: [f32; 2],
    pub caption: [f32; 4],
    pub close: [f32; 4],
    pub input: [f32; 4],
    pub cells: Vec<[f32; 4]>,
    pub save: [f32; 4],
}

pub fn tag_places(look: &HealthLook) -> Option<TagPlaces> {
    let panel = look.tag_window.as_ref()?;
    let size = info_size(panel);
    let rect = |path: &str| panel.find(path).map(|index| panel.rect(index, size));
    let cells = (0..TAG_COLORS).filter_map(|n| rect(&tag_cell(n))).collect();
    Some(TagPlaces { size, caption: rect(TAG_CAPTION_PANEL)?, close: rect(TAG_CLOSE)?, input: rect(TAG_INPUT)?, cells, save: rect(TAG_SAVE)? })
}

/// What the window that edits a tag shows: the name typed so far, the
/// colour chosen, and whether its field has the keys (a caret after the
/// name).
pub struct TagWords<'a> {
    pub name: &'a str,
    pub color: u32,
    pub focused: bool,
}

/// The window that edits a tag (`EditTagWindow`), as the game draws it with
/// what its script writes: its caption in the install's words, the colours
/// with the chosen one's border, the count of letters against the most
/// (`TagTextChange`), the name or, with none, the placeholder (`container
/// tag`), and its button's `SAVE`; in a frame at its top left corner, `s`
/// pixels to a unit, the `index`th window open. A caret after the name
/// while the field has the keys (`PROVISIONAL`: TextMeshPro's caret is not
/// read: a line of the text's colour, a unit wide, not blinking).
pub fn draw_tag_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, words: &TagWords, index: usize, s: f32) {
    let Some(panel) = &look.tag_window else { return };
    let size = info_size(panel);
    let mut texts = HashMap::new();
    let mut set = HashMap::new();
    let say = |key: &str| said.word(key, key);
    for (path, now) in [
        (TAG_CAPTION, say("ADD/EDIT CONTAINER TAG")),
        (TAG_COUNT, format!("{}/{}", words.name.chars().count(), look.tag_limit)),
        (TAG_TEXT, words.name.to_owned()),
        (TAG_PLACEHOLDER, say("container tag")),
        (TAG_SAVE_LABEL, say("Save").to_uppercase()),
        (TAG_SAVE_SIZE, say("Save").to_uppercase()),
    ] {
        if let Some(index) = panel.find(path) {
            texts.insert(index, now);
        }
    }
    for n in 0..TAG_COLORS {
        if let Some(border) = panel.find(&format!("{}/Border", tag_cell(n))) {
            set.insert(border, n as u32 == words.color);
        }
    }
    let named = !words.name.is_empty();
    for (path, on) in [(TAG_TEXT, named), (TAG_PLACEHOLDER, !named)] {
        if let Some(index) = panel.find(path) {
            set.insert(index, on);
        }
    }
    let base = TAG_WINDOW + (index << 12);
    draw_nodes(column, look, panel, &look.tag_window_drawn, base, Marks { words: Some(&texts), set: Some(&set), ..Marks::default() }, said, Frame::Is(size), [0.0, 0.0], s);
    // The caret, after the name, the text's height.
    if let (true, Some(text_index)) = (words.focused, panel.find(TAG_TEXT)) {
        let Some(text) = panel.nodes[text_index].text.as_ref() else { return };
        let [x, y, _, h] = panel.rect(text_index, size);
        let width = panel.file_of(text.font.0).and_then(|file| look.tmp_faces.get(&(file, text.font.1))).map_or(0.0, |face| face.font.width(words.name, text.size, text.spacing, text.style & 1 != 0));
        let tall = panel.file_of(text.font.0).and_then(|file| look.tmp_faces.get(&(file, text.font.1))).map_or(text.size, |face| tmp_line(&face.font, text.size));
        let top = match text.vertical {
            256 => y,
            1024 => y + h - tall,
            _ => y + (h - tall).max(0.0) / 2.0,
        };
        column.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px((x + width + 1.0) * s), top: Val::Px(top * s), width: Val::Px(s.max(1.0)), height: Val::Px(tall * s), ..default() },
            BackgroundColor(rgba(text.color)),
        ));
    }
}

/// One object of one of the game's panels as drawn, by its number among
/// them (`STRIP`, `FOOT`); a filled image's picture inside its clip, and
/// how wide the clip is when full; a text's words, its own and those put
/// before them.
#[derive(Component, Clone, Copy)]
pub struct Piece(pub usize);
#[derive(Component)]
pub(crate) struct PieceFill(pub(crate) usize);
#[derive(Component)]
pub(crate) struct FullWidth(pub(crate) f32);
#[derive(Component)]
struct PieceText(usize);
#[derive(Component)]
struct PieceSpan(usize);
/// A rate's arrow, which bobs while its figure moves: its top as laid out,
/// in pixels, and the pixels to a unit.
#[derive(Component)]
struct Bob(f32, f32);
/// The effects on a part, by the part's place in `Part::ALL`, in words
/// where the game has its icons (`EffectsPanel`, whose icons are the hard
/// settings' table, not read: `docs/spec/health.md`, "The HEALTH tab").
#[derive(Component)]
struct EffectWords(usize);

pub(crate) fn rgba(color: [f32; 4]) -> Color {
    Color::srgba(color[0], color[1], color[2], color[3])
}

/// Draws the game's health panel in the inventory's left panel: each
/// object where the game lays it out in a left panel of that size (the
/// screen's units), the left panel's top left corner at `origin` in what
/// it is drawn into, `s` pixels to a unit.
pub fn draw_panel(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, health: Option<&Health>, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(game) = &look.panel else {
        column.spawn(words("The game's health panel could not be read from the install.", 12.0, FAINT));
        return;
    };
    if health.is_none() {
        column.spawn(words("The options' HEALTH SYSTEM is off: the character cannot be hurt, and is taken as fed and watered.", 13.0, WORDS));
        return;
    }
    let panel = &game.panel;
    let arrows: Vec<usize> = game.views.parameters.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.up, f.down]).flatten().collect();
    let glows: Vec<usize> = game.views.parts.iter().filter_map(|view| panel.find(&format!("{}/BackGlowImage", panel.nodes[view.view].path))).collect();
    let headers: Vec<usize> = game.views.parts.iter().filter_map(|view| panel.find(&format!("{}/Header/Text", panel.nodes[view.view].path))).collect();
    let marks = Marks { arrows: &arrows, glows: &glows, headers: &headers, ..Marks::default() };
    draw_nodes(column, look, panel, &look.health, 0, marks, said, Frame::In(left), origin, s);
    // The effects on each part in its view's effects panel: as the game's
    // icons where they were read (`EffectsPanel`: a grid of 24 by 22 from
    // its top left, left to right), else in words.
    for (n, view) in game.views.parts.iter().enumerate() {
        let Some(effects) = view.effects else { continue };
        let [x, y, w, h] = panel.placed_in(effects, left).rect;
        let at = Node { position_type: PositionType::Absolute, left: Val::Px((origin[0] + x) * s), top: Val::Px((origin[1] + y) * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        if look.effect_pictures.is_empty() {
            column.spawn((at, Visibility::Inherited)).with_children(|cell| {
                cell.spawn((EffectWords(n), words("", 10.0 * s, FAINT)));
            });
        } else {
            let grid = Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_content: AlignContent::FlexStart, ..at };
            column.spawn((EffectIcons(n, s, Vec::new()), grid, Visibility::Inherited));
        }
    }
    // Where a thing let go goes to a part (`HealthBarButton`, on the part's
    // view: what of the view is drawn takes the mouse, the engine's
    // pointer events going up to the button), over all of it.
    for (n, view) in game.views.parts.iter().enumerate() {
        let path = &panel.nodes[view.view].path;
        let mut area: Option<[f32; 4]> = None;
        for (index, node) in panel.nodes.iter().enumerate() {
            let below = index == view.view || node.path.starts_with(&format!("{path}/"));
            let drawn = look.health.shown.get(index).is_some_and(|shown| *shown != Shown::Never) && (node.image.is_some() || node.text.is_some());
            if !below || !drawn {
                continue;
            }
            let [x, y, w, h] = panel.placed_in(index, left).rect;
            area = Some(match area {
                None => [x, y, x + w, y + h],
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h)],
            });
        }
        let Some([x0, y0, x1, y1]) = area else { continue };
        let at = Node { position_type: PositionType::Absolute, left: Val::Px((origin[0] + x0) * s), top: Val::Px((origin[1] + y0) * s), width: Val::Px((x1 - x0) * s), height: Val::Px((y1 - y0) * s), ..default() };
        column.spawn((HealthDrop(n), RelativeCursorPosition::default(), at));
    }
}

/// Where a thing carried and let go is used on a part, by the part's
/// place in `Part::ALL`: over the part's view on the HEALTH tab, the
/// game's or the app's.
#[derive(Component)]
pub struct HealthDrop(pub usize);

/// Lights a part's glow while a thing that can be used on it is carried
/// over its view (`HealthBarButton.OnPointerEnter`: `Highlighted`, the
/// glow's alpha by its clip, from 0 to 1 in a third of a second; a thing
/// that can do nothing to it lights nothing), and puts it out as the
/// thing leaves or is let go (`Normal`). The game's look (`TARKOV UI`)
/// alone: the app's has no glow.
#[allow(clippy::too_many_arguments)]
/// The piece of the window that shows a thing, number `index` among the
/// windows open, drawn for its panel's node `node`.
pub(crate) fn info_piece(index: usize, node: usize) -> usize {
    INFO + (index << 12) + node
}

/// A button of a window that shows a thing, to be lit under the mouse: its
/// ground's piece, its colours at rest and lit, its group's alpha, and the
/// window's layer among the covers.
#[derive(Component)]
pub(crate) struct InfoButtonLight {
    pub(crate) piece: usize,
    pub(crate) colors: [Color; 2],
    pub(crate) alpha: f32,
    pub(crate) layer: u8,
}

/// A window's buttons under the mouse (`SimpleContextMenuButton.
/// OnPointerEnter`, `OnPointerExit`): the ground of the one under it, on
/// the window on top there, in its lit colour at once, the rest in their
/// own; one that cannot be done lights too, at its group's 0.3 (it still
/// takes the mouse: `SetUnlockStatus`, no raycast change). Recoloured in
/// place: a rebuild of the screen would lose the mouse for a frame.
fn light_info_buttons(
    controller: Res<Controller>,
    lights: Query<(&InfoButtonLight, &RelativeCursorPosition)>,
    covers: Query<(&crate::inventory_view::Cover, &RelativeCursorPosition)>,
    mut pieces: Query<(Option<&Piece>, Option<&SliceOf>, &mut ImageNode)>,
) {
    if lights.is_empty() {
        return;
    }
    let top = crate::inventory_view::cover_under(covers.iter());
    let wanted: HashMap<usize, Color> = lights
        .iter()
        .map(|(light, at)| {
            let lit = controller.inventory_open && at.cursor_over() && light.layer == top;
            let color = light.colors[lit as usize];
            (light.piece, color.with_alpha(color.alpha() * light.alpha))
        })
        .collect();
    for (piece, slice, mut image) in &mut pieces {
        let Some(index) = piece.map(|piece| piece.0).or(slice.map(|slice| slice.0)) else { continue };
        let Some(want) = wanted.get(&index) else { continue };
        if image.color != *want {
            image.color = *want;
        }
    }
}

fn light_drop_glow(
    look: Res<HealthLook>,
    controller: Res<Controller>,
    carried: Res<crate::inventory_view::Carried>,
    game: Res<crate::GameData>,
    dragging: Res<crate::inventory_view::Dragging>,
    time: Res<Time<Real>>,
    drops: Query<(&HealthDrop, &RelativeCursorPosition, &InheritedVisibility)>,
    mut glows: Query<(Option<&Piece>, Option<&SliceOf>, &mut ImageNode)>,
    mut lit: Local<[Option<f32>; 7]>,
) {
    let Some(panel) = look.panel.as_ref().filter(|_| look.tarkov_ui) else { return };
    let over = drops.iter().find(|(_, at, shown)| shown.get() && at.cursor_over()).map(|(drop, ..)| drop.0);
    let usable = |n: usize| -> bool {
        let (Some(id), Some(health)) = (dragging.0, controller.health.as_ref()) else { return false };
        let Some(item) = carried.equipment.item.find(id) else { return false };
        let Some(medicine) = trp_sim::health::Medicine::of(&game.db, &item.kind.template) else { return false };
        let resource = item.resource.unwrap_or_else(|| medicine.full());
        health.part_to_apply(&medicine, resource, Some(Part::ALL[n])).is_some()
    };
    let dt = time.delta_secs();
    for n in 0..lit.len() {
        lit[n] = if over == Some(n) && usable(n) { Some(lit[n].map_or(0.0, |t| t + dt)) } else { None };
    }
    let mut values = Vec::new();
    let alpha = |n: usize, values: &mut Vec<f32>| match (lit[n], &look.glow_lit) {
        (Some(t), Some((clip, curve))) => {
            clip.sample(t, values);
            values.get(*curve).copied().unwrap_or(1.0)
        }
        (Some(_), None) => 1.0,
        (None, _) => look.glow_alpha.unwrap_or(0.0),
    };
    let glow_of: Vec<Option<usize>> = panel.views.parts.iter().map(|view| panel.panel.find(&format!("{}/BackGlowImage", panel.panel.nodes[view.view].path))).collect();
    for (piece, slice, mut image) in &mut glows {
        let Some(index) = piece.map(|piece| piece.0).or(slice.map(|slice| slice.0)) else { continue };
        let Some(n) = glow_of.iter().position(|glow| *glow == Some(index)) else { continue };
        let want = alpha(n, &mut values);
        if (image.color.alpha() - want).abs() > 1e-4 {
            image.color = image.color.with_alpha(want);
        }
    }
}

/// A part's effects panel drawn with the game's icons: the part's place in
/// `Part::ALL`, the pixels to a unit, and what it shows (each icon's
/// effect, by the game's name, and whether it is left over, blinking).
#[derive(Component)]
struct EffectIcons(usize, f32, Vec<(&'static str, bool)>);

/// One effect's icon (`EffectIcon`): what its tooltip says (the effect's
/// name in the install's words, and while it has under two hours left, its
/// seconds left: `BuffDescription.TimeLeftDisplay`; a left-over one's name
/// alone), and whether it blinks.
#[derive(Component)]
struct EffectIcon {
    part: usize,
    kind: Kind,
    residual: bool,
}

/// The game's icon of an effect of the app's health, by the game's name
/// (`StaticIcons.EffectSprites`); none for what the game shows no icon of.
pub(crate) fn effect_icon(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::LightBleeding => "LightBleeding",
        Kind::HeavyBleeding => "HeavyBleeding",
        Kind::Wound => "Wound",
        Kind::Fracture => "Fracture",
        Kind::Pain => "Pain",
        Kind::Tremor => "Tremor",
        Kind::PainKiller => "Painkiller",
        Kind::Exhaustion => "Exhaustion",
        Kind::Dehydration => "Dehydration",
        Kind::ChronicFatigue => "ChronicStaminaFatigue",
        Kind::Encumbered => "Encumbered",
        Kind::OverEncumbered => "OverEncumbered",
        Kind::Contusion => "Contusion",
        Kind::Stun => "Stun",
        Kind::TunnelVision => "TunnelVision",
        // The game shows a stimulator's buffs and its side effects by two
        // icons (`IStimulatorBuff`, `IStimulatorDebuff`); here the buffs'
        // alone. PROVISIONAL.
        Kind::Stimulator => "StimulatorBuff",
        Kind::LowHealth | Kind::Existence | Kind::MedEffect | Kind::Flash | Kind::Endurance | Kind::DamageModifier => return None,
    })
}

/// The cell an effect's icon takes in its panel's grid (`GridLayoutGroup`:
/// `m_CellSize`), in units.
const EFFECT_CELL: [f32; 2] = [24.0, 22.0];

/// Keeps each part's icons as its effects are (`EffectsPanel.UpdateEffects`:
/// the part's effects at work, then those left over, one icon of a kind),
/// and blinks those left over (`EffectIcon.Co_Blinking`: the alpha from 0.2
/// to 1 and back at one a second).
fn fill_effect_icons(mut commands: Commands, controller: Res<Controller>, look: Res<HealthLook>, time: Res<Time<Real>>, mut panels: Query<(Entity, &mut EffectIcons)>, mut icons: Query<(&EffectIcon, &mut ImageNode)>) {
    let Some(health) = controller.health.as_ref() else { return };
    for (entity, mut panel) in &mut panels {
        let part = Part::ALL[panel.0];
        let mut wanted: Vec<(&'static str, bool, Kind)> = Vec::new();
        for residual in [false, true] {
            for effect in health.effects.iter().filter(|e| e.part == part && if residual { e.stage == trp_sim::health::Stage::Residual } else { e.existing() }) {
                let Some(name) = effect_icon(effect.kind) else { continue };
                if !wanted.iter().any(|(had, ..)| *had == name) {
                    wanted.push((name, residual, effect.kind));
                }
            }
        }
        let shown: Vec<(&'static str, bool)> = wanted.iter().map(|(name, residual, _)| (*name, *residual)).collect();
        if panel.2 == shown {
            continue;
        }
        panel.2 = shown;
        let s = panel.1;
        let images = &look.images;
        commands.entity(entity).despawn_related::<Children>().with_children(|grid| {
            for (name, residual, kind) in wanted {
                let Some(picture) = look.effect_pictures.get(name) else { continue };
                let Some(image) = images.get(picture.texture).cloned().flatten() else { continue };
                let node = Node { width: Val::Px(EFFECT_CELL[0] * s), height: Val::Px(EFFECT_CELL[1] * s), flex_shrink: 0.0, ..default() };
                let drawn = ImageNode { image, rect: Some(picture.cut), flip_y: true, ..default() };
                grid.spawn((EffectIcon { part: panel.0, kind, residual }, node, drawn, RelativeCursorPosition::default()));
            }
        });
    }
    // The blink: a triangle between 0.2 and 1 taking 0.8 s each way.
    let phase = (time.elapsed_secs() / 0.8) % 2.0;
    let alpha = 0.2 + 0.8 * if phase < 1.0 { phase } else { 2.0 - phase };
    for (icon, mut image) in &mut icons {
        let want = if icon.residual { alpha } else { 1.0 };
        if (image.color.alpha() - want).abs() > 1e-3 {
            image.color = Color::WHITE.with_alpha(want);
        }
    }
}

/// The game's simple tooltip as the app draws it: its ground, frame and
/// text, with the mouse on an effect's icon.
#[derive(Component)]
struct EffectTooltip;
#[derive(Component)]
struct EffectTooltipText;

/// Shows the tooltip of the effect's icon the mouse is on (`EffectIcon`'s
/// hover: `SimpleTooltip.Show`), its lower left 10 units up and to the
/// right of the mouse (`Tooltip.SetPosition`), kept on the screen
/// (`CorrectPositionResolution`), its words as the game writes them; and
/// hides it as the mouse leaves. Made the first time it is wanted, as the
/// game's scene has it: a black ground 6 units in at the sides, 2 above and
/// 5 below its words, a frame of `border_generic`'s colour, the words in
/// Bender at 14 in their colour.
#[allow(clippy::too_many_arguments)]
fn show_effect_tooltip(
    mut commands: Commands,
    controller: Res<Controller>,
    look: Res<HealthLook>,
    said: Res<Words>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    icons: Query<(&EffectIcon, &RelativeCursorPosition, &InheritedVisibility)>,
    mut tooltip: Query<(Entity, &mut Node, &mut Visibility), With<EffectTooltip>>,
    mut words: Query<(&mut Text, &mut TextFont), With<EffectTooltipText>>,
) {
    let Ok(window) = window.single() else { return };
    let s = window.height() / 1080.0;
    let over = icons.iter().find(|(_, at, shown)| shown.get() && at.cursor_over()).map(|(icon, ..)| icon);
    let (Some(icon), Some(health), Some(cursor)) = (over, controller.health.as_ref(), window.cursor_position()) else {
        for (_, _, mut visibility) in &mut tooltip {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
        }
        return;
    };
    let part = Part::ALL[icon.part];
    let effect = health.effects.iter().find(|e| e.part == part && e.kind == icon.kind && if icon.residual { e.stage == trp_sim::health::Stage::Residual } else { e.existing() });
    let name = said.effect(icon.kind).unwrap_or_else(|| icon.kind.name().to_owned());
    let left = effect.map_or(0.0, |e| e.delay + e.work - e.whole).round();
    let line = if !icon.residual && left > 0.0 && left < 7200.0 { format!("{name} ({left:.0})") } else { name };
    // The game's tooltip's look, where it was read.
    let text = look.tooltip.as_ref().and_then(|panel| panel.nodes.iter().find_map(|node| node.text.as_ref()).map(|text| (panel, text)));
    let (font, size, color) = text.map_or((Handle::default(), 14.0, Color::srgb(0.651, 0.690, 0.710)), |(panel, text)| (look.font(panel, text.font), text.size, rgba(text.color)));
    let frame = look.tooltip.as_ref().and_then(|panel| panel.find("Background/Border")).and_then(|index| look.tooltip.as_ref()?.nodes[index].image.map(|image| rgba(image.color))).unwrap_or(Color::srgb(0.322, 0.349, 0.353));
    // Its lower left 10 up and right of the mouse, kept on the screen.
    let (width, height) = (window.width(), window.height());
    let at = |node: &mut Node| {
        node.left = Val::Px((cursor.x + 10.0 * s).min(width - 40.0 * s).max(0.0));
        node.bottom = Val::Px((height - cursor.y + 10.0 * s).min(height - 30.0 * s).max(0.0));
    };
    match tooltip.single_mut() {
        Ok((_, mut node, mut visibility)) => {
            at(&mut node);
            *visibility = Visibility::Inherited;
            for (mut text, mut text_font) in &mut words {
                if text.0 != line {
                    text.0 = line.clone();
                }
                if text_font.font_size != size * s {
                    text_font.font_size = size * s;
                }
            }
        }
        Err(_) => {
            let mut node = Node {
                position_type: PositionType::Absolute,
                padding: UiRect { left: Val::Px(6.0 * s), right: Val::Px(6.0 * s), top: Val::Px(2.0 * s), bottom: Val::Px(5.0 * s) },
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            };
            at(&mut node);
            commands.spawn((EffectTooltip, node, BackgroundColor(Color::BLACK), BorderColor::all(frame), GlobalZIndex(45))).with_children(|ground| {
                ground.spawn((EffectTooltipText, Text::new(line), TextFont { font, font_size: size * s, ..default() }, TextColor(color), TextLayout::new_with_no_wrap()));
            });
        }
    }
}

// ==== The strip, the gear, the foot, the border, the tab bar, the stash, the quick bar, the containers ====

/// Draws the GEAR tab's strip of figures as the game's (`TARKOV UI`),
/// where the game lays it out in the left panel, as `draw_panel` draws.
pub fn draw_strip(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some((game, strip)) = look.panel.as_ref().and_then(|game| Some((game, game.strip.as_ref()?))) else { return };
    let arrows: Vec<usize> = game.strip_figures.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.up, f.down]).flatten().collect();
    draw_nodes(column, look, strip, &look.strip, STRIP, Marks { arrows: &arrows, ..Marks::default() }, said, Frame::In(left), origin, s);
}

/// Draws the game's equipment panel (`TARKOV UI`) as a panel of that size
/// with its top left at `origin`: its outline of the character and the
/// slot view of each slot the page has (`present`), full or empty as the
/// slot is (`full`), its header in the install's words for the slot
/// (`SlotViewHeader.Show`: the slot's name in capitals). The page's own
/// boxes for the slots, with what is in them, lie over these.
#[allow(clippy::too_many_arguments)]
pub fn draw_gear(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], origin: [f32; 2], s: f32, present: &dyn Fn(&str) -> bool, full: &dyn Fn(&str) -> bool) {
    let Some((game, gear)) = look.panel.as_ref().and_then(|game| Some((game, game.gear_panel.as_ref()?))) else { return };
    let (mut set, mut skip, mut words) = (HashMap::new(), Vec::new(), HashMap::new());
    for (slot, parts) in &game.slot_views {
        if !present(slot) {
            skip.push(parts.view);
            continue;
        }
        let full = full(slot);
        for (part, on) in [(parts.full_border, full), (parts.back_image, !full), (parts.background, !full), (parts.empty_border, !full)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        let text = parts.header.and_then(|header| (header..gear.nodes.len()).find(|&below| gear.nodes[below].text.is_some() && gear.nodes[below].path.starts_with(&gear.nodes[header].path)));
        if let Some(text) = text {
            words.insert(text, said.say(slot, slot).trim().to_uppercase());
        }
    }
    let marks = Marks { set: Some(&set), skip: &skip, words: Some(&words), ..Marks::default() };
    draw_nodes(column, look, gear, &look.gear, GEAR, marks, said, Frame::Is(size), origin, s);
}

/// Draws the left panel's foot (`Bottom Bar`), which lies under both tabs'
/// panels, as the game's (`TARKOV UI`).
pub fn draw_foot(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(foot) = look.panel.as_ref().and_then(|game| game.foot.as_ref()) else { return };
    draw_nodes(column, look, foot, &look.foot, FOOT, Marks::default(), said, Frame::In(left), origin, s);
}

/// The left panel's own ground (`TARKOV UI`): its picture's colour.
pub fn left_ground(look: &HealthLook) -> Option<Color> {
    look.panel.as_ref()?.left_ground.map(rgba)
}

/// Draws the left side's frame round its two panels (`Border`), over
/// them, as the game's (`TARKOV UI`): `side` is the left side's size and
/// `origin` its corner, in units, in what it is drawn into.
pub fn draw_border(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, side: [f32; 2], origin: [f32; 2], s: f32) {
    let Some(border) = look.panel.as_ref().and_then(|game| game.border.as_ref()) else { return };
    draw_nodes(column, look, border, &look.border, BORDER, Marks::default(), said, Frame::In(side), origin, s);
}

/// Draws the game's bar of tabs (`TARKOV UI`) into a node as large as the
/// window (`screen`, units): its ground, then each tab over the one before
/// it, as its chosen version or its idle one (`Tab.UpdateVersion`) and,
/// idle, its graphic under the mouse or its own (`Tab.Hover`), with its
/// words and its fill cut to its mask (`fills`, by the mask's object); and
/// the app's own tab (its words `own`) drawn as the first tab is, without
/// its icon, at the bar's other end as far in. `chosen` and `hovered` are
/// tabs' places in the row, the app's own after the game's.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn draw_tab_bar(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, screen: [f32; 2], chosen: Option<usize>, hovered: Option<usize>, own: &str, dimmed: &[bool], fills: &HashMap<usize, Handle<Image>>, s: f32) {
    let Some(bar) = &look.tab_bar else { return };
    let tabs: Vec<usize> = bar.tabs.iter().map(|tab| tab.object).collect();
    let masks: Vec<usize> = look.tab_fills.keys().copied().collect();
    draw_nodes(column, look, &bar.panel, &look.tabs_drawn, TABS, Marks { skip: &tabs, ..Marks::default() }, said, Frame::In(screen), [0.0, 0.0], s);
    let draw = |column: &mut ChildSpawnerCommands, place: usize, tab: &trp_data::screen::TabParts, words: &str, icon: bool, origin: [f32; 2], mirror: Option<f32>| {
        let (is_chosen, is_hovered) = (chosen == Some(place), hovered == Some(place));
        let mut set = HashMap::new();
        for (part, on) in [(tab.normal, !is_chosen), (tab.selected, is_chosen), (tab.hover, is_hovered), (tab.idle, !is_hovered)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        if !icon {
            let path = &bar.panel.nodes[tab.object].path;
            for index in (0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "Icon" && bar.panel.nodes[index].path.starts_with(path.as_str())) {
                set.insert(index, false);
            }
        }
        let words: HashMap<usize, String> = tab.labels.iter().map(|&label| (label, words.to_owned())).collect();
        // The prestige tab does not work in a raid (`InventoryScreen.Show`:
        // `SetInteractable(... && !InRaid)`): the game draws its group at
        // 0.3 (`SetUnlockStatus`), the whole tab see-through. The tabs the
        // app has not built are dimmed until they are (the user,
        // 2026-10-16: "Let's dim out the tabs that aren't made yet like the
        // prestige one"), and, the user's look for all five, by their words
        // and icon at 0.3 alone, the tab solid ("Dont make the tabs
        // transparent, sorry. just dim them out so they don't overlap
        // eachother").
        let mut groups: HashMap<usize, f32> = HashMap::new();
        if dimmed.get(place).copied().unwrap_or(false) {
            let path = &bar.panel.nodes[tab.object].path;
            let icons = (0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "Icon" && bar.panel.nodes[index].path.starts_with(path.as_str()));
            for part in tab.labels.iter().copied().chain(icons) {
                groups.insert(part, UNLOCKED_NOT);
            }
        }
        let marks = Marks { set: Some(&set), words: Some(&words), within: &[tab.object], skip_below: &masks, fills: Some(fills), groups: Some(&groups), mirror, ..Marks::default() };
        draw_nodes(column, look, &bar.panel, &look.tabs_drawn, TABS + ((place + 1) << 16), marks, said, Frame::In(screen), origin, s);
    };
    // The app's own tab, the options, after the game's last as the row
    // would lay another (the user's place for it, at the bar's right; the
    // game's back button keeps the bar's end), mirrored, its slanted edge to
    // the left (the user, 2026-10-16: "Flip the Options tab square so the
    // edge is on the left").
    let options = |column: &mut ChildSpawnerCommands| {
        if let Some(first) = bar.tabs.first() {
            let [x, _, w, _] = bar.panel.placed_in(first.object, screen).rect;
            let origin = [options_tab_left(look, screen) - x, 0.0];
            draw(column, bar.tabs.len(), first, own, false, origin, Some(x + w / 2.0));
        }
    };
    // In the order the game's tab group leaves them (`TabGroup.SelectTab`:
    // every tab put last in turn from the last to the first, then the one
    // chosen): each tab over the one after it, its slanted end on the
    // next, and the one chosen over all. The app's, as a ninth, under all.
    if chosen != Some(bar.tabs.len()) {
        options(column);
    }
    for (place, tab) in bar.tabs.iter().enumerate().rev().filter(|(place, _)| chosen != Some(*place)) {
        draw(column, place, tab, look.tab_words.get(place).map_or("", String::as_str), true, [0.0, 0.0], None);
    }
    match chosen {
        Some(place) if place == bar.tabs.len() => options(column),
        Some(place) => {
            if let Some(tab) = bar.tabs.get(place) {
                draw(column, place, tab, look.tab_words.get(place).map_or("", String::as_str), true, [0.0, 0.0], None);
            }
        }
        None => {}
    }
    if let Some(back) = &bar.back {
        draw_nodes(column, look, back, &look.back_drawn, BACK_BUTTON, Marks::default(), said, Frame::In(screen), [0.0, 0.0], s);
    }
}

/// The alpha of a tab that does not work (`CanvasGroup.SetUnlockStatus`).
const UNLOCKED_NOT: f32 = 0.3;

/// The left of the app's options tab, in units: after the game's last tab
/// as the row would lay another, less the slant the mirrored tab now has
/// on its left (the outline's right border, `tab_idle_outline`'s 33
/// pixels, which the first tab's next overlaps by the row's gap), so that
/// its slant meets the last tab's.
fn options_tab_left(look: &HealthLook, screen: [f32; 2]) -> f32 {
    let Some(bar) = &look.tab_bar else { return 0.0 };
    let Some(first) = bar.tabs.first() else { return 0.0 };
    let [x, ..] = bar.panel.placed_in(first.object, screen).rect;
    x + (look.tab_width + bar.gap) * bar.tabs.len() as f32 - bar.gap
}

/// Where the screen's back button takes the mouse (`TARKOV UI`), in units
/// from the screen's top left corner.
pub fn back_place(look: &HealthLook, screen: [f32; 2]) -> Option<[f32; 4]> {
    let back = look.tab_bar.as_ref()?.back.as_ref()?;
    Some(back.placed_in(0, screen).rect)
}

/// The game's filters of a container's panel, in their tabs' order
/// (`FilterPanel._filterRules`): the game's item classes each stands for,
/// by the install's database's nodes of the same names; all for the
/// first, none for the last (what was found in the raid, which the app
/// does not keep).
pub const FILTERS: [&[&str]; 18] = [
    &[],
    &["Weapon"],
    &["Magazine"],
    &["Ammo", "AmmoBox"],
    &["Meds"],
    &["FoodAndDrink", "Food", "Drink"],
    &["Knife"],
    &["Mod"],
    &["ThrowWeap"],
    &["BarterItem"],
    &["Vest"],
    &["Visors"],
    &["SearchableItem", "SimpleContainer"],
    &["Armor"],
    &["Info"],
    &["Key"],
    &["SpecItem"],
    &[],
];

/// Where the panel for a container opened (`TARKOV UI`) puts things, in the
/// stash panel's frame of that size (units, from its top left): what of the
/// grids shows (its mask), where the wheel scrolls them (its scroll view),
/// the corner the grids are laid from (the scroll view's content, with its
/// column's padding), its bar, and its scrolling.
pub struct StashPlaces {
    pub mask: [f32; 4],
    pub view: [f32; 4],
    pub content: [f32; 2],
    pub bar: Option<([f32; 4], trp_data::screen::ScrollBar)>,
    pub scrolling: trp_data::screen::Scrolling,
}

pub fn stash_places(look: &HealthLook, size: [f32; 2]) -> Option<StashPlaces> {
    let stash = look.stash.as_ref()?;
    let scrolling = stash.scrolling?;
    let scroll = stash.scroll?;
    let mask = stash.panel.nodes[scroll].parent.map_or_else(|| stash.panel.rect(scroll, size), |mask| stash.panel.rect(mask, size));
    let view = stash.panel.rect(scrolling.view.unwrap_or(scroll), size);
    let content = scrolling.content.or(stash.grids).map_or([view[0], view[1]], |content| {
        let [x, y, ..] = stash.panel.rect(content, size);
        let padding = stash.panel.nodes[content].group.map_or([0.0; 4], |group| group.padding);
        [x + padding[0], y + padding[2]]
    });
    let bar = scrolling.bar.map(|bar| (stash.panel.rect(bar.object, size), bar));
    Some(StashPlaces { mask, view, content, bar, scrolling })
}

/// Draws the panel for a container opened (`TARKOV UI`) as the game's in a
/// raid, into the stash panel's place of that size: under the grids
/// (`layer` 0) its ground, its header with what it holds (the install's
/// word for `LOOT`); over them (1) its frame, its column of filters (all
/// chosen; those of a kind the container holds nothing of at 0.15,
/// `present`) and the container's name in its box, sized to the name as
/// the game's text sizes it (`ContentSizeFitter`).
#[allow(clippy::too_many_arguments)]
pub fn draw_stash(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], name: &str, present: &[bool], fills: &HashMap<usize, Handle<Image>>, layer: u8, s: f32) {
    let Some(stash) = &look.stash else { return };
    let mut panel = stash.panel.clone();
    // The name's box, as wide and high as its words with its padding.
    if let (Some(name_panel), Some(text), Some(font)) = (stash.name_panel, stash.name, look.name_font.as_ref()) {
        if let Some(look_of) = panel.nodes[text].text.clone() {
            let words = if look_of.style & 16 != 0 { name.to_uppercase() } else { name.to_owned() };
            let own = [font.width(&words, look_of.size, look_of.spacing, false) + look_of.margin[0] + look_of.margin[2], font.height(look_of.size)];
            panel.nodes[text].element = Some(trp_data::screen::Element { ignore: false, min: own, preferred: own, flexible: [-1.0, -1.0] });
            let padding = panel.nodes[name_panel].group.map_or([0.0; 4], |group| group.padding);
            panel.nodes[name_panel].layout.size_delta = [own[0] + padding[0] + padding[1], own[1] + padding[2] + padding[3]];
        }
    }
    let mut set = HashMap::new();
    let mut alphas = HashMap::new();
    for (place, tab) in stash.filter_tabs.iter().enumerate() {
        for (part, on) in [(tab.normal, place != 0), (tab.selected, place == 0)] {
            if let Some(part) = part {
                set.insert(part, on);
            }
        }
        if !present.get(place).copied().unwrap_or(place == 0) {
            for part in [tab.target, tab.icon].into_iter().flatten() {
                alphas.insert(part, 0.15);
            }
        }
    }
    let mut words = HashMap::new();
    if let Some(grid_name) = stash.grid_name {
        words.insert(grid_name, said.say("LOOT", "LOOT"));
    }
    if let Some(text) = stash.name {
        words.insert(text, name.to_owned());
    }
    // The header's toggle lays out its icon (`StashHeaderIcons_Off`, the
    // loot's mark) and its word by what they ask, which the layout here
    // does not ask of them itself: they were drawn at no size (the user's
    // "loot icon", 2026-10-21 and 2026-10-23).
    let top_panel = panel.find("Simple Panel/TopPanel");
    look.ask_by_content_under(&mut panel, &look.stash_drawn, &words, top_panel);
    let over: Vec<usize> = (0..panel.nodes.len()).filter(|&index| panel.nodes[index].parent == Some(stash.simple) && ["Border"].contains(&panel.nodes[index].name.as_str())).chain(stash.filters).chain(stash.name_panel).collect();
    let marks = match layer {
        0 => Marks { set: Some(&set), words: Some(&words), within: &[stash.simple], skip: &over, fills: Some(fills), alphas: Some(&alphas), ..Marks::default() },
        _ => Marks { set: Some(&set), words: Some(&words), within: &over, fills: Some(fills), alphas: Some(&alphas), ..Marks::default() },
    };
    draw_nodes(column, look, &panel, &look.stash_drawn, STASH + ((layer as usize) << 16), marks, said, Frame::Is(size), [0.0, 0.0], s);
}

/// The crate panel's stripes cut to their masks (`TARKOV UI`), as images so
/// many pixels to a unit, by the mask's object, in a stash panel so large.
pub fn stash_fill_images(look: &HealthLook, size: [f32; 2], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    let Some(stash) = &look.stash else { return HashMap::new() };
    fill_images(&look.stash_fills, |mask| stash.panel.rect(mask, size), s, images)
}

/// Fills cut to their masks as images, each as large as its mask's object
/// (`rect`, units) at so many pixels to a unit.
fn fill_images(fills: &HashMap<usize, std::sync::Arc<MaskedFill>>, rect: impl Fn(usize) -> [f32; 4], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    fills
        .iter()
        .map(|(&mask, fill)| {
            let [_, _, w, h] = rect(mask);
            let pixels = [((w * s).round() as u32).max(1), ((h * s).round() as u32).max(1)];
            let image = Image::new(
                bevy::render::render_resource::Extent3d { width: pixels[0], height: pixels[1], depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                fill.compose([w, h], pixels),
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            (mask, images.add(image))
        })
        .collect()
}

/// Draws the quick-use panel (`TARKOV UI`) as the game's into its place of
/// that size: its header (its words the install's), and each view with
/// its key's words (`HotKey`, the binding's keys) and the name of what is
/// bound to it (`Caption`, its short name; none for nothing), by the
/// binding's name in the game's `EBoundItem` (`words`). Under what is put
/// in the views (`layer` 0) the panel but each view's caption, key label
/// and border, which lie over it (1).
#[allow(clippy::too_many_arguments)]
pub fn draw_quick(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], words: &HashMap<String, (String, String)>, layer: u8, s: f32) {
    let Some(quick) = &look.quick else { return };
    draw_quick_views(column, look, quick, &look.quick_drawn, QUICK, said, size, words, None, layer, s);
}

/// Draws a quick-use panel's views (the inventory's, or the battle
/// screen's bar), as `draw_quick` draws the inventory's; `selected`, the
/// keys of the views whose thing is in the hands, which a view lights
/// where it has a ground for it (`QuickSlotView.SwitchVisualSelection`:
/// the ground white and the caption black where selected, else the ground
/// black at 0.6 and the caption white; the caption in the normal style).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_quick_views(
    column: &mut ChildSpawnerCommands,
    look: &HealthLook,
    quick: &trp_data::screen::QuickViews,
    drawn: &Drawn,
    base: usize,
    said: &Words,
    size: [f32; 2],
    words: &HashMap<String, (String, String)>,
    selected: Option<&[String]>,
    layer: u8,
    s: f32,
) {
    let mut colors = HashMap::new();
    let mut unbold = Vec::new();
    if let Some(selected) = selected {
        for slot in quick.slots.iter().filter(|slot| slot.background.is_some()) {
            let lit = selected.contains(&slot.key);
            if let Some(ground) = slot.background {
                colors.insert(ground, if lit { Color::WHITE } else { Color::srgba(0.0, 0.0, 0.0, 0.6) });
            }
            if let Some(caption) = slot.caption {
                colors.insert(caption, if lit { Color::BLACK } else { Color::WHITE });
                unbold.push(caption);
            }
        }
    }
    let mut texts = HashMap::new();
    for slot in &quick.slots {
        let Some((key, caption)) = words.get(&slot.key) else { continue };
        if let Some(hot_key) = slot.hot_key {
            texts.insert(hot_key, key.clone());
        }
        if let Some(text) = slot.caption {
            texts.insert(text, caption.clone());
        }
    }
    let headers: Vec<usize> = quick.panel.find("Header/Text").into_iter().collect();
    let over: Vec<usize> = quick.slots.iter().flat_map(|slot| [slot.caption, slot.hot_key.and_then(|key| quick.panel.nodes[key].parent), slot.border]).flatten().collect();
    let marks = match layer {
        0 => Marks { words: Some(&texts), headers: &headers, skip: &over, colors: Some(&colors), unbold: &unbold, ..Marks::default() },
        _ => Marks { words: Some(&texts), headers: &headers, within: &over, colors: Some(&colors), unbold: &unbold, ..Marks::default() },
    };
    draw_nodes(column, look, &quick.panel, drawn, base + ((layer as usize) << 16), marks, said, Frame::Is(size), [0.0, 0.0], s);
}

/// Where each quick-use view puts its thing (`InstallPlace`), by its
/// binding's name, in the panel's place of that size (units).
pub fn quick_places(look: &HealthLook, size: [f32; 2]) -> Vec<(String, [f32; 4])> {
    let Some(quick) = &look.quick else { return Vec::new() };
    quick.slots.iter().filter_map(|slot| Some((slot.key.clone(), quick.panel.rect(slot.install?, size)))).collect()
}

/// Where the bar's tabs lie (`TARKOV UI`) in a window so large (units):
/// each tab's rectangle (left, top, width, height) in the row's order, the
/// app's own last, at the bar's other end.
pub fn tab_places(look: &HealthLook, screen: [f32; 2]) -> Vec<[f32; 4]> {
    let Some(bar) = &look.tab_bar else { return Vec::new() };
    let mut places: Vec<[f32; 4]> = bar.tabs.iter().map(|tab| bar.panel.placed_in(tab.object, screen).rect).collect();
    if let Some(&[_, top, width, height]) = places.first() {
        places.push([options_tab_left(look, screen), top, width, height]);
    }
    places
}

/// Each tab's fill cut to its mask (`TARKOV UI`), as an image so many
/// pixels to a unit, by the mask's object.
pub fn tab_fill_images(look: &HealthLook, screen: [f32; 2], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    let Some(bar) = &look.tab_bar else { return HashMap::new() };
    fill_images(&look.tab_fills, |mask| bar.panel.placed_in(mask, screen).rect, s, images)
}

/// The containers' column's ground: the colour of its scroll view's
/// parent (`Scrollview Parent`, a plain image), which the page's column
/// takes as its own.
pub fn containers_ground(look: &HealthLook) -> Option<Color> {
    let views = look.containers.as_ref()?;
    let ground = views.panel.find("Scrollview Parent")?;
    views.panel.nodes[ground].image.map(|image| rgba(image.color))
}

/// One container slot's row as the game's row template draws it: its
/// header (the slot's name in the install's words, in capitals) and its
/// slot panel (its empty picture, `Slots/Equipment/<slot>`, its fill and
/// its borders, full or empty), with the slot panel's top left at `at`
/// (units, in what it is drawn into). The grids are drawn apart
/// (`draw_grid_view`), where `container_grids` puts them.
#[allow(clippy::too_many_arguments)]
pub fn draw_container_row(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], slot: &str, full: bool, panel_shown: bool, at: [f32; 2], s: f32) {
    let Some(views) = &look.containers else { return };
    let (Some(header), Some(slot_panel)) = (views.header, views.slot_panel) else { return };
    let panel = &views.panel;
    let [x, y, ..] = panel.rect(slot_panel, left);
    let origin = [at[0] - x, at[1] - y];
    let path = &panel.nodes[slot_panel].path;
    let mut set = HashMap::new();
    let mut pictures = HashMap::new();
    for (name, on) in [("Full Border", full), ("Empty Border", !full), ("Background", !full)] {
        if let Some(part) = panel.find(&format!("{path}/{name}")) {
            set.insert(part, on);
        }
    }
    if let Some(back) = panel.find(&format!("{path}/BackImage")) {
        match look.container_backs.get(slot) {
            Some(picture) => {
                pictures.insert(back, *picture);
                set.insert(back, !full);
            }
            None => {
                set.insert(back, false);
            }
        }
    }
    let header_path = &panel.nodes[header].path;
    let arrow: Vec<usize> = panel.find(&format!("{header_path}/ArrowHolder")).into_iter().collect();
    let mut words = HashMap::new();
    if let Some(text) = (header..panel.nodes.len()).find(|&below| panel.nodes[below].text.is_some() && panel.nodes[below].path.starts_with(header_path.as_str())) {
        words.insert(text, said.say(slot, slot).trim().to_uppercase());
    }
    // A thing not shown in its slot (`Item.NotShownInSlot`: the pockets)
    // has the slot's panel off (`SlotView.Show`, its `_slotPlace`).
    let within = if panel_shown { vec![header, slot_panel] } else { vec![header] };
    let marks = Marks { set: Some(&set), words: Some(&words), within: &within, pictures: Some(&pictures), skip: &arrow, ..Marks::default() };
    draw_nodes(column, look, panel, &look.containers_drawn, CONTAINERS, marks, said, Frame::Is(left), origin, s);
}

/// The key of the special slots' header's words
/// (`SearchableSlotView.CreateSlots`).
const SPECIAL_SLOTS_HEADER: &str = "InventoryScreen/SpecialSlotsHeader";

/// A container's special slots as the game's row shows them
/// (`SearchableSlotView.CreateSlots`, its `_specSlotsPanel` turned on and
/// laid last in the row): the panel's header over its left (the words
/// `InventoryScreen/SpecialSlotsHeader` in capitals), and a view for each
/// slot (`_specSlotTemplate`, 64 units square, 10 apart in the panel's row):
/// its striped fill, its empty border a unit outside and its picture
/// (`SpecialSlot`) at its own size in its middle. `at` is the panel's top
/// left corner in units; each view's place is given back.
pub fn draw_special_slots(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], at: [f32; 2], count: usize, s: f32) -> Vec<[f32; 4]> {
    let places: Vec<[f32; 4]> = (0..count).map(|n| [at[0] + n as f32 * (SPECIAL_SIDE + SPECIAL_GAP), at[1], SPECIAL_SIDE, SPECIAL_SIDE]).collect();
    let Some(views) = &look.containers else { return places };
    let panel = &views.panel;
    let ends = |end: &str| (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end));
    if let (Some(template), Some(header)) = (ends("Slot Template/Special Slots Panel Template"), ends("Special Slots Panel Template/Header Panel/Header")) {
        let [x, y, ..] = panel.rect(template, left);
        let mut words = HashMap::new();
        if let Some(text) = (header..panel.nodes.len()).find(|&below| panel.nodes[below].text.is_some() && panel.nodes[below].path.starts_with(panel.nodes[header].path.as_str())) {
            words.insert(text, said.say(SPECIAL_SLOTS_HEADER, "Special slots").trim().to_uppercase());
        }
        let marks = Marks { words: Some(&words), within: &[header], ..Marks::default() };
        draw_nodes(column, look, panel, &look.containers_drawn, CONTAINERS + (1 << 16), marks, said, Frame::Is(left), [at[0] - x, at[1] - y], s);
    }
    let sprite = |end: &str| -> Option<Sprite> {
        let index = ends(&format!("Special Slot Template/{end}"))?;
        let picture = look.containers_drawn.pictures.get(index).copied().flatten()?;
        let image = look.images.get(picture.texture)?.clone()?;
        Some(Sprite { image, cut: picture.cut, size: [panel.nodes[index].layout.size_delta[0].max(picture.size[0]), panel.nodes[index].layout.size_delta[1].max(picture.size[1])], border: picture.border, color: panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color)) })
    };
    let (fill, border, back) = (sprite("Background"), sprite("Empty Border"), sprite("BackImage"));
    for &[x, y, w, h] in &places {
        if let Some(fill) = &fill {
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
            let image = ImageNode { image: fill.image.clone(), color: fill.color, rect: Some(fill.cut), flip_y: true, image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: s }, ..default() };
            column.spawn((node, image));
        }
        if let Some(border) = &border {
            draw_sprite(column, border, [(x - 1.0) * s, (y - 1.0) * s, (w + 2.0) * s, (h + 2.0) * s], 1.0);
        }
        if let Some(back) = &back {
            let [bw, bh] = back.size;
            draw_sprite(column, back, [(x + (w - bw) / 2.0) * s, (y + (h - bh) / 2.0) * s, bw * s, bh * s], s);
        }
    }
    places
}

/// A special slot's view's side and the gap between two in their panel's
/// row (`Special Slot Template`'s `LayoutElement`, the panel's
/// `HorizontalLayoutGroup`), in units.
const SPECIAL_SIDE: f32 = 64.0;
const SPECIAL_GAP: f32 = 10.0;

/// The game's view of one grid (`GridView`): its tile of cells and its
/// border, as large as the grid, its top left at `at`.
pub fn draw_grid_view(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, at: [f32; 2], size: [f32; 2], s: f32) {
    let Some(grid) = look.containers.as_ref().and_then(|views| views.grid.as_ref()) else { return };
    draw_nodes(column, look, grid, &look.grid_drawn, GRID, Marks::default(), said, Frame::Is(size), at, s);
}

/// Where a container's grids lie to the right of its slot panel, from the
/// row's grids' corner: its own layout's places where it has one with as
/// many grids (`TemplatedGridsView`), else the default row
/// (`GeneratedGridsView`'s row: each grid as large as its cells, 63 units
/// to a cell and one more, so far apart). Each left, top, width, height.
pub fn container_grids(look: &HealthLook, layout: &str, grids: &[[u32; 2]]) -> Vec<[f32; 4]> {
    if let Some((_, rects)) = look.layouts.get(layout).filter(|(_, rects)| rects.len() >= grids.len()) {
        return rects[..grids.len()].to_vec();
    }
    let row = look.containers.as_ref().and_then(|views| views.grids_row).unwrap_or_default();
    let mut x = row.padding[0];
    grids
        .iter()
        .map(|cells| {
            let size = [cells[0] as f32 * 63.0 + 1.0, cells[1] as f32 * 63.0 + 1.0];
            let rect = [x, row.padding[2], size[0], size[1]];
            x += size[0] + row.spacing;
            rect
        })
        .collect()
}

/// How far right of a container slot's panel its grids begin: the panel's
/// width and the row template's spacing (`Slot Template`'s row).
pub fn grids_after(look: &HealthLook) -> f32 {
    let Some(views) = &look.containers else { return 132.0 };
    let width = views.slot_panel.map_or(125.0, |panel| views.panel.nodes[panel].element.map_or(125.0, |e| e.min[0].max(0.0)));
    width + views.panel.nodes[views.template].group.map_or(7.0, |row| row.spacing)
}

// ==== Nine-slices, marks, frames, the nodes drawn, rich spans ====

/// A sliced picture: its texture and cut, its border (left, bottom,
/// right, top) in the sprite's pixels, the screen's pixels to one of
/// them, whether its middle is drawn, and its colour.
struct Slices {
    image: Handle<Image>,
    cut: Rect,
    border: [f32; 4],
    per: f32,
    fill_center: bool,
    color: Color,
    /// Mirrored across its frame's middle: each piece where its mirror
    /// image lies, its picture flipped.
    flip_x: bool,
    /// Its edges and middle repeated at the sprite's size, not stretched
    /// (`Image.Type.Tiled`).
    tiled: bool,
}

/// One piece of a sliced picture, by the drawn object it is of.
#[derive(Component)]
struct SliceOf(usize);

/// A sliced picture in a frame of that size, in pixels, as nine pieces:
/// the corners whole, the edges and the middle stretched (the texture
/// kept as the engine has it, its lowest row first, and drawn turned up).
fn nine_slice(frame: &mut ChildSpawnerCommands, slices: &Slices, size: [f32; 2], index: usize) {
    let [l, b, r, t] = slices.border;
    let (x0, y0, x1, y1) = (slices.cut.min.x, slices.cut.min.y, slices.cut.max.x, slices.cut.max.y);
    let (mut left, mut right, mut top, mut bottom) = (l * slices.per, r * slices.per, t * slices.per, b * slices.per);
    if left + right > size[0] && left + right > 0.0 {
        let k = size[0] / (left + right);
        (left, right) = (left * k, right * k);
    }
    if top + bottom > size[1] && top + bottom > 0.0 {
        let k = size[1] / (top + bottom);
        (top, bottom) = (top * k, bottom * k);
    }
    let columns = [(x0, x0 + l, 0.0, left), (x0 + l, x1 - r, left, size[0] - left - right), (x1 - r, x1, size[0] - right, right)];
    let rows = [(y1 - t, y1, 0.0, top), (y0 + b, y1 - t, top, size[1] - top - bottom), (y0, y0 + b, size[1] - bottom, bottom)];
    // A middle with no pixels of its own (a sprite all border, as the
    // skills' bars' `level_fill_bar`, 8 by 8 with 4 all round) is the
    // texture at the line between its borders, stretched: the engine's
    // sliced mesh gives its middle that line's coordinates. A pixel's
    // width about the line stands for it.
    let widened = |a: f32, b: f32| if b - a < 1e-3 { ((a + b) / 2.0 - 0.5, (a + b) / 2.0 + 0.5) } else { (a, b) };
    for (row, (ya, yb, at_y, h)) in rows.into_iter().enumerate() {
        for (column, (xa, xb, at_x, w)) in columns.into_iter().enumerate() {
            if (row == 1 && column == 1 && !slices.fill_center) || w <= 0.0 || h <= 0.0 || xb < xa || yb < ya {
                continue;
            }
            let ((xa, xb), (ya, yb)) = (widened(xa, xb), widened(ya, yb));
            let at_x = if slices.flip_x { size[0] - at_x - w } else { at_x };
            let node = Node { position_type: PositionType::Absolute, left: Val::Px(at_x), top: Val::Px(at_y), width: Val::Px(w), height: Val::Px(h), ..default() };
            let mut piece = ImageNode { image: slices.image.clone(), color: slices.color, rect: Some(Rect::new(xa, ya, xb, yb)), flip_x: slices.flip_x, flip_y: true, ..default() };
            if slices.tiled && (row == 1 || column == 1) {
                piece.image_mode = NodeImageMode::Tiled { tile_x: column == 1, tile_y: row == 1, stretch_value: slices.per };
            }
            frame.spawn((SliceOf(index), node, piece));
        }
    }
}

/// What of a panel is drawn as its scripts have it: its rates' arrows,
/// the views' glows, the headers whose texts are keys to the install's
/// words; objects shown or hidden as the caller has them (`set`), those
/// not drawn with all under them (`skip`), and texts the caller writes
/// (`words`).
#[derive(Default)]
pub(crate) struct Marks<'a> {
    pub(crate) arrows: &'a [usize],
    pub(crate) glows: &'a [usize],
    pub(crate) headers: &'a [usize],
    pub(crate) set: Option<&'a HashMap<usize, bool>>,
    pub(crate) skip: &'a [usize],
    pub(crate) words: Option<&'a HashMap<usize, String>>,
    /// Only what lies in these objects (themselves among it), where any
    /// are named; and none of what lies under these.
    pub(crate) within: &'a [usize],
    pub(crate) skip_below: &'a [usize],
    /// Pictures the caller puts on objects (a slot's empty picture), drawn
    /// at their own size about the object's middle (`Image.SetNativeSize`).
    pub(crate) pictures: Option<&'a HashMap<usize, Picture>>,
    /// Sprites the caller swaps into objects (`Image.sprite` and no more: a
    /// menu line's icon), drawn in the object's own rectangle, kept to the
    /// sprite's shape where the image keeps its shape.
    pub(crate) sprites: Option<&'a HashMap<usize, Picture>>,
    /// Images the caller has made for objects, drawn over the whole of
    /// each in place of what the object draws (a tab's fill cut to its
    /// mask, `MaskedFill`).
    pub(crate) fills: Option<&'a HashMap<usize, Handle<Image>>>,
    /// The alpha a script puts on objects' pictures (`ChangeImageAlpha`: a
    /// filter that does not work at 0.15).
    pub(crate) alphas: Option<&'a HashMap<usize, f32>>,
    /// The alpha a script puts on objects' groups (`CanvasGroup.alpha`: a
    /// tab that does not work at 0.3, `SetUnlockStatus`), which every
    /// picture and text in the object or under it is drawn at besides.
    pub(crate) groups: Option<&'a HashMap<usize, f32>>,
    /// Drawn mirrored across this line, in the panel's units before the
    /// origin is added (the app's options tab, its slanted edge to the
    /// left: the user's).
    pub(crate) mirror: Option<f32>,
    /// The colour a script puts on objects' pictures or texts (a quick
    /// view's ground and caption for the thing in the hands,
    /// `QuickSlotView.SwitchVisualSelection`), and texts it sets to the
    /// normal style (`fontStyle = Normal`).
    pub(crate) colors: Option<&'a HashMap<usize, Color>>,
    pub(crate) unbold: &'a [usize],
}

/// How a panel's top lies: by its own anchors in what it hangs from, so
/// large (the left panel), or as a rectangle of that size.
#[derive(Clone, Copy)]
pub(crate) enum Frame {
    In([f32; 2]),
    Is([f32; 2]),
}

/// Each object of one of the game's panels that shows, where the game lays
/// it out in a left panel of that size, its picture as the engine draws it
/// and its text, numbered from `base` among the drawn pieces.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_nodes(column: &mut ChildSpawnerCommands, look: &HealthLook, panel: &trp_data::screen::ScenePanel, drawn: &Drawn, base: usize, marks: Marks, said: &Words, frame: Frame, origin: [f32; 2], s: f32) {
    let at = |x: f32, y: f32, w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Px((origin[0] + x) * s),
        top: Val::Px((origin[1] + y) * s),
        width: Val::Px(w * s),
        height: Val::Px(h * s),
        ..default()
    };
    for (index, node) in panel.nodes.iter().enumerate() {
        let mut visibility = match drawn.shown.get(index) {
            None | Some(Shown::Never) => continue,
            Some(Shown::Always) => Visibility::Inherited,
            Some(Shown::ByScript) => Visibility::Hidden,
        };
        // Left out with all under it, or shown as the caller has it; hidden
        // with all under it where the caller hides something above it (an
        // object off hides what hangs from it).
        let mut above = Some(index);
        let (mut skipped, mut inside, mut hidden) = (false, marks.within.is_empty(), false);
        while let Some(at) = above {
            skipped |= marks.skip.contains(&at) || (at != index && marks.skip_below.contains(&at));
            inside |= marks.within.contains(&at);
            hidden |= at != index && marks.set.and_then(|set| set.get(&at)) == Some(&false);
            above = panel.nodes[at].parent;
        }
        if skipped || !inside {
            continue;
        }
        let mut group = 1.0;
        let mut above = Some(index);
        while let Some(at) = above {
            group *= marks.groups.and_then(|groups| groups.get(&at)).copied().unwrap_or(1.0);
            above = panel.nodes[at].parent;
        }
        if let Some(&on) = marks.set.and_then(|set| set.get(&index)) {
            visibility = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
        if hidden {
            visibility = Visibility::Hidden;
        }
        let piece = Piece(base + index);
        let mut placed = match frame {
            Frame::In(outer) => panel.placed_in(index, outer),
            Frame::Is(size) => panel.placed(index, size),
        };
        // A picture whose border is kept a pixel to a pixel
        // (`PixelPerfectSpriteScaler.UpdateScale`): its offsets from the
        // edges of what it hangs from, on the sides it scales
        // (`_sidesToScale`, all unless set), divided by the screen's scale,
        // so it lies that many pixels out, not units (a quick-use view's
        // shadowed frame, five pixels about it).
        if let (Some((_, fields)), Some(parent)) = (node.scripts.iter().find(|(class, _)| class == "PixelPerfectSpriteScaler"), node.parent) {
            let sides = fields.get(..4).and_then(|b| b.try_into().ok()).map_or(0xF, i32::from_le_bytes);
            let [px, py, pw, ph] = match frame {
                Frame::In(outer) => panel.placed_in(parent, outer).rect,
                Frame::Is(size) => panel.placed(parent, size).rect,
            };
            let [x, y, w, h] = placed.rect;
            let (mut left, mut top, mut right, mut bottom) = (x, y, x + w, y + h);
            let k = 1.0 / s.max(1e-3);
            if sides & 1 != 0 {
                top = py - (py - top) * k;
            }
            if sides & 2 != 0 {
                left = px - (px - left) * k;
            }
            if sides & 4 != 0 {
                bottom = py + ph + (bottom - (py + ph)) * k;
            }
            if sides & 8 != 0 {
                right = px + pw + (right - (px + pw)) * k;
            }
            placed.rect = [left, top, right - left, bottom - top];
        }
        if let Some(own) = marks.pictures.and_then(|pictures| pictures.get(&index)) {
            let [x, y, w, h] = placed.rect;
            placed.rect = [x + (w - own.size[0]) / 2.0, y + (h - own.size[1]) / 2.0, own.size[0], own.size[1]];
        }
        // A simple picture that keeps its sprite's shape
        // (`Image.preserveAspect`): narrowed or lowered to it, moved by the
        // object's pivot (`PreserveSpriteAspectRatio`). The sprite is the one
        // swapped in, where one is.
        let sprite = marks.sprites.and_then(|sprites| sprites.get(&index).copied()).or_else(|| drawn.pictures.get(index).copied().flatten());
        if let Some(picture) = node.image.filter(|image| image.preserve_aspect && image.kind == 0).and(sprite) {
            let [x, y, w, h] = placed.rect;
            let (sprite, rect) = (picture.size[0] / picture.size[1].max(1e-3), w / h.max(1e-3));
            let pivot = node.layout.pivot;
            placed.rect = if sprite > rect {
                let height = w / sprite;
                [x, y + (h - height) * (1.0 - pivot[1]), w, height]
            } else {
                let width = h * sprite;
                [x + (w - width) * pivot[0], y, width, h]
            };
        }
        // Turned with what it hangs from (a slider's handle, turned to
        // point at its bar, and the arrow under it): its middle turned
        // about the pivot of each turned object above it, nearest first,
        // and it turned as much besides.
        let mut turn = node.layout.turn;
        let mut above = node.parent;
        while let Some(at) = above {
            let layout = panel.nodes[at].layout;
            if layout.turn.abs() > 1e-3 {
                let [ax, ay, aw, ah] = match frame {
                    Frame::In(outer) => panel.placed_in(at, outer).rect,
                    Frame::Is(size) => panel.placed(at, size).rect,
                };
                let pivot = [ax + layout.pivot[0] * aw, ay + (1.0 - layout.pivot[1]) * ah];
                let [x, y, w, h] = placed.rect;
                let (dx, dy) = (x + w / 2.0 - pivot[0], y + h / 2.0 - pivot[1]);
                let (sin, cos) = layout.turn.to_radians().sin_cos();
                placed.rect = [pivot[0] + dx * cos + dy * sin - w / 2.0, pivot[1] - dx * sin + dy * cos - h / 2.0, w, h];
                turn += layout.turn;
            }
            above = panel.nodes[at].parent;
        }
        if let Some(axis) = marks.mirror {
            let [x, y, w, h] = placed.rect;
            placed.rect = [2.0 * axis - x - w, y, w, h];
            placed.flip[0] = !placed.flip[0];
        }
        let [x, y, w, h] = placed.rect;
        // Over a button of the game's the pointer takes its hover look
        // (`cursor::point`).
        if let Some(resets) = crate::cursor::hover_of(&node.scripts) {
            column.spawn((crate::cursor::HoverPointer(base + index, resets), at(x, y, w, h), bevy::ui::RelativeCursorPosition::default(), visibility));
        }
        if let Some(handle) = marks.fills.and_then(|fills| fills.get(&index)) {
            let fill = ImageNode { flip_x: placed.flip[0], ..ImageNode::new(handle.clone()).with_color(Color::WHITE.with_alpha(group)) };
            column.spawn((piece, at(x, y, w, h), EXACT, fill, visibility));
            continue;
        }
        // A mask that does not show its own picture (`Mask.showMaskGraphic`
        // off) draws nothing; what hangs from it is cut to it.
        let hidden_mask = node.scripts.iter().any(|(class, fields)| class == "Mask" && fields.first() == Some(&0));
        if let Some(image) = node.image.as_ref().filter(|_| !hidden_mask) {
            let mut color = marks.colors.and_then(|colors| colors.get(&index)).copied().unwrap_or_else(|| rgba(image.color));
            if let Some(&alpha) = marks.alphas.and_then(|alphas| alphas.get(&index)) {
                color = color.with_alpha(alpha);
            }
            if let (true, Some(alpha)) = (marks.glows.contains(&index), look.glow_alpha) {
                color = color.with_alpha(alpha);
            }
            color = color.with_alpha(color.alpha() * group);
            let own = marks.pictures.and_then(|pictures| pictures.get(&index).copied()).or_else(|| marks.sprites.and_then(|sprites| sprites.get(&index).copied()));
            let picture = own.or_else(|| drawn.pictures.get(index).copied().flatten()).and_then(|picture| Some((picture, look.images.get(picture.texture)?.clone()?)));
            // (A sprite a script puts on it counts as its own: a slot's
            // picture on an image kept with none.)
            match (image.sprite.or(own.map(|_| (0, 0))), picture) {
                // No sprite: the engine's plain white, coloured.
                (None, _) => {
                    column.spawn((piece, at(x, y, w, h), EXACT, BackgroundColor(color), visibility));
                }
                (Some(_), Some((picture, handle))) => {
                    // The cut's share of the rectangle, mirrored with it.
                    let mut c = picture.covers;
                    if placed.flip[0] {
                        c = [1.0 - c[2], c[1], 1.0 - c[0], c[3]];
                    }
                    if placed.flip[1] {
                        c = [c[0], 1.0 - c[3], c[2], 1.0 - c[1]];
                    }
                    let (dx, dy, dw, dh) = (x + w * c[0], y + h * (1.0 - c[3]), w * (c[2] - c[0]), h * (c[3] - c[1]));
                    let mut drawn = ImageNode { image: handle.clone(), color, rect: Some(picture.cut), flip_x: placed.flip[0], flip_y: !placed.flip[1], ..default() };
                    // Sliced: nine pieces, the border kept whole and its
                    // middle left out unless the image fills it
                    // (`Image.fillCenter`); the border a pixel to a pixel
                    // of the screen where the object keeps it so
                    // (`PixelPerfectSpriteScaler`), else a unit to a pixel
                    // of the sprite over its multiplier.
                    let [l, b, r, t] = picture.border;
                    // Tiled with a border (`Image.GenerateTiledSprite`):
                    // as sliced, the border whole, but its edges and its
                    // middle repeated at their own size, not stretched (the
                    // quick-use panel's header, its slanted end in its
                    // right border).
                    if (image.kind == 1 || image.kind == 2) && (l > 0.0 || b > 0.0 || r > 0.0 || t > 0.0) {
                        let pixel_perfect = node.scripts.iter().any(|(class, _)| class == "PixelPerfectSpriteScaler");
                        let per = if pixel_perfect { 1.0 } else { s / image.pixels_per_unit.max(1e-3) };
                        let slices = Slices { image: handle, cut: picture.cut, border: picture.border, per, fill_center: image.fill_center, color, flip_x: placed.flip[0], tiled: image.kind == 2 };
                        column.spawn((piece, at(dx, dy, dw, dh), EXACT, visibility)).with_children(|frame| nine_slice(frame, &slices, [dw * s, dh * s], base + index));
                        continue;
                    }
                    // Tiled: the sprite repeated at its own size, a unit
                    // to a pixel.
                    if image.kind == 2 {
                        drawn.image_mode = NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: s };
                    }
                    // Filled across from the left (`Image.Type.Filled`,
                    // horizontal, from the left): a clip as wide as the
                    // share filled, the picture whole inside it.
                    if image.kind == 3 && image.fill_method == 0 {
                        let clip = Node { overflow: Overflow::clip(), ..at(dx, dy, dw * image.fill_amount, dh) };
                        column.spawn((piece, FullWidth(dw * s), clip, EXACT, visibility)).with_children(|clip| {
                            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(dw * s), height: Val::Px(dh * s), ..default() };
                            clip.spawn((PieceFill(base + index), whole, drawn));
                        });
                    } else if marks.arrows.contains(&index) {
                        column.spawn((piece, Bob((origin[1] + dy) * s, s), at(dx, dy, dw, dh), EXACT, drawn, visibility));
                    } else {
                        // Turned about its middle where the scene turns it
                        // (the sheath's picture, 45 degrees): the engine's
                        // counter-clockwise, Bevy's clockwise.
                        let mut piece = column.spawn((piece, at(dx, dy, dw, dh), EXACT, drawn, visibility));
                        if turn.abs() > 1e-3 {
                            piece.insert(UiTransform { rotation: Rot2::degrees(-turn), ..UiTransform::IDENTITY });
                        }
                    }
                }
                // A sprite that could not be read: left out.
                (Some(_), None) => {}
            }
        }
        if let Some(text) = &node.text {
            let justify = match text.horizontal {
                2 => JustifyContent::Center,
                4 => JustifyContent::FlexEnd,
                _ => JustifyContent::FlexStart,
            };
            let align = match text.vertical {
                256 => AlignItems::FlexStart,
                1024 => AlignItems::FlexEnd,
                _ => AlignItems::Center,
            };
            // A view's header says what the install's words have for its
            // own text (`LocalizedText`); every other text is as the scene
            // has it until a script writes to it.
            let mut said_now = match marks.words.and_then(|words| words.get(&index)) {
                Some(words) => words.clone(),
                None if marks.headers.contains(&index) => said.say(&text.text, &text.text),
                None => text.text.clone(),
            };
            if text.style & 16 != 0 {
                said_now = said_now.to_uppercase();
            }
            // In the game's own font where it is had.
            let style = if marks.unbold.contains(&index) { text.style & !1 } else { text.style };
            let font = TextFont { font: look.styled_font(panel, text.font, style), font_size: text.size * s, weight: weight_of(style), ..default() };
            let color = marks.colors.and_then(|colors| colors.get(&index)).copied().unwrap_or_else(|| rgba(text.color));
            let color = TextColor(color.with_alpha(color.alpha() * group));
            let frame = Node { justify_content: justify, align_items: align, ..at(x, y, w, h) };
            // (Its rim or shadow at the group's alpha as well.)
            let effect = look.text_effect(panel, text.font).map(|effect| effect.faded(group));
            let size = text.size * s;
            // As the game draws it, where its font asset is had: what a
            // script puts before the words (a figure's noughts), then the
            // words.
            // Words with TextMeshPro's colour tags in them (`<color=#...>`,
            // a localized format's figure) as runs of their colours.
            let spans = rich_spans(&said_now);
            if let Some((asset, material)) = look.tmp_of(panel, text) {
                let bold = style & 1 != 0;
                let run = |said: String| trp_data::tmp_text::TmpRun { text: said, size, color: screen_numbers(color.0), bold };
                let layout = trp_data::tmp_text::TmpLayout { em: text.size * s, spacing: text.spacing, kerning: text.kerning, wrap: None, align: text.horizontal & 7 };
                let runs = match spans {
                    Some(spans) => std::iter::once(run(String::new()))
                        .chain(spans.into_iter().map(|(said, tint)| {
                            let tinted = tint.map_or(color.0, |c| Color::srgba(c[0], c[1], c[2], c[3] * group));
                            trp_data::tmp_text::TmpRun { text: said, size, color: screen_numbers(tinted), bold }
                        }))
                        .collect(),
                    None => vec![run(String::new()), run(said_now)],
                };
                let words = TmpText { asset, material, runs, layout };
                column.spawn((piece, frame, visibility)).with_children(|frame| {
                    tmp_words(frame, Node::default(), words, &mut |entity| {
                        entity.insert(PieceText(base + index));
                    });
                });
            } else {
                let runs: Vec<(String, TextFont, Color)> = match spans {
                    Some(spans) => spans.into_iter().map(|(said, tint)| (said, font.clone(), tint.map_or(color.0, |c| Color::srgba(c[0], c[1], c[2], c[3] * group)))).collect(),
                    None => vec![(said_now, font, color.0)],
                };
                column.spawn((piece, frame, visibility)).with_children(|frame| {
                    effect_words(frame, Node::default(), &runs, effect, size, &mut |entity, run, _| {
                        match run {
                            None => entity.insert(PieceText(base + index)),
                            Some(_) => entity.insert(PieceSpan(base + index)),
                        };
                    });
                });
            }
        }
    }
}

/// Words with TextMeshPro's rich text in them as runs, each with the colour
/// its `<color=#RRGGBB>` or `<color=#RRGGBBAA>` gives it (none: the text's
/// own) up to its `</color>`, the tags of the bold, the italic and the size
/// left out; none where the words have no colour tag, which are drawn as
/// they are.
pub(crate) fn rich_spans(words: &str) -> Option<Vec<(String, Option<[f32; 4]>)>> {
    if !words.contains("<color") {
        return None;
    }
    let hex = |digits: &str| -> Option<[f32; 4]> {
        let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok().map(|b| b as f32 / 255.0);
        match digits.len() {
            6 => Some([byte(0)?, byte(2)?, byte(4)?, 1.0]),
            8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
            _ => None,
        }
    };
    let mut spans: Vec<(String, Option<[f32; 4]>)> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut current = String::new();
    let mut rest = words;
    while let Some(open) = rest.find('<') {
        current.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>').map(|at| open + at) else {
            current.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let tag = &rest[open + 1..close];
        // A colour by its figures, or by one of TextMeshPro's names for one
        // (`TMP_Text`'s `<color=white>`: black, blue, green, orange, purple,
        // red, white, yellow).
        let named = |name: &str| -> Option<[f32; 4]> {
            let figures = match name {
                "black" => "000000",
                "blue" => "0000FF",
                "green" => "00FF00",
                "orange" => "FF8000",
                "purple" => "A020F0",
                "red" => "FF0000",
                "white" => "FFFFFF",
                "yellow" => "FFFF00",
                _ => return None,
            };
            hex(figures)
        };
        let colour = tag.strip_prefix("color=").map(|value| value.trim_matches('"')).and_then(|value| value.strip_prefix('#').map_or_else(|| named(value).or_else(|| hex(value)), hex));
        let known = colour.is_some() || matches!(tag, "/color" | "b" | "/b" | "i" | "/i" | "u" | "/u" | "/size") || tag.starts_with("size=");
        if !known {
            current.push_str(&rest[open..=close]);
        } else if colour.is_some() || tag == "/color" {
            if !current.is_empty() {
                spans.push((std::mem::take(&mut current), colours.last().copied()));
            }
            match colour {
                Some(colour) => colours.push(colour),
                None => {
                    colours.pop();
                }
            }
        }
        rest = &rest[close + 1..];
    }
    current.push_str(rest);
    if !current.is_empty() {
        spans.push((current, colours.last().copied()));
    }
    Some(spans)
}

/// The figures of the app's own panel: each one's field on the game's
/// panel, the key of its words in the install's texts and the app's word
/// where the install has none. Blood pressure, which the game never
/// writes (its `120/80` is the scene's own), is left out.
const FIGURES: [(&str, &str, &str); 6] = [
    ("OverallHealth", "Health", "HEALTH"),
    ("Hydration", "HYDRATION", "HYDRATION"),
    ("Energy", "ENERGY", "ENERGY"),
    ("Temperature", "Temperature", "TEMPERATURE"),
    ("Poisoning", "Poisoning", "POISONING"),
    ("Radiation", "Radiation", "RADIATION"),
];

/// The game's view of a part and its figures, in units: a view's bar
/// (its header is above it, its effects under it) and a figure's panel
/// (`probe healthpanel`).
const VIEW: [f32; 2] = [130.0, 19.0];
const HEADER: f32 = 21.0;
const FIGURE: [f32; 2] = [180.0, 46.0];

/// Where the app's own panel puts each part's view (in `Part::ALL`'s
/// order) and each figure (in `FIGURES`' order) where the game's panel
/// could not be read: the game's places on a 16:9 screen (read
/// 2026-10-15, at night, `probe healthpanel`), from the equipment panel's
/// top left corner, in units.
const OWN_PARTS: [[f32; 2]; 7] = [[325.1, 65.5], [196.1, 164.1], [196.1, 243.8], [362.2, 243.8], [29.8, 243.8], [315.1, 405.8], [75.1, 405.8]];
const OWN_FIGURES: [[f32; 2]; 6] = [[161.0, 612.0], [177.5, 729.0], [366.9, 729.0], [-14.6, 681.0], [177.5, 681.0], [366.9, 681.0]];

// ==== The app's own panel, the figures, the HEALTH panel filled ====

/// The app's own panel's parts: a part's figures and its bar's fill, by
/// the part's place in `Part::ALL`; a figure's value and its rate, by its
/// place in `FIGURES`.
#[derive(Component)]
struct OwnFigures(usize);
#[derive(Component)]
struct OwnFill(usize);
#[derive(Component)]
struct OwnValue(usize);
#[derive(Component)]
struct OwnRate(usize);

/// The app's colours for a part's bar: whole, hurt, badly hurt,
/// destroyed (the app's own since 0.10.0).
const GOOD: Color = Color::srgb(0.42, 0.60, 0.30);
const HURT: Color = Color::srgb(0.78, 0.60, 0.22);
const BAD: Color = Color::srgb(0.74, 0.22, 0.18);
const GONE: Color = Color::srgb(0.10, 0.10, 0.10);

fn shade(share: f32, destroyed: bool) -> Color {
    if destroyed {
        GONE
    } else if share > 0.6 {
        GOOD
    } else if share > 0.3 {
        HURT
    } else {
        BAD
    }
}

/// The app's own HEALTH panel (`TARKOV UI` off): a box for each part (its
/// name, its figures, a bar and the effects on it) and the figures, in the
/// app's style, each where the game's panel has it in a left panel of
/// that size, the panel's top left at `origin` (units, in what it is drawn
/// into); over the character's model, which the page draws where the gear
/// tab has it. Where the game's panel could not be read, at its places on
/// a 16:9 screen from the equipment panel's top left, `gear`.
#[allow(clippy::too_many_arguments)]
pub fn draw_own_panel(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, health: Option<&Health>, left: Option<[f32; 2]>, origin: [f32; 2], gear: [f32; 2], s: f32) {
    if health.is_none() {
        column.spawn(words("The options' HEALTH SYSTEM is off: the character cannot be hurt, and is taken as fed and watered.", 13.0, WORDS));
        return;
    }
    let game = look.panel.as_ref().zip(left);
    let place = |index: usize, fallback: [f32; 2]| match game {
        Some((game, left)) => {
            let [x, y, ..] = game.panel.rect(index, left);
            [origin[0] + x, origin[1] + y]
        }
        None => [gear[0] + fallback[0], gear[1] + fallback[1]],
    };
    let at = |[x, y]: [f32; 2], width: f32| Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(width * s), ..default() };
    for (n, part) in Part::ALL.iter().enumerate() {
        let view = game.and_then(|(game, _)| game.views.parts.get(n)).map(|view| view.view);
        let [x, y] = view.map_or([gear[0] + OWN_PARTS[n][0], gear[1] + OWN_PARTS[n][1]], |view| place(view, OWN_PARTS[n]));
        let node = Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), ..at([x, y - HEADER], VIEW[0]) };
        // (A thing let go on the box is used on the part: `HealthDrop`.)
        column.spawn((HealthDrop(n), RelativeCursorPosition::default(), node, BorderColor::all(crate::inventory_view::LINE), BackgroundColor(crate::inventory_view::PANEL.with_alpha(0.85)))).with_children(|cell| {
            cell.spawn(words(said.part(*part), 10.0, FAINT));
            cell.spawn((OwnFigures(n), words("", 12.0, WORDS)));
            let track = Node { width: Val::Percent(100.0), height: Val::Px(5.0), ..default() };
            cell.spawn((track, BackgroundColor(crate::inventory_view::LINE))).with_children(|track| {
                track.spawn((OwnFill(n), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(GOOD)));
            });
            cell.spawn((EffectWords(n), words("", 10.0, FAINT)));
        });
    }
    for (n, (name, key, otherwise)) in FIGURES.iter().enumerate() {
        let figure = game.and_then(|(game, _)| game.views.parameter(name)).map(|figure| figure.panel);
        let [x, y] = figure.map_or([gear[0] + OWN_FIGURES[n][0], gear[1] + OWN_FIGURES[n][1]], |figure| place(figure, OWN_FIGURES[n]));
        // A box as a part's: its name and its rate over its figures.
        let node = Node {
            height: Val::Px(FIGURE[1] * s),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(2.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..at([x, y], FIGURE[0])
        };
        column.spawn((node, BorderColor::all(crate::inventory_view::LINE), BackgroundColor(crate::inventory_view::PANEL.with_alpha(0.85)))).with_children(|cell| {
            cell.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
                row.spawn((words(said.say(key, otherwise).to_uppercase(), 10.0, FAINT), TextLayout::new_with_no_wrap()));
                row.spawn((OwnRate(n), words("", 10.0, FAINT), TextLayout::new_with_no_wrap()));
            });
            cell.spawn((OwnValue(n), words("", 15.0, WORDS), TextLayout::new_with_no_wrap()));
        });
    }
}

/// Keeps the app's own panel, every frame, and each part's effects in
/// words on either panel: the same figures as the game's panel shows.
#[allow(clippy::type_complexity)]
fn fill_own(
    controller: Res<Controller>,
    said: Res<Words>,
    mut texts: Query<(&mut Text, &mut TextColor, Option<&OwnFigures>, Option<&OwnValue>, Option<&OwnRate>, Option<&EffectWords>), Or<(With<OwnFigures>, With<OwnValue>, With<OwnRate>, With<EffectWords>)>>,
    mut fills: Query<(&OwnFill, &mut Node, &mut BackgroundColor)>,
) {
    let Some(health) = controller.health.as_ref() else { return };
    if texts.is_empty() {
        return;
    }
    let rows = figure_rows(health);
    for (mut text, mut color, figures, value, rate, effects) in &mut texts {
        let (now, tint) = if let Some(OwnFigures(n)) = figures {
            let state = health.part(Part::ALL[*n]);
            let shown = if state.maximum / state.current < 2.0 { state.current.floor() } else { state.current.ceil() };
            let words = if state.destroyed { format!("{shown} / {}   DESTROYED", state.maximum) } else { format!("{shown} / {}", state.maximum) };
            (words, WORDS)
        } else if let Some(OwnValue(n)) = value {
            let Some(&(_, [current, maximum], _, _, places, _)) = rows.iter().find(|row| row.0 == FIGURES[*n].0) else { continue };
            let (_, written) = figure(current, maximum, places, -1);
            // Temperature has no maximum written, as on the game's panel.
            (if places > 0 { written } else { format!("{written} / {maximum:.0}") }, WORDS)
        } else if let Some(OwnRate(n)) = rate {
            match rows.iter().find(|row| row.0 == FIGURES[*n].0).and_then(|row| row.5) {
                Some((value, _, rise_good)) if value != 0.0 => (format!("{value:+.2}/min"), if (value > 0.0) == rise_good { GOOD } else { BAD }),
                _ => (String::new(), FAINT),
            }
        } else if let Some(EffectWords(n)) = effects {
            let part = Part::ALL[*n];
            let mut names: Vec<String> = health
                .effects
                .iter()
                .filter(|e| e.part == part && e.existing() && !matches!(e.kind, Kind::Encumbered | Kind::OverEncumbered))
                .filter_map(|e| said.effect(e.kind))
                .collect();
            names.dedup();
            (names.join(", "), color.0)
        } else {
            continue;
        };
        if text.0 != now {
            text.0 = now;
        }
        if color.0 != tint {
            color.0 = tint;
        }
    }
    for (fill, mut node, mut back) in &mut fills {
        let state = health.part(Part::ALL[fill.0]);
        let share = if state.maximum > 0.0 { (state.current / state.maximum).clamp(0.0, 1.0) } else { 0.0 };
        let width = Val::Percent((share * 100.0).round());
        if node.width != width {
            node.width = width;
        }
        let tint = shade(share, state.destroyed);
        if back.0 != tint {
            back.0 = tint;
        }
    }
}

/// What a frame wants of a drawn object: shown or not, its colour, how
/// much of it is filled, its words and what goes before them.
#[derive(Default, Clone)]
struct Want {
    visible: Option<bool>,
    color: Option<Color>,
    fill: Option<f32>,
    before: Option<(String, Color)>,
    words: Option<(String, Color)>,
    /// An arrow: whether its figure moves (`SetBuffValue`'s `active`, its
    /// animator's `Active`), and how far up it is drawn, in units.
    moving: Option<bool>,
    raised: f32,
}

/// `ColorSchemeExtension.GetBodyPartColor` with the green-to-red scheme:
/// the share squared as a hue from red to green, full; black destroyed.
fn part_color(share: f32, destroyed: bool) -> Color {
    if destroyed {
        return Color::BLACK;
    }
    let share = share.clamp(0.0, 1.0);
    let [r, g, b] = hsv(share * share * 120.0, 1.0, 1.0);
    Color::srgb_u8(r, g, b)
}

/// `ColorUtilities.ColorFromHSV`: a hue in degrees, each channel a byte
/// rounded to the nearest (`Convert.ToByte`, a half to even).
pub(crate) fn hsv(hue: f32, saturation: f32, value: f32) -> [u8; 3] {
    let sector = (hue / 60.0).floor() as i32 % 6;
    let f = (hue / 60.0) as f64 - (hue / 60.0).floor() as f64;
    let value = value * 255.0;
    let byte = |x: f64| x.round_ties_even().clamp(0.0, 255.0) as u8;
    let v = byte(value as f64);
    let p = byte((value * (1.0 - saturation)) as f64);
    let q = byte(value as f64 * (1.0 - f * saturation as f64));
    let t = byte(value as f64 * (1.0 - (1.0 - f) * saturation as f64));
    match sector {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A figure as `HealthParameterPanel.SetParameterValue` writes it: whole,
/// down where more than half is left and up where less (`F0`), or to so
/// many places (a half to even); and the noughts that make up the fewest
/// digits it is written with.
fn figure(current: f32, maximum: f32, places: i32, least: i32) -> (String, String) {
    let text = if places <= 0 {
        let whole = if current / maximum > 0.5 { current.floor() } else { current.ceil() };
        format!("{whole:.0}")
    } else {
        let scale = 10f64.powi(places);
        format!("{}", (current as f64 * scale).round_ties_even() / scale)
    };
    let short = (least - text.chars().count() as i32).max(0) as usize;
    ("0".repeat(short), text)
}

/// One of the figures the panel writes (`HealthParametersPanel.
/// UpdateMainHealthAndStats`): its field's name, its current and maximum,
/// its warning and whether it is reached from above, its places, and its
/// rate a minute with whether it shows it moving (short of its maximum)
/// and whether a rise is good.
type FigureRow = (&'static str, [f32; 2], f32, bool, i32, Option<(f32, bool, bool)>);

fn figure_rows(health: &Health) -> [FigureRow; 6] {
    let rates = health.rates();
    let settings = &health.settings;
    let common = health.common();
    [
        ("OverallHealth", [common[0].round_ties_even(), common[1]], 10.0, false, 0, Some((rates[0] * 60.0, common[0] < common[1], true))),
        ("Hydration", [health.hydration[0], health.hydration[2]], 10.0, false, 0, Some((rates[2] * 60.0, health.hydration[0] < health.hydration[2], true))),
        ("Energy", [health.energy[0], health.energy[2]], 10.0, false, 0, Some((rates[1] * 60.0, health.energy[0] < health.energy[2], true))),
        ("Temperature", [settings.temperature[2], settings.temperature[1]], 10.0, false, 1, Some((0.0, settings.temperature[2] < settings.temperature[1], false))),
        ("Poisoning", [settings.poisoning[2], settings.poisoning[1]], 100.0, true, 0, None),
        ("Radiation", [0.0, 100.0], -1.0, false, 0, None),
    ]
}

/// The rate's colours (`BuffableHealthParameterPanel`).
const RISING: Color = Color::srgb(0.349, 0.667, 0.835);
const FALLING: Color = Color::srgb(0.729, 0.0, 0.0);

/// Keeps the panel as the game's scripts keep it: each part's view on
/// every frame (`BodyPartView.Update`), the figures once a second and as
/// they are first drawn (`HealthParametersPanel.Update`, on the clock of
/// the world outside).
#[allow(clippy::type_complexity)]
fn fill_panel(
    controller: Res<Controller>,
    look: Res<HealthLook>,
    time: Res<Time<Real>>,
    carried: Res<crate::inventory_view::Carried>,
    added: Query<(), Added<Piece>>,
    mut pieces: Query<(&Piece, &mut Visibility, Option<&mut ImageNode>, Option<&mut Node>, Option<&FullWidth>, Option<&Bob>)>,
    mut fills: Query<(&PieceFill, &mut ImageNode), Without<Piece>>,
    mut slices: Query<(&SliceOf, &mut ImageNode), (Without<Piece>, Without<PieceFill>)>,
    mut texts: Query<(&PieceText, &mut Text, &mut TextColor, Has<EffectCopy>), (Without<PieceSpan>, Without<EffectWords>)>,
    mut spans: Query<(&PieceSpan, &mut TextSpan, &mut TextColor, Has<EffectCopy>), Without<PieceText>>,
    mut figures: Local<(f64, HashMap<usize, Want>)>,
    mut drawn_texts: Query<(&PieceText, &mut TmpText)>,
) {
    let (Some(game), Some(health)) = (&look.panel, controller.health.as_ref()) else { return };
    if pieces.is_empty() {
        return;
    }
    let (panel, views) = (&game.panel, &game.views);
    let mut wants: HashMap<usize, Want> = HashMap::new();
    let mut want = |index: Option<usize>, change: &dyn Fn(&mut Want)| {
        if let Some(index) = index {
            change(wants.entry(index).or_default());
        }
    };
    // The parts' views.
    for (n, view) in views.parts.iter().enumerate() {
        let state = health.part(Part::ALL[n]);
        let (current, maximum, destroyed) = (state.current, state.maximum, state.destroyed);
        let share = if maximum > 0.0 { current / maximum } else { 0.0 };
        let shown = if maximum / current < 2.0 { current.floor() } else { current.ceil() };
        let value = (format!("{shown}/{maximum}"), if share > 0.15 { Color::WHITE } else { Color::srgb(1.0, 0.0, 0.0) });
        want(view.value, &|w| w.words = Some(value.clone()));
        let shape = if destroyed {
            Color::BLACK
        } else if share > 0.8 {
            Color::srgba(1.0, 0.0, 0.0, 0.0)
        } else {
            Color::srgba(1.0, 0.0, 0.0, 1.0 - share / 0.8)
        };
        want(view.shape, &|w| w.color = Some(shape));
        want(view.bar, &|w| {
            w.fill = Some(share.clamp(0.0, 1.0));
            w.color = Some(part_color(share / 0.8, destroyed));
        });
        let border = view.border.and_then(|border| panel.nodes[border].image.map(|image| rgba(image.color)));
        want(view.border, &|w| w.color = Some(if destroyed { Color::srgb(1.0, 0.0, 0.0) } else { border.unwrap_or(Color::WHITE) }));
        want(view.broken, &|w| w.visible = Some(destroyed));
    }
    // The figures, once a second, or as they are drawn anew: the health
    // panel's, and the GEAR tab's strip's with the carried weight
    // (`_weight`, written as its weight changes, to a place, against the
    // upper limit, and coloured by the two limits: `SetWarningColor`).
    let now = time.elapsed_secs_f64();
    if !added.is_empty() || now - figures.0 >= 1.0 {
        figures.0 = now;
        figures.1.clear();
        // At elite Strength the elite sum, over the upper limit times the
        // skills' carrying factor (`HealthParametersPanel`).
        let weight = carried.equipment.carried_weight(carried.strength_elite);
        let mut rows: Vec<FigureRow> = figure_rows(health).to_vec();
        if let Some([_, upper]) = look.weight_limits {
            rows.push(("Weight", [weight, upper * carried.carry], 0.0, false, 1, None));
        }
        let mut panels = vec![(panel, views.parameters.as_slice(), 0)];
        if let Some(strip) = &game.strip {
            panels.push((strip, game.strip_figures.as_slice(), STRIP));
        }
        for ((name, [current, maximum], threshold, from_top, places, rate), (panel, list, base)) in rows.iter().flat_map(|row| panels.iter().map(move |at| (*row, *at))) {
            let Some(f) = trp_data::screen::figure_named(list, name) else { continue };
            let scene_color = |text: Option<usize>| text.and_then(|text| panel.nodes[text].text.as_ref()).map_or(Color::WHITE, |text| rgba(text.color));
            let (default, max_color) = (scene_color(f.current), scene_color(f.max));
            let (noughts, text) = figure(current, maximum, places, f.least_digits);
            let mut color = default;
            let mut set = |index: Option<usize>, change: &dyn Fn(&mut Want)| {
                if let Some(index) = index {
                    change(figures.1.entry(base + index).or_default());
                }
            };
            if threshold >= 0.0 {
                let warned = if from_top { current as i32 as f32 >= threshold } else { current as i32 as f32 <= threshold };
                if warned {
                    color = rgba(f.warning);
                }
                set(f.icon, &|w| w.color = Some(color));
                set(f.glow, &|w| w.visible = Some(warned));
                set(f.ok_glow, &|w| w.visible = Some(!warned));
            } else {
                set(f.ok_glow, &|w| w.visible = Some(true));
            }
            // The weight's colour, after: the default under the lower
            // limit, the other colour under the upper, the warning over it.
            if let (true, Some([lower, upper])) = (name == "Weight", look.weight_limits.map(|limits| limits.map(|limit| limit * carried.carry))) {
                color = if current <= lower {
                    default
                } else if current < upper {
                    rgba(f.alternative)
                } else {
                    rgba(f.warning)
                };
                set(f.icon, &|w| w.color = Some(color));
            }
            set(f.current, &|w| {
                w.before = Some((noughts.clone(), rgba(f.noughts)));
                w.words = Some((text.clone(), color));
            });
            set(f.max, &|w| w.words = Some((format!("/{maximum:.0}"), max_color)));
            if let Some((value, moving, rise_good)) = rate {
                let good = if rise_good { value >= f32::EPSILON } else { value <= -1e-45 };
                let (up, down) = if rise_good { (RISING, FALLING) } else { (FALLING, RISING) };
                set(f.rate, &|w| {
                    w.visible = Some(value != 0.0);
                    w.words = Some((format!("{value:.2}"), if good { RISING } else { FALLING }));
                });
                set(f.up, &|w| {
                    w.visible = Some(value > 0.0);
                    w.color = Some(up);
                    w.moving = Some(moving);
                });
                set(f.down, &|w| {
                    w.visible = Some(value < 0.0);
                    w.color = Some(down);
                    w.moving = Some(moving);
                });
            }
        }
    }
    for (index, figure) in &figures.1 {
        wants.insert(*index, figure.clone());
    }
    // The arrows of a moving figure bob as their clip has them, on the
    // world's clock; a still one's stand whole where they are laid out.
    let bob = look.bob.as_ref().map(|(clip, height, alpha)| {
        let mut values = Vec::new();
        clip.sample((now % clip.duration().max(1e-3) as f64) as f32, &mut values);
        (values.get(*height).copied().unwrap_or(0.0), values.get(*alpha).copied().unwrap_or(1.0))
    });
    for w in wants.values_mut() {
        if let (Some(true), Some((height, alpha))) = (w.moving, bob) {
            w.raised = height;
            w.color = w.color.map(|color| color.with_alpha(alpha));
        }
    }
    for (piece, mut visibility, image, node, full, bob) in &mut pieces {
        let Some(w) = wants.get(&piece.0) else { continue };
        let mut node = node;
        if let (Some(bob), Some(node)) = (bob, node.as_mut()) {
            let top = Val::Px(bob.0 - w.raised * bob.1);
            if node.top != top {
                node.top = top;
            }
        }
        if let Some(on) = w.visible {
            let now = if on { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != now {
                *visibility = now;
            }
        }
        if let (Some(color), Some(mut image)) = (w.color, image) {
            if image.color != color {
                image.color = color;
            }
        }
        if let (Some(fill), Some(mut node), Some(full)) = (w.fill, node, full) {
            let width = Val::Px(full.0 * fill);
            if node.width != width {
                node.width = width;
            }
        }
    }
    for (piece, mut image) in &mut fills {
        if let Some(color) = wants.get(&piece.0).and_then(|w| w.color) {
            if image.color != color {
                image.color = color;
            }
        }
    }
    for (piece, mut image) in &mut slices {
        if let Some(color) = wants.get(&piece.0).and_then(|w| w.color) {
            if image.color != color {
                image.color = color;
            }
        }
    }
    // A text drawn as the game's: what goes before its words, and its
    // words, each in its colour.
    for (piece, mut drawn) in &mut drawn_texts {
        let Some(w) = wants.get(&piece.0) else { continue };
        let mut runs = drawn.runs.clone();
        if let (Some((before, tint)), Some(first)) = (&w.before, runs.first_mut()) {
            (first.text, first.color) = (before.clone(), screen_numbers(*tint));
        }
        if let (Some((said, tint)), Some(last)) = (&w.words, runs.last_mut()) {
            (last.text, last.color) = (said.clone(), screen_numbers(*tint));
        }
        if runs != drawn.runs {
            drawn.runs = runs;
        }
    }
    // (A copy drawn as its font's rim or shadow takes the words, and keeps
    // its own colour.)
    for (piece, mut text, mut color, copy) in &mut texts {
        if let Some((before, tint)) = wants.get(&piece.0).and_then(|w| w.before.as_ref()) {
            if text.0 != *before {
                text.0 = before.clone();
            }
            if !copy && color.0 != *tint {
                color.0 = *tint;
            }
        }
    }
    for (piece, mut span, mut color, copy) in &mut spans {
        if let Some((words, tint)) = wants.get(&piece.0).and_then(|w| w.words.as_ref()) {
            if span.0 != *words {
                span.0 = words.clone();
            }
            if !copy && color.0 != *tint {
                color.0 = *tint;
            }
        }
    }
}

// ==== Damage keys, the death screen, the barbed wire ====

/// The app's commands for trying the health out (the user, 2026-10-16:
/// "Can we add a hurt/death trigger somewhere? a bind or an area"; not the
/// game's), the debug panel's since 0.141.0: HURT THE NEXT PART does a
/// bullet's damage (`HURT_DAMAGE`, with the game's own chances of a
/// bleeding and a fracture) to the next part in turn, the limbs first
/// (`HURT_ORDER`), and DIE kills as a bullet through the head does. The
/// line under the inventory's slots says what was hurt.
fn damage_keys(
    options: Res<crate::Options>,
    mut controller: ResMut<Controller>,
    mut carried: ResMut<crate::inventory_view::Carried>,
    mut next: Local<usize>,
    mut received: MessageWriter<crate::contusion_view::Received>,
    mut asked: MessageReader<crate::debug_menu::DebugCommand>,
) {
    use crate::debug_menu::DebugCommand;
    // The debug panel's two buttons (`debug_menu`; their keys `F7` and `F8`
    // went, the user, 2026-10-26: "just leave them as commands in the new
    // debug menu").
    let asked: Vec<DebugCommand> = asked.read().copied().collect();
    if !options.keys_live() {
        return;
    }
    let hurt = asked.contains(&DebugCommand::HurtNext);
    let kill = asked.contains(&DebugCommand::Die);
    if !hurt && !kill {
        return;
    }
    let Some(health) = controller.health.as_mut().filter(|health| health.alive) else { return };
    if kill {
        let left = health.part(Part::Head).current;
        health.apply_damage(Part::Head, left + 1.0, DamageKind::Bullet);
        // A shot: the player told of it (`ApplyShot` → `ReceiveDamage`).
        received.write(crate::contusion_view::Received { part: Part::Head, damage: left + 1.0, absorbed: 0.0, kind: DamageKind::Bullet, burn: 0.0 });
        carried.note = "Killed (a bullet through the head)".to_owned();
        return;
    }
    let part = HURT_ORDER[*next % HURT_ORDER.len()];
    *next += 1;
    let done = health.apply_damage(part, HURT_DAMAGE, DamageKind::Bullet);
    received.write(crate::contusion_view::Received { part, damage: HURT_DAMAGE, absorbed: 0.0, kind: DamageKind::Bullet, burn: 0.0 });
    carried.note = format!("{} hurt by {done:.0}", part.name());
}

/// What HURT THE NEXT PART does to a part, and the parts it goes to in turn.
const HURT_DAMAGE: f32 = 15.0;
const HURT_ORDER: [Part; 7] = [Part::LeftArm, Part::RightArm, Part::LeftLeg, Part::RightLeg, Part::Stomach, Part::Chest, Part::Head];

/// The damage a run nobody is at the keys of was told to do (`--damage`),
/// on its drawn frame, counted as the taps are: a bullet's, so that the
/// game's own chances of a bleeding or a fracture go with it.
fn damage_asked(options: Res<crate::Options>, mut controller: ResMut<Controller>, mut frame: Local<u32>, mut received: MessageWriter<crate::contusion_view::Received>) {
    *frame += 1;
    if !options.unattended() {
        return;
    }
    let Some(health) = controller.health.as_mut() else { return };
    for (_, part, amount, burn) in options.damage.iter().filter(|(at, ..)| *at == *frame) {
        // Nothing reaches the dead (`ApplyShot`).
        if health.alive {
            health.apply_damage(*part, *amount, DamageKind::Bullet);
            received.write(crate::contusion_view::Received { part: *part, damage: *amount, absorbed: 0.0, kind: DamageKind::Bullet, burn: burn * amount });
        }
    }
}

/// What the armour and the backpack worn take off the speed
/// (`Player.UpdatePenalties`, `ESpeedLimit.Armor`), worked out again
/// whenever what is carried or the skills change: the vest skills' shares
/// only while `SKILLS` is on (movement spec, "Speed limits").
fn armour_slows(
    mut controller: ResMut<Controller>,
    carried: Res<crate::inventory_view::Carried>,
    game: Res<crate::GameData>,
    skills: Option<Res<crate::skills::Skills>>,
    mut last: Local<Option<(Vec<trp_sim::armor::Worn>, trp_sim::armor::VestSkills)>>,
) {
    let skills_changed = skills.as_ref().is_some_and(|s| s.is_changed());
    if !carried.is_changed() && !skills_changed {
        return;
    }
    let vests = skills.as_deref().filter(|s| s.on()).map_or_else(trp_sim::armor::VestSkills::default, |s| trp_sim::armor::VestSkills::of(&s.levels));
    let worn = crate::dummy::worn_of(&carried.equipment.item);
    if last.as_ref().is_some_and(|(w, v)| *w == worn && *v == vests) {
        return;
    }
    let penalty = trp_sim::armor::speed_penalty(&game.db, &worn, &vests);
    if last.is_some() || penalty != 0.0 {
        info!("armour and backpack: {penalty:+.2}% speed");
    }
    *last = Some((worn, vests));
    controller.movement.set_armour_penalty(penalty);
}

/// Each hit given to the player, as the movement takes it
/// (`Player.ApplyHitDebuff`): a weapon's caps the speed for a while by the
/// stamina it burns, and a leg's ends a sprint (`Movement::hit`; movement
/// spec, "Speed limits"). Nothing reaches the dead.
fn hits_taken(mut controller: ResMut<Controller>, mut received: MessageReader<crate::contusion_view::Received>) {
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    for hit in received.read() {
        if alive {
            controller.movement.hit(hit.burn, hit.part, hit.kind);
        }
    }
}

/// The death screen: over everything, shown once the character is dead
/// and the game's black is done (`death::SCREEN_AT`).
#[derive(Component)]
struct DeathScreen;
#[derive(Component)]
struct DeathCause;

fn spawn_death_screen(mut commands: Commands) {
    let whole = Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(14.0),
        ..default()
    };
    commands.spawn((DeathScreen, whole, BackgroundColor(Color::srgba(0.02, 0.0, 0.0, 0.94)), GlobalZIndex(40), Visibility::Hidden)).with_children(|screen| {
        screen.spawn((DeathCause, words("", 30.0, Color::srgb(0.82, 0.24, 0.20))));
        screen.spawn(words("ENTER: the run again, where it began", 15.0, FAINT));
    });
}

fn show_death(
    controller: Res<Controller>,
    dying: Res<crate::death::Dying>,
    said: Res<Words>,
    mut screens: Query<&mut Visibility, With<DeathScreen>>,
    mut causes: Query<&mut Text, With<DeathCause>>,
) {
    let shut = dying.since().is_some_and(|since| since >= crate::death::SCREEN_AT);
    let dead = controller.health.as_ref().filter(|health| !health.alive && shut);
    let wanted = if dead.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    for mut shown in &mut screens {
        if *shown != wanted {
            *shown = wanted;
        }
    }
    if let Some(health) = dead {
        let title = said.say("Killed in Action", "Killed in Action").to_uppercase();
        let cause = said.cause(health.cause.unwrap_or(DamageKind::Undefined));
        let now = format!("{title}\n{}{cause}", said.say("Killed by", "Killed by "));
        for mut text in &mut causes {
            if text.0 != now {
                text.0 = now.clone();
            }
        }
    }
}


/// The body's hit colliders that barbed wire is told of
/// (`BodyPartCollider`), each where its bone is, or halfway to the next
/// bone of its limb, and the part it is of. `PROVISIONAL`: the game's own
/// colliders (twenty-six, with their sizes, in the player's prefab) are not
/// read; these are the largest ones, at their bones.
const COLLIDERS: [(&str, Option<&str>, Part); 12] = [
    ("Base HumanHead", None, Part::Head),
    ("Base HumanSpine3", None, Part::Chest),
    ("Base HumanSpine2", None, Part::Chest),
    ("Base HumanPelvis", None, Part::Stomach),
    ("Base HumanLUpperarm", Some("Base HumanLForearm1"), Part::LeftArm),
    ("Base HumanLForearm1", Some("Base HumanLPalm"), Part::LeftArm),
    ("Base HumanRUpperarm", Some("Base HumanRForearm1"), Part::RightArm),
    ("Base HumanRForearm1", Some("Base HumanRPalm"), Part::RightArm),
    ("Base HumanLThigh1", Some("Base HumanLCalf"), Part::LeftLeg),
    ("Base HumanLCalf", Some("Base HumanLFoot"), Part::LeftLeg),
    ("Base HumanRThigh1", Some("Base HumanRCalf"), Part::RightLeg),
    ("Base HumanRCalf", Some("Base HumanRFoot"), Part::RightLeg),
];

/// Barbed wire on the range (`BarbedWire`, a `DamageTrigger`; health spec,
/// "Barbed wire"): while the character's capsule is in its box the speed
/// is held down and a sprint ended, and each collider in the box is cut by
/// how far it has moved.
fn barbed_wire(
    mut controller: ResMut<Controller>,
    rig: Option<Res<BodyRig>>,
    parts: Option<Res<BodyParts>>,
    bones: Query<&GlobalTransform>,
    range: Res<Range>,
    mut tallies: Local<Vec<BarbTally>>,
    mut received: MessageWriter<crate::contusion_view::Received>,
) {
    let controller = &mut *controller;
    let Some(health) = controller.health.as_mut().filter(|health| health.alive) else { return };
    let [low, high] = crate::range::barbed_wire();
    let feet = controller.movement.position;
    let feet = Vec3::new(feet[0], feet[1], -feet[2]);
    // The capsule's footprint over the box, and the box's height over the
    // feet.
    let nearest = Vec2::new(feet.x.clamp(low.x, high.x), feet.z.clamp(low.z, high.z));
    let inside = nearest.distance(Vec2::new(feet.x, feet.z)) <= range.radius && feet.y < high.y && feet.y + controller.movement.capsule_height() > low.y;
    let (Some(rig), Some(parts)) = (rig, parts) else {
        // No body to find the colliders on: the capsule stands for them.
        controller.movement.set_in_barbed_wire(inside, inside);
        return;
    };
    if !inside {
        controller.movement.set_in_barbed_wire(false, false);
        return;
    }
    if tallies.len() != COLLIDERS.len() {
        *tallies = vec![BarbTally::default(); COLLIDERS.len()];
    }
    let at = |end: &str| rig.bone(&parts, end).and_then(|bone| bones.get(bone).ok()).map(GlobalTransform::translation);
    let mut cut = false;
    for ((bone, toward, part), tally) in COLLIDERS.into_iter().zip(tallies.iter_mut()) {
        let Some(from) = at(bone) else { continue };
        let middle = toward.and_then(at).map_or(from, |to| (from + to) / 2.0);
        let within = middle.cmpge(low).all() && middle.cmple(high).all();
        if !within {
            continue;
        }
        cut = true;
        if let Some(damage) = tally.cut(middle.to_array()) {
            // `ApplyEnvironmentalDamage` → `ApplyDamageInfo`: health's, and
            // the player told of it, while alive.
            if health.alive {
                health.apply_damage(part, damage, DamageKind::Barbed);
                received.write(crate::contusion_view::Received { part, damage, absorbed: 0.0, kind: DamageKind::Barbed, burn: 0.0 });
            }
        }
    }
    controller.movement.set_in_barbed_wire(true, cut);
}

// ==== Tests ====

#[cfg(test)]
mod tests {
    use super::*;

    /// A tab's fill cut to its mask (`MaskedFill`): the fill drawn where
    /// the mask is, nothing where it is clear, between texels as the
    /// graphics card filters them, and the fill tiled at its own size.
    #[test]
    fn a_tab_s_fill_is_cut_to_its_mask() {
        let picture = |w: f32, h: f32| Picture { texture: 0, cut: Rect::new(0.0, 0.0, w, h), covers: [0.0, 0.0, 1.0, 1.0], border: [0.0; 4], size: [w, h] };
        // A mask 4 by 4 whose left half is solid; a fill 2 by 1, red then
        // white.
        let mask: Vec<u8> = (0..16).flat_map(|at| [255, 255, 255, if at % 4 < 2 { 255 } else { 0 }]).collect();
        let fill = vec![255, 0, 0, 255, 255, 255, 255, 255];
        let masked = MaskedFill {
            mask: (std::sync::Arc::new(Pixels { rgba: mask, width: 4, height: 4, point: false }), picture(4.0, 4.0), 1.0),
            fill: (std::sync::Arc::new(Pixels { rgba: fill, width: 2, height: 1, point: true }), picture(2.0, 1.0), 1.0),
            color: [1.0; 4],
        };
        let out = masked.compose([4.0, 4.0], [4, 4]);
        let at = |column: usize, row: usize| &out[(row * 4 + column) * 4..(row * 4 + column) * 4 + 4];
        // Drawn on the left, the fill's red then white repeated; clear on
        // the right.
        assert_eq!(at(0, 0), [255, 0, 0, 255]);
        assert_eq!(at(1, 0), [255, 255, 255, 255]);
        assert_eq!(at(0, 3), [255, 0, 0, 255]);
        assert_eq!(at(2, 1), [0, 0, 0, 0]);
        assert_eq!(at(3, 2), [0, 0, 0, 0]);
        // Filtered between two texels, in linear light.
        let half = sample(&masked.mask.0, 2.0, 1.5);
        assert!((half[3] - 0.5).abs() < 1e-6, "{half:?}");
    }

    #[test]
    fn figures_are_written_as_the_panel_writes_them() {
        // Whole, down with more than half left, up with less; noughts to
        // three digits.
        assert_eq!(figure(75.6, 100.0, 0, 3), ("0".to_owned(), "75".to_owned()));
        assert_eq!(figure(0.4, 100.0, 0, 3), ("00".to_owned(), "1".to_owned()));
        assert_eq!(figure(363.0, 440.0, 0, 3), (String::new(), "363".to_owned()));
        // To a place, with no noughts asked for.
        assert_eq!(figure(36.6, 40.0, 1, -1), (String::new(), "36.6".to_owned()));
    }

    #[test]
    fn the_bars_go_from_green_to_red_by_the_square_of_the_share() {
        assert_eq!(hsv(120.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(hsv(60.0, 1.0, 1.0), [255, 255, 0]);
        // Half the share left: a quarter of the way from red to green.
        assert_eq!(hsv(0.25 * 120.0, 1.0, 1.0), [255, 128, 0]);
        assert_eq!(part_color(0.5, true), Color::BLACK);
    }
}

#[cfg(test)]
mod item_tests {
    use super::*;

    #[test]
    fn things_are_coloured_as_the_game_colours_them() {
        assert_eq!(item_ground("blue"), Color::srgba_u8(28, 65, 86, 77));
        assert_eq!(item_ground("violet"), Color::srgba_u8(76, 42, 85, 77));
        assert_eq!(item_ground("unknown"), Color::srgba_u8(127, 127, 127, 77));
        assert_eq!(stack_color(&["Ammo".to_owned()]), Color::srgb_u8(0xda, 0xda, 0xbc));
        assert_eq!(stack_color(&["Money".to_owned()]), Color::srgb_u8(0x8d, 0xb5, 0x00));
        assert_eq!(stack_color(&["Food".to_owned()]), Color::srgb_u8(0xb6, 0xc1, 0xc7));
    }
}

/// Where the OVERALL tab's objects are numbered among the drawn pieces, and
/// its lists' lines, each line's past the one before it.
const OVERALL: usize = 17 << 20;
const OVERALL_LINES: usize = 18 << 20;

// ==== The OVERALL tab: its texts and drawing ====

/// What the OVERALL tab shows as it stands (`overall_view`): each
/// dropdown's words and whether its box is lit (the mouse on it or on its
/// list), the list open, the model's picture, and whether a suit is being
/// read (the model's loading spinner, `PlayerModelView.Show`).
pub struct OverallShows<'a> {
    pub names: [&'a str; 2],
    pub lit: [bool; 2],
    pub open: Option<OpenList<'a>>,
    pub figure: Option<Handle<Image>>,
    pub loading: bool,
}

/// A dropdown's list open: which dropdown's, its lines' words, how far it
/// is scrolled (units from its top) and its bar's handle: its place (left,
/// top, width, height in the screen's units) and its colour as tinted.
pub struct OpenList<'a> {
    pub role: usize,
    pub lines: &'a [&'a str],
    pub scroll: f32,
    pub handle: Option<([f32; 4], Color)>,
}

/// A text a script writes by `string.Format`, its values in its holes
/// (`{0}`, and `{1:00}` with its noughts).
fn format_values(template: &str, values: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}').map(|at| open + at) else { break };
        let hole = &rest[open + 1..close];
        let (index, format) = hole.split_once(':').unwrap_or((hole, ""));
        match index.parse::<usize>().ok().and_then(|index| values.get(index)) {
            Some(value) => {
                let noughts = format.len();
                if noughts > 0 && format.chars().all(|c| c == '0') && value.len() < noughts {
                    out.push_str(&"0".repeat(noughts - value.len()));
                }
                out.push_str(value);
            }
            None => out.push_str(&rest[open..=close]),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The words the OVERALL tab's scripts write on it for a character with no
/// figures of its own (no profile is kept): the install's words for its
/// localized texts in their case; the short statistics, the dashboard's
/// and the online time as the scripts write them with every counter at
/// none (`ShortStatsPanel.SetStats`, `OverallStatsPanel.FillStats`: a
/// rate with no raids `-`, the account's lifetime `-` as for another's
/// statistics, the registration not being had); the level and the
/// experience as at none (`PlayerLevelPanel.Set`); the nickname empty; each
/// dropdown its suit's name.
fn overall_texts(overall: &trp_data::screen::OverallPanel, said: &Words, names: [&str; 2]) -> HashMap<usize, String> {
    let panel = &overall.panel;
    let mut texts: HashMap<usize, String> = overall.localized.iter().map(|(index, key, case)| (*index, trp_data::screen::in_case(said.say(key, key), *case))).collect();
    // The objects a script's fields end with, in their order.
    let pointed = |path: &str, class: &str, count: usize| -> Vec<Option<usize>> {
        let Some(fields) = panel.find(path).and_then(|object| panel.script(object, class)) else { return vec![None; count] };
        let start = fields.len().saturating_sub(count * 12);
        (0..count).map(|n| panel.pointer_at(fields, start + n * 12)).collect()
    };
    fn put(texts: &mut HashMap<usize, String>, at: Option<usize>, words: String) {
        if let Some(at) = at {
            texts.insert(at, words);
        }
    }
    let short = pointed("LeftSide/CharacterPanel/Level Panel/Stats", "ShortStatsPanel", 6);
    for at in short.into_iter().flatten() {
        let template = texts.get(&at).cloned().unwrap_or_default();
        put(&mut texts, Some(at), format_values(&template, &["0"]));
    }
    // `_raidsLabel` to `_kdRatioLabel`, then the online time's and the
    // lifetime's localized texts.
    let dashboard = pointed("RightSide/Dashboard", "OverallStatsPanel", 14);
    let figures = ["0", "0", "0", "-", "0", "-", "0", "0", "0", "-", "0", "-"];
    for (at, figure) in dashboard.iter().zip(figures) {
        put(&mut texts, *at, figure.to_owned());
    }
    if let Some(at) = dashboard[12] {
        let template = texts.get(&at).cloned().unwrap_or_default();
        put(&mut texts, Some(at), format_values(&template, &["0", "0"]));
    }
    put(&mut texts, dashboard[13], said.say("-", "-"));
    let level = pointed("LeftSide/CharacterPanel/Level Panel", "PlayerLevelPanel", 2);
    put(&mut texts, level[0], OVERALL_LEVEL.to_owned());
    // The character panel's: its nickname's and its experience's labels.
    let window = pointed("LeftSide", "InventoryPlayerModelWithStatsWindow", 15);
    put(&mut texts, window[0], String::new());
    put(&mut texts, window[2], "0".to_owned());
    for (selector, name) in OVERALL_SELECTORS.iter().zip(names) {
        put(&mut texts, panel.find(&format!("{selector}/Text")), name.to_owned());
    }
    texts
}

/// Draws the game's OVERALL tab over the whole screen (`TARKOV UI`; screen
/// units, `s` pixels to one): its objects as `draw_nodes` draws them, the
/// model's picture in its view (drawn after what lies under the view and
/// before what lies over it), and the list open over all of it, as the
/// game puts it on its menus' layer (`DropDownBox.SetPanelState`): its
/// panel, its lines from the template in its scroll area's mask, and its
/// bar's handle.
pub fn draw_overall(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, screen: [f32; 2], shows: &OverallShows, s: f32) {
    let Some(overall) = &look.overall else {
        column.spawn(words("The game's OVERALL tab could not be read from the install.", 12.0, FAINT));
        return;
    };
    let panel = &overall.panel;
    let find = |path: &str| panel.find(path);
    let texts = overall_texts(overall, said, shows.names);
    let mut set: HashMap<usize, bool> = HashMap::new();
    let mut colors: HashMap<usize, Color> = HashMap::new();
    for (role, selector) in OVERALL_SELECTORS.iter().enumerate() {
        let open = shows.open.as_ref().is_some_and(|list| list.role == role);
        for (end, on) in [("Open Panel", open), ("Button/Closed Image", !open), ("Button/Open Image", open)] {
            if let Some(at) = find(&format!("{selector}/{end}")) {
                set.insert(at, on);
            }
        }
        // The box's ground under the mouse: its colour faded across by
        // the highlight (`Graphic.CrossFadeColor`, alpha with it), the
        // engine's multiply of the two.
        if let (true, Some(ground), Some(lit)) = (shows.lit[role], find(&format!("{selector}/Background")), overall.dropdown_highlight) {
            if let Some(image) = panel.nodes[ground].image {
                let c = image.color;
                colors.insert(ground, Color::srgba(c[0] * lit[0], c[1] * lit[1], c[2] * lit[2], c[3] * lit[3]));
            }
        }
    }
    if let Some(spinner) = find(&format!("{OVERALL_VIEW}/Progress Spinner")) {
        set.insert(spinner, shows.loading);
    }
    let groups: HashMap<usize, f32> = OVERALL_DIMMED.iter().filter_map(|path| find(path)).map(|at| (at, OVERALL_DIM)).collect();
    let mut sprites: HashMap<usize, Picture> = HashMap::new();
    if let (Some(icon), Some(rank)) = (find("LeftSide/CharacterPanel/Level Panel/Level Icon"), look.overall_pictures.get("rank")) {
        sprites.insert(icon, *rank);
    }
    // The side's and the prestige's at their own sizes (`SetNativeSize`).
    let mut pictures: HashMap<usize, Picture> = HashMap::new();
    for (path, name) in [(format!("{OVERALL_VIEW}/IconsContainer/SideImageHolder/SideImage"), "side"), (format!("{OVERALL_VIEW}/IconsContainer/PrestigeImageHolder/PrestigeImage"), "prestige")] {
        if let (Some(at), Some(picture)) = (find(&path), look.overall_pictures.get(name)) {
            pictures.insert(at, *picture);
        }
    }
    let view = find(OVERALL_VIEW);
    let clothing = find(OVERALL_CLOTHING);
    let right = find("RightSide");
    let open_panels: Vec<usize> = OVERALL_SELECTORS.iter().filter_map(|selector| find(&format!("{selector}/Open Panel"))).collect();
    // Under the view.
    let under: Vec<usize> = [view, clothing, right].into_iter().flatten().collect();
    let base = Marks { set: Some(&set), words: Some(&texts), colors: Some(&colors), groups: Some(&groups), sprites: Some(&sprites), pictures: Some(&pictures), ..Marks::default() };
    draw_nodes(column, look, panel, &look.overall_drawn, OVERALL, Marks { skip: &under, ..base }, said, Frame::In(screen), [0.0, 0.0], s);
    // The model, drawn into its view (`CameraImage`'s picture on its
    // `RawImage`, white).
    if let (Some(view), Some(figure)) = (view, shows.figure.clone()) {
        let [x, y, w, h] = panel.placed_in(view, screen).rect;
        let at = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        column.spawn((Piece(OVERALL + panel.nodes.len()), at, EXACT, ImageNode::new(figure)));
    }
    // Over it, and the rest; the lists apart.
    let over: Vec<usize> = [view, clothing, right].into_iter().flatten().collect();
    let base = Marks { set: Some(&set), words: Some(&texts), colors: Some(&colors), groups: Some(&groups), sprites: Some(&sprites), pictures: Some(&pictures), ..Marks::default() };
    draw_nodes(column, look, panel, &look.overall_drawn, OVERALL, Marks { within: &over, skip: &open_panels, ..base }, said, Frame::In(screen), [0.0, 0.0], s);
    let Some(list) = &shows.open else { return };
    let selector = OVERALL_SELECTORS[list.role];
    let (Some(open), Some(area)) = (find(&format!("{selector}/Open Panel")), find(&format!("{selector}/Open Panel/Scroll Area"))) else { return };
    let handle = find(&format!("{selector}/Open Panel/Scrollbar/Sliding Area/Handle"));
    let skip: Vec<usize> = handle.into_iter().collect();
    let base = Marks { set: Some(&set), words: Some(&texts), ..Marks::default() };
    draw_nodes(column, look, panel, &look.overall_drawn, OVERALL, Marks { within: &[open], skip: &skip, ..base }, said, Frame::In(screen), [0.0, 0.0], s);
    // The lines in the scroll area's mask: laid out in its content's column
    // (`VerticalLayoutGroup`: its padding and spacing, each line as wide as
    // the column leaves and as high as it asks), from the scroll's place.
    let [ax, ay, aw, ah] = panel.placed_in(area, screen).rect;
    let clip = Node { position_type: PositionType::Absolute, left: Val::Px(ax * s), top: Val::Px(ay * s), width: Val::Px(aw * s), height: Val::Px(ah * s), overflow: Overflow::clip(), ..default() };
    let content = find(&format!("{selector}/Open Panel/Scroll Area/Content"));
    let row = content.and_then(|content| panel.nodes[content].group).unwrap_or_default();
    let [cx, cy, cw, _] = content.map_or([ax, ay, aw, ah], |content| panel.placed_in(content, screen).rect);
    let line_height = overall.line.as_ref().map_or(28.0, |line| line.asks(0, 1).0.max(1.0));
    let width = cw - row.padding[0] - row.padding[1];
    if let Some(line) = &overall.line {
        let text = (0..line.nodes.len()).find(|&index| line.nodes[index].text.is_some());
        column.spawn((clip, Visibility::Inherited)).with_children(|clip| {
            for (n, words) in list.lines.iter().enumerate() {
                let top = cy - ay + row.padding[2] + n as f32 * (line_height + row.spacing) - list.scroll;
                if top + line_height < 0.0 || top > ah {
                    continue;
                }
                let said_line: HashMap<usize, String> = text.map(|text| (text, (*words).to_owned())).into_iter().collect();
                let marks = Marks { words: Some(&said_line), ..Marks::default() };
                draw_nodes(clip, look, line, &look.line_drawn, OVERALL_LINES + (n << 8), marks, said, Frame::Is([width, line_height]), [cx - ax + row.padding[0], top], s);
            }
        });
    }
    if let Some(([x, y, w, h], color)) = list.handle {
        let at = Node { position_type: PositionType::Absolute, left: Val::Px(x * s), top: Val::Px(y * s), width: Val::Px(w * s), height: Val::Px(h * s), ..default() };
        column.spawn((at, EXACT, BackgroundColor(color)));
    }
}

#[cfg(test)]
mod overall_tests {
    use super::{format_values, rich_spans};

    /// A localized text's holes filled as `string.Format` fills them: the
    /// online time's `{0}h{1:00}m` with none of either.
    #[test]
    fn a_format_s_holes_are_filled() {
        assert_eq!(format_values("{0}h{1:00}m", &["0", "0"]), "0h00m");
        assert_eq!(format_values("R: {0}", &["12"]), "R: 12");
        assert_eq!(format_values("{1:00}", &["0", "123"]), "123");
        assert_eq!(format_values("{3} left", &["0"]), "{3} left");
    }

    /// TextMeshPro's colour tags as runs of their colours, the others left
    /// out; words without one as they are.
    #[test]
    fn colour_tags_are_runs() {
        assert_eq!(rich_spans("Raids"), None);
        let spans = rich_spans("R: <color=#C5C3B2>0</color>").expect("tagged");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0], ("R: ".to_owned(), None));
        assert_eq!(spans[1].0, "0");
        let colour = spans[1].1.expect("its colour");
        assert!((colour[0] - 0xC5 as f32 / 255.0).abs() < 1e-6 && colour[3] == 1.0);
        let spans = rich_spans("<b>a</b><color=#00000080>b</color>c").expect("tagged");
        assert_eq!(spans.iter().map(|(words, _)| words.as_str()).collect::<String>(), "abc");
        assert!((spans[1].1.expect("b's")[3] - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(spans[2].1, None);
        // A colour by its name (the repair window's `<color=white>`).
        let spans = rich_spans("60/100<color=white> (100)</color>").expect("tagged");
        assert_eq!(spans[1], (" (100)".to_owned(), Some([1.0, 1.0, 1.0, 1.0])));
    }
}

// ==== The repair window ====

/// The repair windows' pieces, each window's past the one before it.
const REPAIR_WINDOW: usize = 30 << 20;
const REPAIR_STATUS: &str = "Inner/Contents/PreviewWarningPanel/RepairStatusPanel";
/// Its warnings' templates in `RepairWarningStatusPanel`'s order: an
/// error's, a warning's, an information's, a common buff's, a rare buff's.
const REPAIR_TEMPLATES: [&str; 5] = [
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RepairErrorPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RepairWarningPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RepairInfoPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/CommonBuffPanelTemplate",
    "Inner/Contents/PreviewWarningPanel/RepairStatusPanel/RareBuffPanelTemplate",
];
const REPAIR_PREVIEW: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/Camera Image";
const REPAIR_LOADER: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/Loader";
const REPAIR_PROCESS: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/RepairProcess";
const REPAIR_MASTERING: &str = "Inner/Contents/PreviewWarningPanel/Preview Panel/MasteringText";
const REPAIR_AVATAR: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerAvatarPanel/Avatar";
const REPAIR_RANK: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerAvatarPanel/Rank Panel";
const REPAIR_CHARGE: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerAvatarPanel/RepairKitChargeText";
const REPAIR_DROPDOWN: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerOptionsPanel/TraderDropdown/Text";
const REPAIR_PRICE_RATE: &str = "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerOptionsPanel/Standing/RepairPriceRate/Text";
const REPAIR_LINES: [&str; 4] = [
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/RepairSpeedLabel",
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/RepairQualityLabel",
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/Repairability",
    "Inner/Contents/RepairerPanel/SelectRepairerPanel/Qualities/RepairerQualitiesPanel/EnhancementChance",
];
const REPAIR_SLIDER: &str = "Inner/Contents/RepairerPanel/ExceptionItemGroup/ConditionPanel/ConditionSlider";
const REPAIR_SUMS: &str = "Inner/Contents/RepairerPanel/ExceptionItemGroup/RepairSumPanel";
const REPAIR_BUTTON: &str = "Inner/Contents/ButtonsContainer/RepairButton";
/// What its scripts show and hide, besides the templates.
const REPAIR_SCRIPTED: [&str; 6] = [REPAIR_LOADER, REPAIR_PROCESS, REPAIR_MASTERING, REPAIR_RANK, "Inner/Contents/RepairerPanel/SelectRepairerPanel/Repairer/RepairerOptionsPanel/Standing/Standing", "Inner/Contents/RepairerPanel/ExceptionItemGroup/ConditionPanel/ConditionSlider/bars/Slider"];

/// The condition's bar as `ConditionCharacteristicsSlider.SetSliderValues`
/// lays it out: fractions of the template durability (the current, the
/// greatest, the slider's top and its value), the degradation's range in
/// points, and the template durability.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RepairCondition {
    pub current: f32,
    pub max: f32,
    pub top: f32,
    pub selected: f32,
    pub range: [f32; 2],
    pub template: f32,
}

/// What the repair window says and shows, as its scripts write it for a
/// kit (`RepairWindow`, `RepairerParametersPanel`, `RepairWarningStatusPanel`,
/// `ConditionCharacteristicsSlider`), worked out by `repair_view`.
#[derive(Clone, Debug, Default)]
pub(crate) struct RepairWords {
    pub caption: String,
    pub kind: String,
    pub weight: String,
    pub mastering: Option<String>,
    pub loading: bool,
    pub repairing: bool,
    /// The warnings shown, each its template (by `REPAIR_TEMPLATES`' order)
    /// and its words.
    pub warnings: Vec<(usize, String)>,
    pub dropdown: String,
    pub charge: String,
    pub price_rate: String,
    pub lines: [String; 4],
    /// The texts the install's words fill (`LocalizedText`), by their paths.
    pub labels: Vec<(String, String)>,
    pub condition: RepairCondition,
    pub durability: String,
    pub repair_time: String,
    pub repair_dp: String,
    pub in_stash: String,
    pub to_pay: String,
    pub to_pay_critical: bool,
    pub broken: bool,
    pub interactable: bool,
}

/// The repair window laid out for a thing: its panel with what its scripts
/// write, its size, and the places of what is clicked or drawn over it: the
/// preview, the kit's picture (the repairer's avatar), the slider's track
/// and the button to its top, the repair button, the caption and the cross.
pub(crate) struct RepairBuilt {
    panel: trp_data::screen::ScenePanel,
    drawn: Drawn,
    texts: HashMap<usize, String>,
    set: HashMap<usize, bool>,
    colors: HashMap<usize, Color>,
    sprites: HashMap<usize, Picture>,
    groups: HashMap<usize, f32>,
    pub(crate) size: [f32; 2],
    pub(crate) preview: [f32; 4],
    pub(crate) avatar: [f32; 4],
    pub(crate) slider: Option<[f32; 4]>,
    pub(crate) top_button: Option<[f32; 4]>,
    pub(crate) button: [f32; 4],
    pub(crate) caption: [f32; 4],
    pub(crate) close: [f32; 4],
}

/// The repair window laid out (`TARKOV UI`): the game's window with what its
/// scripts write (`RepairWords`), each text that sizes itself at its font's
/// size, a copy of a template for each warning in its panel, the condition's
/// bar's pieces at their fractions.
pub(crate) fn repair_built(look: &HealthLook, words: &RepairWords) -> Option<RepairBuilt> {
    let mut panel = look.repair.clone()?;
    let mut drawn = look.repair_drawn.clone();
    let mut texts = HashMap::new();
    let mut set = HashMap::new();
    let mut colors = HashMap::new();
    let mut sprites = HashMap::new();
    let mut groups = HashMap::new();
    let find = |panel: &trp_data::screen::ScenePanel, path: &str| panel.find(path);
    // A text that sizes itself: its preferred size its words' at its font.
    let sized = |panel: &mut trp_data::screen::ScenePanel, texts: &mut HashMap<usize, String>, index: usize, said: &str| {
        let plain = rich_spans(said).map_or_else(|| said.to_owned(), |spans| spans.into_iter().map(|(words, _)| words).collect());
        if let Some(size) = text_size(look, panel, index, &plain) {
            prefer_size(panel, index, size);
        }
        texts.insert(index, said.to_owned());
    };
    // The caption's row (`ItemInfoWindowLabels`), the weight sized to its words.
    for (path, said) in [("Inner/Caption Panel/Caption", &words.caption), ("Inner/Caption Panel/Item Type", &words.kind)] {
        if let Some(index) = find(&panel, path) {
            texts.insert(index, said.clone());
        }
    }
    if let Some(mass) = find(&panel, "Inner/Caption Panel/MassText") {
        if let Some([w, _]) = text_size(look, &panel, mass, &words.weight) {
            panel.nodes[mass].layout.size_delta[0] = w;
        }
        texts.insert(mass, words.weight.clone());
    }
    // The preview's loader, the repair's progress, a weapon's mastering.
    for (path, on) in [(REPAIR_LOADER, words.loading), (REPAIR_PROCESS, words.repairing), (REPAIR_MASTERING, words.mastering.is_some())] {
        if let Some(index) = find(&panel, path) {
            set.insert(index, on);
            panel.nodes[index].active |= on;
        }
    }
    if let (Some(level), Some(index)) = (&words.mastering, find(&panel, REPAIR_MASTERING)) {
        if let Some([w, _]) = text_size(look, &panel, index, level) {
            panel.nodes[index].layout.size_delta[0] = w;
        }
        texts.insert(index, level.clone());
    }
    // A kit is no trader: no rank, no standing (`TraderSelected`).
    for index in find(&panel, REPAIR_RANK).into_iter().chain(look.repair_figures.standing) {
        set.insert(index, false);
        panel.nodes[index].active = false;
    }
    for (path, said) in [(REPAIR_DROPDOWN, &words.dropdown), (REPAIR_CHARGE, &words.charge)] {
        if let Some(index) = find(&panel, path) {
            texts.insert(index, said.clone());
        }
    }
    if let Some(index) = find(&panel, REPAIR_PRICE_RATE) {
        sized(&mut panel, &mut texts, index, &words.price_rate);
    }
    for (path, said) in REPAIR_LINES.iter().zip(&words.lines) {
        if let Some(index) = find(&panel, path) {
            sized(&mut panel, &mut texts, index, said);
        }
    }
    for (path, said) in &words.labels {
        if let Some(index) = find(&panel, path) {
            sized(&mut panel, &mut texts, index, said);
        }
    }
    // The sums' figures, and the kit's points' icons in the cash's place.
    for (end, said) in [("RepairTime/Value", &words.repair_time), ("RepairDP/Value", &words.repair_dp), ("CashInStash/Value", &words.in_stash), ("TotalSum/Value", &words.to_pay)] {
        if let Some(index) = find(&panel, &format!("{REPAIR_SUMS}/{end}")) {
            sized(&mut panel, &mut texts, index, said);
        }
    }
    let [critical, default] = look.repair_figures.sum_colors;
    if let Some(index) = find(&panel, &format!("{REPAIR_SUMS}/TotalSum/Value")) {
        colors.insert(index, rgba(if words.to_pay_critical { critical } else { default }));
    }
    for (end, icon) in [("CashInStash/Icon", look.repair_icons[0]), ("TotalSum/Icon", look.repair_icons[1])] {
        if let (Some(index), Some(picture)) = (find(&panel, &format!("{REPAIR_SUMS}/{end}")), icon) {
            sprites.insert(index, picture);
        }
    }
    // The warnings, each a copy of its template under the panel.
    if let Some(status) = find(&panel, REPAIR_STATUS) {
        for (template, said) in &words.warnings {
            let Some(top) = REPAIR_TEMPLATES.get(*template).and_then(|path| find(&panel, path)) else { continue };
            let copy = copy_within(&mut panel, &mut drawn, top, status);
            set.insert(copy, true);
            texts.insert(copy + 1, said.clone());
        }
    }
    // The condition's bar (`SetSliderValues`), its border and its current
    // durability in the colour broken or not (`SetSliderActive`).
    let c = words.condition;
    let template = c.template.max(1.0);
    let bar = |end: &str| format!("{REPAIR_SLIDER}/bars/{end}");
    let anchor = |panel: &mut trp_data::screen::ScenePanel, end: &str, from: Option<f32>, to: Option<f32>| {
        if let Some(index) = panel.find(&bar(end)) {
            if let Some(from) = from {
                panel.nodes[index].layout.anchor_min[0] = from;
            }
            if let Some(to) = to {
                panel.nodes[index].layout.anchor_max[0] = to;
            }
        }
    };
    let lowest = (c.max - c.range[0] / template).clamp(0.0, 1.0);
    anchor(&mut panel, "CurrentDurability", None, Some(c.current));
    anchor(&mut panel, "MaximumDurability", None, Some(c.max));
    anchor(&mut panel, "MaxDegradation", Some((c.max - c.range[1] / template).clamp(0.0, 1.0)), Some(lowest));
    anchor(&mut panel, "Slider", Some(0.01 + c.current), Some(c.max - c.range[0] / template));
    anchor(&mut panel, "MaximumRemovedDurability", Some(c.current + c.top), Some(c.max));
    // (The game's quirk: the selection's start a fraction over the template
    // again, so from about nought, under the current durability's.)
    anchor(&mut panel, "SelectedDurability", Some(c.current / template), Some(c.current + c.selected));
    let shown = c.top >= 0.001;
    if let Some(slider) = panel.find(&bar("Slider")) {
        set.insert(slider, shown);
        panel.nodes[slider].active = shown;
    }
    let value = if shown { ((c.selected - 0.001) / (c.top - 0.001).max(1e-6)).clamp(0.0, 1.0) } else { 0.0 };
    if let Some(handle) = panel.find(&bar("Slider/Handle Slide Area/Handle")) {
        panel.nodes[handle].layout.anchor_min = [value, 0.0];
        panel.nodes[handle].layout.anchor_max = [value, 1.0];
    }
    let [slider_critical, slider_default] = look.repair_figures.slider_colors;
    let tint = rgba(if words.broken { slider_critical } else { slider_default });
    for end in ["Border", "CurrentDurability"] {
        if let Some(index) = panel.find(&bar(end)) {
            colors.insert(index, tint);
        }
    }
    if let Some(index) = panel.find(&format!("{REPAIR_SLIDER}/Durability")) {
        texts.insert(index, words.durability.clone());
        colors.insert(index, Color::WHITE);
    }
    // The button, dim where it cannot be pressed (`PROVISIONAL`: the
    // button's own disabled look, `DefaultUIButtonAnimation`, not read).
    if let Some(button) = panel.find(REPAIR_BUTTON) {
        if !words.interactable {
            groups.insert(button, 0.5);
        }
    }
    let size = fitted_size(&panel);
    let rect = |path: &str| panel.find(path).map(|index| panel.rect(index, size));
    Some(RepairBuilt {
        preview: rect(REPAIR_PREVIEW).unwrap_or([0.0; 4]),
        avatar: rect(REPAIR_AVATAR).unwrap_or([0.0; 4]),
        slider: shown.then(|| rect(&bar("Slider/Handle Slide Area"))).flatten(),
        top_button: shown.then(|| rect(&bar("Slider/MaximumDurabilityButton"))).flatten(),
        button: rect(REPAIR_BUTTON).unwrap_or([0.0; 4]),
        caption: rect("Inner/Caption Panel").unwrap_or([0.0; 4]),
        close: rect("Inner/Caption Panel/Close Button").unwrap_or([0.0; 4]),
        panel,
        drawn,
        texts,
        set,
        colors,
        sprites,
        groups,
        size,
    })
}

/// The repair window drawn as built, its pieces numbered past the windows
/// before it.
pub(crate) fn draw_repair_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &RepairBuilt, index: usize, s: f32) {
    let RepairBuilt { panel, drawn, texts, set, colors, sprites, groups, size, .. } = built;
    let base = REPAIR_WINDOW + (index << 12);
    let marks = Marks { words: Some(texts), set: Some(set), colors: Some(colors), sprites: Some(sprites), groups: Some(groups), ..Marks::default() };
    draw_nodes(column, look, panel, drawn, base, marks, said, Frame::Is(*size), [0.0, 0.0], s);
}
