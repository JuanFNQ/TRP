//! HealthLook and the other looks read from the game's scene, with their paths.

use super::*;

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
    pub(super) version: Option<(trp_data::interface::screen::ScenePanel, f32)>,
    pub panel: Option<trp_data::interface::screen::HealthPanel>,
    /// Each panel's objects as drawn: the health panel's, the strip's and
    /// the foot's.
    pub(super) health: Drawn,
    pub(super) strip: Drawn,
    pub(super) foot: Drawn,
    pub(super) gear: Drawn,
    pub(super) border: Drawn,
    /// The textures the pictures are cut from: read at the start, made
    /// images by `make_pictures`.
    pub(super) textures: Vec<Option<trp_data::unity::extract::TextureData>>,
    pub(crate) images: Vec<Option<Handle<Image>>>,
    /// The views' glow as their button's animator holds it standing (its
    /// `Normal` clip: the alpha of `BackGlowImage`), lit only as a thing
    /// that can be used on the part is held over it (`Highlighted`).
    pub(super) glow_alpha: Option<f32>,
    /// The glow lit as a thing that can be used on the part is carried over
    /// it (`HealthBarButton.OnPointerEnter`: the animator's `Highlighted`,
    /// its glow's alpha from 0 to 1 in a third of a second): the clip and
    /// its curve of the glow's alpha.
    pub(super) glow_lit: Option<(trp_data::unity::anim::Clip, usize)>,
    /// A rate's arrows while the figure moves (their animator's
    /// `BuffPaneActive`, looping): the clip, and its curves of the arrow's
    /// height and alpha. Standing (`BuffPanePassive`), they are still and
    /// whole.
    pub(super) bob: Option<(trp_data::unity::anim::Clip, usize, usize)>,
    /// The carried weight's two limits the weight's figure is coloured by
    /// (`Stamina.LowerOverweightLimit`, `UpperOverweightLimit`).
    pub(super) weight_limits: Option<[f32; 2]>,
    /// The game's own fonts its panels' texts are drawn in, by their font
    /// asset (the file it is in among the scene's, and its number): read
    /// at the start, made fonts by `make_pictures`.
    pub(super) font_data: Vec<((String, i64), Vec<u8>)>,
    pub(super) fonts: HashMap<(String, i64), Handle<Font>>,
    /// Each font asset's own name (its `m_Name`, as the game's recorder
    /// writes a text's font), by the same key: for the interface's dump
    /// (`ui_dump`).
    pub(super) font_names: HashMap<(String, i64), String>,
    /// The game's views of a thing in a slot and in a grid (`TARKOV UI`)
    /// and their pictures; made, at the start, what the inventory draws a
    /// thing by (`item_looks`).
    pub(super) items: Option<trp_data::interface::screen::ItemViews>,
    pub(super) slot_item: Drawn,
    pub(super) grid_item: Drawn,
    /// The load component a magazine's view shows while it is worked
    /// (`ItemViewLoadAmmoComponent`), and its pictures.
    pub(super) spin: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) spin_drawn: Drawn,
    /// The examination's view over a thing's view while it is examined
    /// (`ItemViewExamineComponent`), and its pictures.
    pub(super) examine: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) examine_drawn: Drawn,
    /// The game scene's panel of an examine (`UsingPanel`, `using_view`),
    /// and its pictures.
    pub(crate) using: Option<trp_data::interface::screen::UsingLook>,
    pub(crate) using_drawn: Drawn,
    pub item_looks: [Option<std::sync::Arc<ItemViewLook>>; 2],
    /// A quick-use view's figure (`QuickSlotItemView._resourceValue`).
    pub quick_value: Option<QuickValueLook>,
    /// The containers' column (`TARKOV UI`): the scene's panel and its
    /// pictures, a grid view's, each container slot's empty picture
    /// (`Slots/Equipment/<slot>`), and containers' own layouts of their
    /// grids by name, each with its size and its grids' rectangles.
    pub containers: Option<trp_data::interface::screen::ContainerViews>,
    pub(super) containers_drawn: Drawn,
    pub(super) grid_drawn: Drawn,
    pub(super) container_backs: HashMap<String, Picture>,
    /// The parts' marks by their keys (`ItemViewLook::mods`).
    pub(super) mod_pictures: HashMap<String, Picture>,
    /// The health's effects' icons by the game's names
    /// (`trp_data::screen::EFFECT_ICONS`), and the game's simple tooltip,
    /// which says what one is with the mouse on it.
    pub(crate) effect_pictures: HashMap<&'static str, Picture>,
    pub(super) tooltip: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) tooltip_drawn: Drawn,
    /// The game's tooltip of lines (`MultiLineTooltip`), with its pictures.
    pub(super) multi_tooltip: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) multi_tooltip_drawn: Drawn,
    /// The battle screen's panels of the character (`TARKOV UI`): the
    /// stance panel and the health panel as drawn, and the sprites their
    /// scripts put on them (the stamina's fills, the noise's levels), by
    /// the stance panel's pointer to each.
    pub(crate) battle: Option<trp_data::interface::screen::BattleViews>,
    pub(crate) stance_drawn: Drawn,
    pub(crate) body_drawn: Drawn,
    /// The battle screen's quick bar (`QuickViews::load_battle`) as drawn.
    pub(crate) battle_quick: Option<trp_data::interface::screen::QuickViews>,
    pub(crate) battle_quick_drawn: Drawn,
    pub(crate) battle_sprites: HashMap<(i32, i64), Picture>,
    /// The game's own settings of when they show and how the parts are
    /// coloured.
    pub(crate) battle_modes: crate::panels::battle_view::BattleModes,
    /// What each font asset's material draws under its letters (a rim, a
    /// drop shadow), by the asset's file and number.
    pub(super) text_effects: HashMap<(String, i64), TextEffect>,
    pub layouts: HashMap<String, ([f32; 2], Vec<trp_data::interface::screen::LayoutGrid>)>,
    /// The bar of tabs (`TARKOV UI`): the scene's bar with its tabs laid
    /// out at their width (`TabBar::lay_out`), its pictures, each tab's
    /// words, the tabs' width, and each tab's masked fill by its mask's
    /// object (`MaskedFill`).
    pub tab_bar: Option<trp_data::interface::screen::TabBar>,
    pub(super) tabs_drawn: Drawn,
    /// The screen's back button as it is drawn at rest.
    pub(super) back_drawn: Drawn,
    pub tab_words: Vec<String>,
    pub tab_width: f32,
    pub tab_fills: HashMap<usize, std::sync::Arc<MaskedFill>>,
    /// The icon of the app's own tab, the options, at rest and chosen: the
    /// game's GAME SETTINGS button's cog (`TabBar::settings_icons`).
    pub(super) options_icons: [Option<Picture>; 2],
    /// The panel for a container opened (`TARKOV UI`), its pictures, its
    /// masked stripes, and the font asset of the container's name, which
    /// sizes the name's box.
    pub stash: Option<trp_data::interface::screen::StashViews>,
    pub(super) stash_drawn: Drawn,
    pub stash_fills: HashMap<usize, std::sync::Arc<MaskedFill>>,
    pub(super) name_font: Option<trp_data::interface::screen::TmpFont>,
    /// The quick-use panel (`TARKOV UI`), its views on as its script shows
    /// them, and its pictures.
    pub quick: Option<trp_data::interface::screen::QuickViews>,
    pub(super) quick_drawn: Drawn,
    /// A font asset's bold typeface, where it has one (its weight table's
    /// 700), which TextMeshPro draws its bold in.
    pub(super) bold_fonts: HashMap<(String, i64), (String, i64)>,
    /// The window that shows a thing (`TARKOV UI`; `InfoWindow`), its
    /// pictures, and the font asset of its weight's figure, which sizes
    /// the figure's box (`ContentSizeFitter`).
    pub(crate) info: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) info_drawn: Drawn,
    /// The window of a container's grids (`TARKOV UI`; `GridWindow`) as its
    /// script leaves it in a raid, and its pictures.
    pub(crate) grid_window: Option<trp_data::interface::screen::GridWindowLook>,
    pub(super) grid_window_drawn: Drawn,
    /// The game's repair window (`RepairWindow`; `repair_built`): its
    /// prefab, its objects as drawn, its scripts' figures and the two
    /// icons a kit's points take.
    pub(crate) repair: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) repair_drawn: Drawn,
    pub(crate) repair_figures: trp_data::interface::screen::RepairFigures,
    pub(super) repair_icons: [Option<Picture>; 2],
    /// The window that asks how many (`SplitDialog`), with what its
    /// scripts point to, and its pictures by its objects (both sliders').
    pub(crate) split: Option<trp_data::interface::screen::SplitLook>,
    pub(crate) split_pictures: Vec<Option<Picture>>,
    pub(super) info_mass_font: Option<trp_data::interface::screen::TmpFont>,
    /// The menu of a right click (`TARKOV UI`; `ItemContextMenu`), its
    /// pictures with its templates of a line on, the font asset of a
    /// line's caption, which sizes the line, and each line's icon by the
    /// line's name (`Characteristics/Icons/<line>`).
    pub(crate) menu: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) menu_drawn: Drawn,
    /// The menu of a line's own lines (`TARKOV UI`; `SubMenuTemplate`) and
    /// its pictures with its templates of a line on.
    pub(crate) submenu: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) submenu_drawn: Drawn,
    /// The wish list's groups' icons (`StaticIcons.WishlistSprites`), and
    /// the outlined ones a thing's view shows (`WishlistSpritesOutline`), by
    /// the group's name.
    pub(super) wish_icons: HashMap<&'static str, Picture>,
    pub(super) wish_outlines: HashMap<&'static str, Picture>,
    /// The window that edits a tag (`TARKOV UI`; `EditTagWindow`), with its
    /// seven colour cells on and what its texts and its grid of colours ask
    /// of its layout put on them; its pictures; and the most letters its
    /// field takes (`m_CharacterLimit`), and its field's own figures (its
    /// caret, its selection's colour: `screen::input_field`).
    pub(crate) tag_window: Option<trp_data::interface::screen::ScenePanel>,
    pub(super) tag_window_drawn: Drawn,
    pub(crate) tag_limit: u32,
    pub(crate) tag_field: Option<trp_data::interface::screen::InputField>,
    pub(super) menu_font: Option<trp_data::interface::screen::TmpFont>,
    pub(super) menu_icons: HashMap<&'static str, Picture>,
    /// The font assets its texts are drawn from as the game draws them
    /// (`trp_data::tmp_text`), each with its atlas and its bold typeface's,
    /// by the asset's file and number; the materials they are drawn with,
    /// by theirs; and each asset's own material.
    pub(crate) tmp_faces: HashMap<(String, i64), std::sync::Arc<trp_data::interface::tmp_text::TmpFace>>,
    pub(crate) tmp_materials: HashMap<(String, i64), trp_data::interface::tmp_text::TmpMaterial>,
    pub(super) tmp_default_material: HashMap<(String, i64), (String, i64)>,
    /// The OVERALL tab (`TARKOV UI`; inventory spec, "The OVERALL tab"): the
    /// scene's panel, its clothing panel on where the options' `CLOTHING`
    /// is; its pictures, a dropdown line's, and those its scripts put on it
    /// (the level's rank, the side's, the prestige's), by their names.
    pub overall: Option<trp_data::interface::screen::OverallPanel>,
    pub(super) overall_drawn: Drawn,
    pub(super) line_drawn: Drawn,
    pub(super) overall_pictures: HashMap<&'static str, Picture>,
    /// The colour the character's name is written in by its member
    /// category (`IconColor`), the mark's picture being `"member"` above.
    pub(super) member_colour: Option<[f32; 4]>,
    /// The SKILLS tab as the game draws it (`TARKOV UI`; `skills_view`),
    /// and the experience each level takes (`exp_table`), which its top
    /// panel counts by.
    pub(crate) skills: Option<crate::screens::skills_view::SkillsLook>,
    pub(crate) level_experience: Vec<u32>,
    /// A skill's effectiveness as the raid begins (`skills_view::
    /// effectiveness` at none earned), which the skills' views and
    /// tooltip show; 1 where it was not read.
    pub(crate) skill_effectiveness: Option<f32>,
    /// The edit build screen as the game draws it (`TARKOV UI` with the
    /// workbench on; workbench spec, "The screen as the game draws it").
    pub(crate) edit_build: Option<EditBuildLook>,
    /// The inspect window's own preview, its two lights and its exposure
    /// (`trp_data::edit_build::InspectPreview`), which light the OVERALL
    /// tab's character, the inspect window's model and the edit build
    /// screen's: read once with the rest (`read`), where it can be read.
    pub(crate) inspect: Option<std::sync::Arc<trp_data::interface::edit_build::InspectPreview>>,
    /// The inspect window's rows (inspect-window spec, "A row"): its bar,
    /// compact and dropdown rows' templates and pictures, their scripts'
    /// colours and the bar's adding and subtracting pictures; and the
    /// attributes' icons by the attribute's name.
    pub(crate) info_rows: Option<InfoRowsLook>,
    pub(super) attribute_icons: HashMap<String, Picture>,
    /// The other three tables of a figure's icons, by the enum its id is of
    /// (`trp_sim::item_attributes::Table`): a health effect's, a damage
    /// effect's and a buff's (the buffs' by the app's name for the kind: the
    /// table cuts its names to thirteen letters).
    pub(super) figure_icons: [HashMap<String, Picture>; 3],
    /// The slots' pictures (`Resources/Slots/<name>`, by the name in lower
    /// case), which an empty slot of the parts' panel shows; how far the
    /// window stretches (`StretchWindow._maxSize`).
    pub(super) slot_pictures: HashMap<String, Picture>,
    pub(crate) info_stretch: Option<[f32; 2]>,
    /// The armour classes' marks a tooltip's words name
    /// (`<sprite name="armor_classes_N">`), by class.
    pub(super) class_marks: HashMap<u32, Picture>,
}

/// The inspect window's row templates as drawn.
pub(crate) struct InfoRowsLook {
    pub(super) bar: (trp_data::interface::screen::ScenePanel, Drawn),
    pub(super) compact: (trp_data::interface::screen::ScenePanel, Drawn),
    pub(super) dropdown: (trp_data::interface::screen::ScenePanel, Drawn),
    pub(super) bar_look: trp_data::interface::screen::RowLook,
    pub(super) compact_look: trp_data::interface::screen::RowLook,
    pub(super) add: Option<Picture>,
    pub(super) sub: Option<Picture>,
    /// A slot of the parts' panel (`ModSlotView`), its pieces as its script
    /// shows them per slot.
    pub(super) mod_slot: Option<(trp_data::interface::screen::ScenePanel, Drawn)>,
    /// Such a slot's picture for a chamber not checked
    /// (`_uncheckedChamberSprite`, `icon_question`).
    pub(super) unchecked_chamber: Option<Picture>,
    pub(super) unchecked_sprite: Option<(String, i64)>,
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
    pub(crate) scene: std::sync::Arc<trp_data::interface::edit_build::EditBuildScene>,
    pub(crate) screen: Drawn,
    pub(crate) slot: trp_data::interface::screen::ScenePanel,
    pub(crate) slot_drawn: Drawn,
    pub(crate) empty: trp_data::interface::screen::ScenePanel,
    pub(crate) empty_drawn: Drawn,
    pub(super) part: Option<(trp_data::interface::screen::ScenePanel, Drawn)>,
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
    pub(super) panel: trp_data::interface::screen::ScenePanel,
    pub(super) drawn: Drawn,
    pub(super) bar: (trp_data::interface::screen::ScenePanel, Drawn),
    pub(super) compact: (trp_data::interface::screen::ScenePanel, Drawn),
    pub(super) bar_look: trp_data::interface::screen::RowLook,
    pub(super) compact_look: trp_data::interface::screen::RowLook,
    pub(super) weight_look: trp_data::interface::screen::RowLook,
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

pub(super) const EB_NEVER: [&str; 4] = ["Attention", "CharacteristicsPanel", "Context Menu Area", "Ellipse Rect"];

/// The characteristics panel's row templates, in it.
pub(super) const EB_CHAR_TEMPLATE: &str = "CharacteristicsExpanded/CharTemplate";

pub(super) const EB_COMPACT_TEMPLATE: &str = "CharacteristicsExpanded/CompactTemplate";

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
pub(super) const EB_EMPTY_FIELD: usize = 40 + 13 * 12;

/// The OVERALL tab's objects the app goes by, below its panel's top.
pub const OVERALL_VIEW: &str = "LeftSide/CharacterPanel/PlayerModelView";

pub const OVERALL_CLOTHING: &str = "LeftSide/ClothingPanel";

pub const OVERALL_SELECTORS: [&str; 2] = ["LeftSide/ClothingPanel/Upper/ClothingSelector", "LeftSide/ClothingPanel/Lower/ClothingSelector"];

/// The character's name and its mark (`_nicknameLabel`, `_specialIcon`),
/// and the experience's icon and figure (`_experienceLabel`), each row
/// sized by what it holds (`HorizontalLayoutGroup`, `ContentSizeFitter`).
pub(super) const OVERALL_NICKNAME: &str = "LeftSide/CharacterPanel/PlayerModelView/BottomField/NicknameAndKarma/Label";

pub(super) const OVERALL_MARK: &str = "LeftSide/CharacterPanel/PlayerModelView/BottomField/NicknameAndKarma/AccountType";

pub(super) const OVERALL_EXPERIENCE_ICON: &str = "LeftSide/CharacterPanel/PlayerModelView/BottomField/Experience/Image";

pub(super) const OVERALL_EXPERIENCE: &str = "LeftSide/CharacterPanel/PlayerModelView/BottomField/Experience/ExpValue";

/// (The statistics are drawn whole, each figure the game's `-`:
/// `overall_texts`.)
/// The level of a character with no experience (`ProfileInfo.GetLevel(0)`:
/// the first of the experience table's levels), its rank's picture
/// (`PlayerLevelPanel.SetLevelIcon`: `Level Icons/rank` and the level's
/// five up from it) and how it is written (`{level:D2}`).
/// The ranks' pictures by name (`Level Icons/rank5` to `rank85`), the
/// level's by `PlayerLevelPanel.Set`'s rule.
pub(super) const RANKS: [&str; 17] = ["rank5", "rank10", "rank15", "rank20", "rank25", "rank30", "rank35", "rank40", "rank45", "rank50", "rank55", "rank60", "rank65", "rank70", "rank75", "rank80", "rank85"];

/// The rank picture's name for a level (`PlayerLevelPanel.Set`: the next
/// five up, at most 80).
pub(crate) fn rank_name(level: u32) -> &'static str {
    let rank = if level > 80 { 80 } else { (level / 5 + 1) * 5 };
    RANKS.get((rank / 5).saturating_sub(1) as usize).copied().unwrap_or("rank5")
}
