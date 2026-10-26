//! The battle screen's panels at the lower right (optics spec, "Zeroing",
//! the HUD; inventory spec, "What the ammunition's panel says"): the
//! ammunition's (`AmmoPanel`: the zeroing distance, the way of firing, a
//! look at the magazine's count with the top round's name on a details line
//! under it) and the optic's (`OpticCratePanel`: the magnification), each
//! where the game's screen has it, its dark ground sized to its words as its
//! row and `ContentSizeFitter` size it (the words' width and height as
//! TextMeshPro works them out, `TmpFont`, and the row's padding), faded in
//! over half a second, held three seconds and faded out
//! (`BattleUIComponentAnimation`: `battle_view::Fader`). What they name is
//! what the sights tell (`trp_sim::sights::SightHud`) and what the hands
//! do (`PanelCall`, from the controller and the inventory). Drawn whatever
//! `TARKOV UI` says (the user, 2026-10-24); the words in the game's own font
//! file, drawn by the engine's text (`PROVISIONAL`: not by TextMeshPro's
//! own drawing, which the `TARKOV UI` panels' texts are).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::battle_view::Fader;
use crate::controller::Controller;
use trp_sim::sights::SightHud;

pub struct SightPanelsPlugin;

impl Plugin for SightPanelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (follow_sights, draw_sight_panels).chain().after(crate::controller::step));
    }
}

/// What the hands tell the ammunition's panel, beside the sights: a way of
/// firing named (`ShowFireMode`), or a magazine's count in words with the
/// top round's name (`ShowAmmoDetails`).
#[derive(Debug, Clone, PartialEq)]
pub enum PanelCall {
    FireMode(trp_sim::FireMode),
    /// The magazine of the weapon in hand looked at: its words are made
    /// here, from what is carried.
    Magazine,
}

/// What the ammunition's panel says of the magazine of the weapon in hand
/// (`ShowAmmoDetails`; inventory spec, "What the ammunition's panel says"):
/// its count in words at the look's level (Mag Drills' level over 10, at
/// least the magazine's `CheckOverride`), a cylinder or a weapon of several
/// chambers by the count itself; under it the top round's full name, none
/// where it is empty. `PROVISIONAL`: a cylinder's round named is its first
/// camora's, where the game takes the one the hammer is at.
fn magazine_words(carried: &crate::inventory_view::Carried, held: Option<trp_sim::inventory::Id>, drills: u32) -> Option<(String, Option<String>)> {
    use trp_sim::inventory::{ammo_words, magazine_round_at, magazine_rounds};
    let weapon = carried.find(held?)?;
    let magazine = weapon.in_slot("mod_magazine")?;
    let camoras: Vec<usize> = (0..magazine.kind.slots.len()).filter(|&i| magazine.kind.slots[i].name.starts_with("camora_")).collect();
    let chambers = weapon.kind.slots.iter().filter(|slot| slot.name.starts_with("patron_in_weapon")).count();
    let folding = !camoras.is_empty() || chambers > 1;
    let (count, max, top) = if camoras.is_empty() {
        (magazine_rounds(magazine), magazine.kind.capacity, magazine_round_at(magazine, 0))
    } else {
        let full: Vec<&trp_sim::inventory::Item> = camoras.iter().filter_map(|&i| magazine.slots.get(i).and_then(Option::as_ref)).collect();
        (full.len() as u32, camoras.len() as u32, full.first().map(|round| round.kind.clone()))
    };
    let level = (drills / 10).max(magazine.kind.check_override);
    let say = |key: &str| carried.texts.say(key).unwrap_or_else(|| key.to_owned());
    let words = match ammo_words(count, max, level, folding).key() {
        ("", Some(n)) => n.to_string(),
        (key, Some(n)) => format!("{} {n}", say(key)),
        (key, None) => say(key),
    };
    let details = top.filter(|_| count > 0).map(|round| carried.texts.names.get(&round.template).cloned().unwrap_or_else(|| round.name.clone()));
    Some((words, details))
}

/// The game's key for a way of firing's words (`EFireMode` by its name).
fn fire_mode_key(mode: trp_sim::FireMode) -> &'static str {
    use trp_sim::FireMode as M;
    match mode {
        M::Single => "single",
        M::FullAuto => "fullauto",
        M::Burst => "burst",
        M::SemiAuto => "semiauto",
        M::DoubleAction => "doubleaction",
        M::Doublet => "doublet",
    }
}

/// A text's look, its font asset for measuring it, and its font's file for
/// drawing it.
struct WordsLook {
    text: trp_data::screen::TextLook,
    measure: Option<trp_data::screen::TmpFont>,
    font_bytes: Option<Vec<u8>>,
    font: Option<Handle<Font>>,
}

impl WordsLook {
    fn read(install: &trp_data::Install, text: trp_data::screen::TextLook, font_file: Option<&String>) -> Self {
        let id = text.font.1;
        let measure = font_file.and_then(|file| trp_data::screen::TmpFont::read(install, file, id).ok());
        let font_bytes = font_file.and_then(|file| {
            let fonts = trp_data::screen::build_fonts(install, file).ok()?;
            let asset = trp_data::screen::script_name(install, file, id).unwrap_or_default();
            trp_data::screen::font_for(&asset, &fonts)
                .or_else(|| measure.as_ref().and_then(|face| trp_data::screen::font_by_face(&face.family, &face.style, &fonts)))
                .map(|(_, bytes)| bytes.clone())
        });
        Self { text, measure, font_bytes, font: None }
    }

    /// Its words' width and height as TextMeshPro works them out, margins
    /// and all.
    fn size(&self, words: &str) -> [f32; 2] {
        let t = &self.text;
        let (width, height) = match &self.measure {
            Some(face) => (face.width(words, t.size, t.spacing, t.style & 1 != 0), face.height(t.size)),
            None => (words.chars().count() as f32 * t.size * 0.5, t.size * 1.1),
        };
        [width + t.margin[0] + t.margin[2], height + t.margin[1] + t.margin[3]]
    }

    fn font(&mut self, fonts: &mut Assets<Font>) -> Handle<Font> {
        if self.font.is_none() {
            self.font = self.font_bytes.as_ref().and_then(|bytes| Font::try_from_bytes(bytes.clone()).ok()).map(|font| fonts.add(font));
        }
        self.font.clone().unwrap_or_default()
    }
}

/// A details line as read: its words, its layout, and its icon: its
/// picture (the texture as the engine keeps it and the cut in its pixels)
/// and the icon's layout.
struct DetailsLook {
    words: WordsLook,
    layout: trp_data::screen::Layout,
    icon: Option<(trp_data::unity::extract::TextureData, Rect, trp_data::screen::Layout)>,
    image: Option<Handle<Image>>,
}

/// One panel as read.
struct PanelLook {
    panel: trp_data::screen::SightPanel,
    words: WordsLook,
    details: Option<DetailsLook>,
}

/// A panel drawn: its root, what it shows and the window's size it was laid
/// out for.
struct Drawn {
    root: Entity,
    shown: (String, Option<String>),
    size: [u32; 2],
}

/// The panels (0 the ammunition's, 1 the optic's, 2 the tactical devices'
/// activation mode's): as read, each one's fade, words and details line,
/// what is drawn, and whether the battle screen is up (the inventory shut,
/// the character alive).
#[derive(Resource)]
pub struct SightPanelsLook {
    looks: [Option<PanelLook>; 3],
    faders: [Fader; 3],
    shown: [(String, Option<String>); 3],
    drawn: [Option<Drawn>; 3],
    up: bool,
}

/// A drawn piece's own colour, before its panel's fade: a ground, words, or
/// a picture.
#[derive(Component, Clone, Copy)]
enum Own {
    Ground(Color),
    Words(Color),
    Picture(Color),
}

impl SightPanelsLook {
    /// Reads them from the scene that holds the inventory screen; none
    /// drawn where it cannot be read.
    pub fn read(install: &trp_data::Install, file: &str) -> Self {
        let looks = match trp_data::screen::SightPanels::load(install, file) {
            Ok(panels) => [Some(look_of(install, panels.ammo)), Some(look_of(install, panels.optic)), panels.tactical.map(|panel| look_of(install, panel))],
            Err(e) => {
                eprintln!("the battle screen's panels at the lower right could not be read; the zeroing, the magnification and the hands' words are not shown: {e:#}");
                [None, None, None]
            }
        };
        Self { looks, faders: [Fader::default(), Fader::default(), Fader::default()], shown: Default::default(), drawn: [None, None, None], up: false }
    }
}

fn look_of(install: &trp_data::Install, panel: trp_data::screen::SightPanel) -> PanelLook {
    let words = WordsLook::read(install, panel.text.clone(), panel.font_file.as_ref());
    let details = panel.details.clone().map(|details| {
        let icon = details.icon.as_ref().and_then(|(file, id, layout)| {
            let mut pictures = trp_data::screen::ScenePictures::new(install).ok()?;
            let cut = pictures.sprite(file, *id).ok()?;
            let texture = pictures.texture(&cut.texture.0, cut.texture.1).ok()?;
            let [x, y, w, h] = cut.rect;
            Some((texture, Rect::new(x, y, x + w, y + h), *layout))
        });
        DetailsLook { words: WordsLook::read(install, details.text.clone(), details.font_file.as_ref()), layout: details.layout, icon, image: None }
    });
    PanelLook { panel, words, details }
}

/// What the sights and the hands told the screen this frame, as its two
/// panels take it: a distance, a way of firing or a magazine's count shows
/// the ammunition's panel, each show setting its details line (none hides
/// it); a magnification shows the optic's, or hides it; each shown for a
/// while (`Show(autoHide: true)`). The screen shut, both go at once and
/// start hidden as it comes back (`Close`, `UpdatePanelsVisibility`).
fn follow_sights(
    mut controller: ResMut<Controller>,
    time: Res<Time>,
    carried: Option<Res<crate::inventory_view::Carried>>,
    (held, skills): (Option<Res<crate::inventory_view::HandsHeld>>, Option<Res<crate::skills::Skills>>),
    hands: Option<ResMut<crate::weapon::HandsRig>>,
    mut look: ResMut<SightPanelsLook>,
    tactical: Option<Res<crate::tactical_view::Tactical>>,
) {
    let told = hands.map(|mut hands| hands.hands.take_sight_hud()).unwrap_or_default();
    let calls = std::mem::take(&mut controller.panel_calls);
    let alive = controller.health.as_ref().is_none_or(|health| health.alive);
    let up = !controller.inventory_open && !controller.edit_build && alive;
    let look = &mut *look;
    if up != look.up {
        look.up = up;
        for fader in &mut look.faders {
            fader.close();
        }
    }
    if !up {
        return;
    }
    let say = |key: &str| carried.as_ref().and_then(|carried| carried.texts.say(key)).unwrap_or_else(|| key.to_owned());
    for message in told {
        debug!("the battle screen told {message:?}");
        match message {
            SightHud::Zeroing(distance) => {
                look.shown[0] = (distance.to_string(), None);
                look.faders[0].show(true);
            }
            SightHud::Magnification(zoom) => {
                look.shown[1] = (format!("x{zoom}"), None);
                look.faders[1].show(true);
            }
            SightHud::HideMagnification => look.faders[1].hide(0.0),
        }
    }
    for call in calls {
        debug!("the battle screen told {call:?}");
        look.shown[0] = match call {
            PanelCall::FireMode(mode) => (say(fire_mode_key(mode)), None),
            PanelCall::Magazine => {
                let drills = skills.as_ref().map_or(0, |skills| skills.level(trp_sim::skills::Skill::MagDrills));
                let Some(said) = carried.as_ref().and_then(|carried| magazine_words(carried, held.as_ref().and_then(|held| held.item), drills)) else { continue };
                said
            }
        };
        look.faders[0].show(true);
    }
    // T+B: the activation mode's words (`TacticalInteractionChanged`; the
    // locale's "TacticalInteractionMode/Press" or ".../Hold"), shown for a
    // while (`AnimatedTextPanel.Show`).
    if let Some(mode) = tactical.as_ref().and_then(|tactical| tactical.mode_switched) {
        look.shown[2] = (say(&format!("TacticalInteractionMode/{}", mode.word())), None);
        look.faders[2].show(true);
    }
    for fader in &mut look.faders {
        fader.advance(time.delta_secs());
    }
}

/// A node at a rectangle on the screen of 1080 units high, given from its
/// lower left corner up, in the window's pixels at the scale `s`.
fn placed(left: f32, bottom: f32, size: [f32; 2], s: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left * s),
        top: Val::Px((1080.0 - bottom - size[1]) * s),
        width: Val::Px(size[0] * s),
        height: Val::Px(size[1] * s),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// Where a child laid out by `layout` lies in its parent's rectangle
/// (`left`, `bottom`, `size`), as large as `size_of` (its anchors one point,
/// as these are): its lower left corner.
fn child_at(layout: &trp_data::screen::Layout, left: f32, bottom: f32, size: [f32; 2], own: [f32; 2]) -> [f32; 2] {
    let pivot = [left + layout.anchor_min[0] * size[0] + layout.offset[0], bottom + layout.anchor_min[1] * size[1] + layout.offset[1]];
    [pivot[0] - layout.pivot[0] * own[0], pivot[1] - layout.pivot[1] * own[1]]
}

/// Draws each panel while it is faded in at all: laid out on the screen of
/// 1080 units high (the battle screen's scaling by height), its box as wide
/// as its words and its row's padding and as high as a line of them, at its
/// anchor, place and pivot; the details line under the words where it has
/// one, and its icon before it; everything at its own colours times the
/// fade.
#[allow(clippy::too_many_arguments)]
fn draw_sight_panels(
    mut commands: Commands,
    mut look: ResMut<SightPanelsLook>,
    mut fonts: ResMut<Assets<Font>>,
    mut images: ResMut<Assets<Image>>,
    window: Query<&Window, With<PrimaryWindow>>,
    children: Query<&Children>,
    mut colours: Query<(&Own, Option<&mut BackgroundColor>, Option<&mut TextColor>, Option<&mut ImageNode>)>,
) {
    let Ok(window) = window.single() else { return };
    let size = [window.width().round() as u32, window.height().round() as u32];
    if size[1] == 0 {
        return;
    }
    let s = size[1] as f32 / 1080.0;
    let screen = [size[0] as f32 / s, 1080.0];
    let look = &mut *look;
    for i in 0..3 {
        let alpha = look.faders[i].alpha;
        let shown = look.shown[i].clone();
        let Some(panel) = look.looks[i].as_mut() else { continue };
        let current = look.drawn[i].as_ref().filter(|drawn| alpha > 0.0 && drawn.shown == shown && drawn.size == size);
        if current.is_none() {
            if let Some(old) = look.drawn[i].take() {
                commands.entity(old.root).despawn();
            }
            if alpha <= 0.0 || shown.0.is_empty() {
                continue;
            }
            let p = &panel.panel;
            let words_size = panel.words.size(&shown.0);
            let w = words_size[0] + p.padding[0] + p.padding[1];
            let h = words_size[1] + p.padding[2] + p.padding[3];
            let [left, bottom] = child_at(&p.layout, 0.0, 0.0, screen, [w, h]);
            let rgba = |c: [f32; 4]| Color::srgba(c[0], c[1], c[2], c[3]);
            let (ground, colour) = (rgba(p.ground), rgba(p.text.color));
            let font = TextFont { font: panel.words.font(&mut fonts), font_size: p.text.size * s, ..default() };
            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
            let mut root = commands.spawn((Name::new(if i == 0 { "Ammunition's panel" } else { "Optic's panel" }), whole, GlobalZIndex(5)));
            root.with_children(|root| {
                root.spawn((placed(left, bottom, [w, h], s), BackgroundColor(ground.with_alpha(0.0)), Own::Ground(ground))).with_children(|ground| {
                    ground.spawn((Text::new(shown.0.clone()), font, TextColor(colour.with_alpha(0.0)), TextLayout::new_with_justify(Justify::Center), Own::Words(colour)));
                });
                // The details line hangs from the words' rectangle (the row
                // less its padding), its icon from the line.
                let (Some(details), Some(said)) = (panel.details.as_mut(), shown.1.as_ref()) else { return };
                let words_left = left + p.padding[0];
                let words_bottom = bottom + p.padding[3];
                let own = details.words.size(said);
                let [dl, db] = child_at(&details.layout, words_left, words_bottom, words_size, own);
                let line_colour = rgba(details.words.text.color);
                let line_font = TextFont { font: details.words.font(&mut fonts), font_size: details.words.text.size * s, ..default() };
                let mut node = placed(dl, db, own, s);
                node.justify_content = JustifyContent::FlexStart;
                root.spawn((node, Own::Words(line_colour))).with_children(|line| {
                    line.spawn((Text::new(said.clone()), line_font, TextColor(line_colour.with_alpha(0.0)), Own::Words(line_colour)));
                });
                if let Some((texture, cut, layout)) = details.icon.as_ref() {
                    if details.image.is_none() {
                        details.image = crate::render_data::build_image(texture, crate::render_data::Role::Color).ok().map(|image| images.add(image));
                    }
                    if let Some(image) = details.image.clone() {
                        let icon = layout.size_delta;
                        let [il, ib] = child_at(layout, dl, db, own, icon);
                        root.spawn((placed(il, ib, icon, s), ImageNode { image, color: Color::WHITE.with_alpha(0.0), rect: Some(*cut), flip_y: true, ..default() }, Own::Picture(Color::WHITE)));
                    }
                }
            });
            let root = root.id();
            debug!("the panel {i} drawn: {shown:?}, {w:.1} by {h:.1} at ({left:.1}, {bottom:.1}) up from the foot");
            look.drawn[i] = Some(Drawn { root, shown, size });
        }
        let Some(drawn) = look.drawn[i].as_ref() else { continue };
        for entity in children.iter_descendants(drawn.root) {
            let Ok((own, ground, text, picture)) = colours.get_mut(entity) else { continue };
            let fade = |colour: Color| colour.with_alpha(colour.alpha() * alpha);
            match (*own, ground, text, picture) {
                (Own::Ground(colour), Some(mut ground), _, _) => {
                    if ground.0 != fade(colour) {
                        ground.0 = fade(colour);
                    }
                }
                (Own::Words(colour), _, Some(mut text), _) => {
                    if text.0 != fade(colour) {
                        text.0 = fade(colour);
                    }
                }
                (Own::Picture(colour), _, _, Some(mut picture)) => {
                    if picture.color != fade(colour) {
                        picture.color = fade(colour);
                    }
                }
                _ => {}
            }
        }
    }
}
