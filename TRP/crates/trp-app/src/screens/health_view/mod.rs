//! Health on the screen and in the world: the HEALTH tab, the death screen and the barbed wire.
//!
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
use trp_sim::condition::health::{BarbTally, DamageKind, Health, Kind, Part};

use crate::player::body_view::{BodyParts, BodyRig};
use crate::player::controller::Controller;
use crate::screens::inventory_view::{words, FAINT, WORDS};
use crate::world::range::Range;

mod looks;
mod pixels;
mod scene_read;
mod screens_read;
mod scene_fonts;
mod panels_read;
mod tmp_texts;
mod view_looks;
mod sprites_tips;
mod info_layout;
mod info_pieces;
mod info_table;
mod eb_stats;
mod grid_window;
mod menu;
mod item_grounds;
mod tag_window;
mod panel_pieces;
mod strip_tabs;
mod stash;
mod row_search;
mod quick_containers;
mod nodes;
mod own_panel;
mod health_panel;
mod damage;
mod overall;
mod repair_window;

pub use looks::*;
pub use pixels::*;
pub(crate) use panels_read::*;
pub use tmp_texts::*;
pub use view_looks::*;
pub use sprites_tips::*;
// Named outright: the engine's prelude has a `Sprite` of its own.
pub use sprites_tips::Sprite;
pub use info_layout::*;
pub(crate) use info_pieces::*;
pub use info_table::*;
pub(crate) use eb_stats::*;
pub(crate) use grid_window::*;
pub use menu::*;
pub use item_grounds::*;
pub use tag_window::*;
pub use panel_pieces::*;
pub use strip_tabs::*;
pub use stash::*;
pub use row_search::*;
pub use quick_containers::*;
pub(crate) use nodes::*;
pub use own_panel::*;
pub use overall::*;
pub(crate) use repair_window::*;
use looks::{EB_CHAR_TEMPLATE, EB_COMPACT_TEMPLATE, EB_EMPTY_FIELD, EB_NEVER, OVERALL_EXPERIENCE, OVERALL_EXPERIENCE_ICON, OVERALL_MARK, OVERALL_NICKNAME, RANKS};
use pixels::{BACK_BUTTON, BORDER, CONTAINERS, FOOT, GEAR, GRID, INFO, MENU, MENU_LINES, MENU_TEMPLATES, QUICK, STASH, STRIP, TABS, TAG_WINDOW, masked_fills};
#[cfg(test)]
use pixels::{sample};
use panels_read::{edit_build_panels, health_shown, overall_panel};
use view_looks::{quick_value_look};
use sprites_tips::{PartsMarks};
use info_layout::{INFO_BARS, INFO_BUTTONS, INFO_BUTTON_TEMPLATES, INFO_COMPACT, INFO_CORNER, INFO_DESCRIPTION, INFO_DURABILITY, INFO_LOADER, INFO_MASS, INFO_MASTERING, INFO_MASTERING_BADGE, INFO_MASTERING_VALUE, INFO_MODS, INFO_MODS_BAR, INFO_MODS_CONTENT, INFO_OFF, INFO_SCROLL, INFO_TAG, INFO_WISH, MOD_SLOT_PIECES, info_size};
use info_pieces::{fitted_size, graft_row, info_buttons, info_mastering, info_slots, inverse_lerp};
use info_table::{info_rows};
use grid_window::{WINDOW_SEARCH_PARTS};
use menu::{menu_line_colors};
use item_grounds::{FontAssets, FontsRead, TAG_COLORS, finish_looks, font_assets, make_fonts, make_image, make_pictures, receive_screens, share_item_looks, show_version};
use tag_window::{TAG_CELL, TAG_COLORS_PANEL, TAG_COUNT, TAG_INPUT, TAG_PLACEHOLDER, TAG_SAVE_SIZE, TAG_TEXT, tag_cell, tmp_line};
use panel_pieces::{Bob, EffectWords, PieceSpan, PieceText, fill_effect_icons, light_drop_glow, light_info_buttons, show_effect_tooltip};
use strip_tabs::{options_tab_left};
use stash::{tint_search_button};
#[cfg(test)]
use stash::{step_tint};
use row_search::{ROW_SEARCH_PARTS, SEARCH_PARTS, fill_images};
use quick_containers::{SPECIAL_SLOTS_HEADER};
use nodes::{SliceOf, Slices, nine_slice, slice_pieces};
use own_panel::{FALLING, FIGURES, FigureRow, RISING, Want, figure, figure_rows, fill_own, part_color};
use health_panel::{fill_panel};
use damage::{IMMUNITY_NOTICE, armour_slows, barbed_wire, damage_asked, damage_keys, hits_taken, show_death, spawn_death_screen};
#[cfg(test)]
use overall::{without_figure};
#[cfg(test)]
use overall::format_values;
use repair_window::{REPAIR_SCRIPTED, REPAIR_TEMPLATES};

// ==== The plugin and the words ====

pub struct HealthViewPlugin;

impl Plugin for HealthViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_death_screen, make_pictures, share_item_looks.after(make_pictures)))
            .init_resource::<SearchTint>()
            // The rest of the screens, merged in on the frame they come (the
            // warm-up waits for them: `ScreensComing`).
            .add_systems(First, receive_screens)
            .add_systems(Update, (fill_panel, fill_own, fill_effect_icons, show_effect_tooltip, light_drop_glow, light_info_buttons, tint_search_button).after(crate::screens::inventory_view::ScreenDrawn))
            .add_systems(Update, (show_death, damage_asked, damage_keys.after(crate::screens::inventory_view::ScreenDrawn), show_version))
            .add_systems(Update, hits_taken.after(damage_asked).after(damage_keys).after(barbed_wire))
            .add_systems(Update, armour_slows)
            .add_systems(Update, barbed_wire.after(crate::player::controller::step))
            // Texts drawn as the game draws them, as they are made or
            // changed, before the screen is laid out.
            .add_systems(PostUpdate, draw_tmp_texts.before(bevy::ui::UiSystems::Layout))
            // With `TARKOV UI`, the screens laid out to a fraction of a
            // pixel, their texts put on whole pixels once laid out.
            .add_systems(PostUpdate, lay_out_unrounded.before(bevy::ui::UiSystems::Prepare))
            .add_systems(PostUpdate, texts_on_pixels.after(bevy::ui::UiSystems::Layout).before(bevy::ui::UiSystems::PostLayout));
    }
}

/// With `TARKOV UI`, every screen laid out as the game's engine lays out
/// its canvas: each thing where its units put it at the screen's scale, a
/// fraction of a pixel included (a unit is a third of a pixel more than
/// one at 1440 lines), never rounded to the screen's pixels, and a pixel
/// filled by what covers its middle. Bevy rounds a node's place unless told
/// not to, and an unrounded node under a rounded one lies from that one's
/// rounded corner: the game's pictures (drawn unrounded, `EXACT`) fell up
/// to half a pixel from the game's places under the rounded panels holding
/// them, a grid's light edge and a quick view's on another pixel than the
/// game's, or half lit where the game's is whole. Each root node as it comes is set unrounded, and all under it
/// with it.
fn lay_out_unrounded(look: Res<HealthLook>, mut commands: Commands, roots: Query<Entity, (Added<Node>, Without<ChildOf>, Without<bevy::ui::LayoutConfig>)>) {
    if !look.tarkov_ui {
        return;
    }
    for root in &roots {
        commands.entity(root).insert(EXACT);
    }
}

/// With `TARKOV UI`, each text on whole pixels of the screen once laid out:
/// laid out unrounded it may lie a fraction of a pixel off, and the app
/// draws a text as a picture made at the screen's own pixels (Bevy's
/// glyphs, or the game's words as `draw_tmp_texts` makes them), which
/// drawn a fraction off is blurred. Its top left goes to the nearest whole
/// pixel, less than half a pixel, as it is drawn: where anything lies, and
/// what the mouse finds there, stays the layout's.
fn texts_on_pixels(look: Res<HealthLook>, mut texts: Query<(&ComputedNode, &mut UiGlobalTransform), Or<(With<Text>, With<TmpText>)>>) {
    if !look.tarkov_ui {
        return;
    }
    for (node, mut transform) in &mut texts {
        let affine = transform.affine();
        // (A turned or scaled text is left as it lies.)
        if affine.matrix2 != bevy::math::Mat2::IDENTITY {
            continue;
        }
        let half = node.size() * 0.5;
        let corner = affine.translation - half;
        let whole = corner.round();
        if whole != corner {
            *transform = UiGlobalTransform::from(bevy::math::Affine2::from_translation(whole + half));
        }
    }
}

/// The install's words for what the page and the death screen show.
#[derive(Resource, Default)]
pub struct Words(HashMap<String, String>);

impl Words {
    /// The words wanted, and those of the health panel's own texts
    /// (`extra`: a header's text is the key its words are found by,
    /// `LocalizedText`), from the run's texts (`Locale::words`).
    pub fn load(words: &HashMap<String, String>, extra: &[String]) -> Self {
        let mut wanted: Vec<String> = Part::ALL.iter().map(|part| part.name().to_owned()).collect();
        let kinds = [Kind::LightBleeding, Kind::HeavyBleeding, Kind::Wound, Kind::Pain, Kind::Tremor, Kind::Dehydration, Kind::Exhaustion, Kind::ChronicFatigue, Kind::Contusion, Kind::Intoxication, Kind::Stun, Kind::TunnelVision, Kind::Berserk];
        wanted.extend(kinds.iter().map(|kind| kind.name().to_owned()));
        wanted.extend(["Painkiller", "BrokenBone", "Encumbered", "OverEncumbered", IMMUNITY_NOTICE, "Killed in Action", "Killed by"].map(str::to_owned));
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
            DamageKind::Poison,
        ];
        wanted.extend(causes.iter().map(|kind| format!("DamageType_{}", kind.name())));
        wanted.push(SPECIAL_SLOTS_HEADER.to_owned());
        // The menu of a right click: its lines' captions and its reasons.
        wanted.extend(trp_sim::items::item_menu::Line::ALL.iter().map(|line| line.name().to_owned()));
        wanted.extend(trp_sim::items::item_menu::REASONS.iter().map(|key| (*key).to_owned()));
        wanted.extend(trp_sim::items::item_menu::WishLine::ALL.iter().map(|line| line.caption_key().to_owned()));
        // The window that edits a tag: its caption, its field's placeholder
        // and its button.
        wanted.extend(["ADD/EDIT CONTAINER TAG", "container tag", "Save"].map(str::to_owned));
        // A search's words (`SearchButton.SetSearchStatus`: "SEARCHING..."
        // is "SEARCHING" in the install's English; `UNSEARCHED`).
        wanted.extend(["SEARCH", "SEARCHING...", "UNSEARCHED"].map(str::to_owned));
        // The window that asks how many: its captions and its button.
        wanted.extend(crate::screens::split_view::WORD_KEYS.map(str::to_owned));
        // The weapon's malfunctions' notices (`crate::wear`) and the repair
        // window's words (`crate::repair_view`).
        wanted.extend(crate::weapons::wear::word_keys());
        wanted.extend(crate::screens::repair_view::word_keys());
        // The SKILLS tab's: the skills', the buffs' and the page's words.
        wanted.extend(crate::screens::skills_view::word_keys());
        wanted.extend(crate::screens::mastery_view::word_keys());
        // The stimulators' buffs' names, and the weight's units (`StimulatorHelper.
        // BuffName`, `UnitsName`).
        wanted.extend(trp_sim::condition::health::BuffKind::names().map(str::to_owned));
        wanted.push(" kg".to_owned());
        wanted.extend(extra.iter().cloned());
        let names: Vec<&str> = wanted.iter().map(String::as_str).collect();
        Self(trp_data::character::inventory::words_of(words, &names))
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
            Kind::ImmunityPrevented => (IMMUNITY_NOTICE, "Immunity"),
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

// ==== Tests ====

#[cfg(test)]
mod tests;

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

#[cfg(test)]
mod overall_tests;
