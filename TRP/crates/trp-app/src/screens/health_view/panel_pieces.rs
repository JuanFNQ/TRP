//! The panel's pieces, the drop's glow, the effects' icons and tooltips.

use super::*;

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

/// A fill's clip made `want` pixels wide (the screen's, as `Val::Px`), its
/// width written only when the layout would draw it otherwise. The layout
/// rounds a node's edges to whole physical pixels: its width drawn is
/// `round(x + w) - round(x)`, `x` its left edge before rounding, of which
/// only the share within a pixel counts. That share is not kept; every
/// share that gives the width last drawn from the width last laid out is
/// tried, and the width is written if any of them draws the new one
/// otherwise. A width written lays the whole interface out again (0.17 ms
/// of the profiled frame), and a stamina bar moves on every frame of a
/// sprint or an aim, its edge a pixel a few times a second. Drawn the same
/// (`docs/spec/inventory.md`, "When the interface is laid out").
pub(crate) fn set_fill_width(node: &mut Mut<Node>, computed: Option<&bevy::ui::ComputedNode>, want: f32) {
    let want_val = Val::Px(want);
    if node.width == want_val {
        return;
    }
    let skip = match (node.width, computed) {
        (Val::Px(now), Some(computed)) if computed.inverse_scale_factor > 0.0 => {
            let scale = 1.0 / computed.inverse_scale_factor;
            let (before, after, drawn) = (computed.unrounded_size.x, want * scale, computed.size.x);
            // Laid out from the width now written (not, say, under a hidden
            // node the layout leaves as it was).
            (before - now * scale).abs() <= 1e-3 && same_drawn(before, after, drawn)
        }
        _ => false,
    };
    if !skip {
        node.width = want_val;
    }
}

/// Whether every share within a pixel `f` that draws `before` as `drawn`
/// pixels draws `after` the same. The widths drawn change with `f` only
/// where `f`, `f + before` or `f + after` crosses a half: each such point,
/// a hair either side of it and the middle of each stretch between are
/// tried, which is every case.
pub(super) fn same_drawn(before: f32, after: f32, drawn: f32) -> bool {
    let width = |f: f32, w: f32| (f + w).round() - f.round();
    let mut points = vec![0.0f32, 0.5, (0.5 - before).rem_euclid(1.0), (0.5 - after).rem_euclid(1.0)];
    points.sort_by(f32::total_cmp);
    let mut tried = Vec::with_capacity(points.len() * 4);
    for (k, &p) in points.iter().enumerate() {
        let next = points.get(k + 1).copied().unwrap_or(1.0);
        tried.extend([p, (p - 1e-3).rem_euclid(1.0), (p + 1e-3).rem_euclid(1.0), (p + next) / 2.0]);
    }
    let mut feasible = false;
    for f in tried {
        if width(f, before) == drawn {
            feasible = true;
            if width(f, after) != drawn {
                return false;
            }
        }
    }
    feasible
}

#[derive(Component)]
pub(super) struct PieceText(pub(super) usize);

#[derive(Component)]
pub(super) struct PieceSpan(pub(super) usize);

/// A rate's arrow, which bobs while its figure moves: its top as laid out,
/// in pixels, and the pixels to a unit, less than nought where it lies
/// upside down (the falling figure's `Arrows/Down`, whose clip's up is the
/// screen's down: `courses10`'s `inventory-open-close`, the clip's −2 at
/// its start drawn at the top of its travel, 1179.34 of 1179.3 to 1184.7).
#[derive(Component)]
pub(super) struct Bob(pub(super) f32, pub(super) f32);

/// The effects on a part, by the part's place in `Part::ALL`, in words
/// where the game has its icons (`EffectsPanel`, whose icons are the hard
/// settings' table, not read: `docs/spec/health.md`, "The HEALTH tab").
#[derive(Component)]
pub(super) struct EffectWords(pub(super) usize);

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
/// takes the mouse: `SetUnlockStatus`, no raycast change); one whose menu
/// of lines is open stays lit with the mouse on that (`Blocked`).
/// Recoloured in place: a rebuild of the screen would lose the mouse for a
/// frame.
pub(super) fn light_info_buttons(
    controller: Res<Controller>,
    lights: Query<(&InfoButtonLight, &RelativeCursorPosition)>,
    covers: Query<(&crate::screens::inventory_view::Cover, &RelativeCursorPosition)>,
    mut pieces: Query<(Option<&Piece>, Option<&SliceOf>, &mut ImageNode)>,
    menu: Res<crate::screens::item_menu_view::ItemMenu>,
) {
    if lights.is_empty() {
        return;
    }
    let top = crate::screens::inventory_view::cover_under(covers.iter());
    let blocked = menu.0.as_ref().filter(|open| open.sub.is_some()).and_then(|open| open.button).and_then(|button| button.piece);
    let wanted: HashMap<usize, Color> = lights
        .iter()
        .map(|(light, at)| {
            let lit = controller.inventory_open && ((at.cursor_over() && light.layer == top) || blocked == Some(light.piece));
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

pub(super) fn light_drop_glow(
    look: Res<HealthLook>,
    controller: Res<Controller>,
    carried: Res<crate::screens::inventory_view::Carried>,
    game: Res<crate::GameData>,
    dragging: Res<crate::screens::inventory_view::Dragging>,
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
        let Some(medicine) = trp_sim::condition::health::Medicine::of(&game.db, &item.kind.template) else { return false };
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
pub(super) struct EffectIcons(usize, f32, Vec<(&'static str, bool)>);

/// One effect's icon (`EffectIcon`): what its tooltip says (the effect's
/// name in the install's words, and while it has under two hours left, its
/// seconds left: `BuffDescription.TimeLeftDisplay`; a left-over one's name
/// alone; a stimulators' icon's buffs, `stimulator_lines`), and whether it
/// blinks. Its picture's name tells the stimulators' two apart.
#[derive(Component)]
pub(super) struct EffectIcon {
    pub(super) part: usize,
    pub(super) kind: Kind,
    residual: bool,
    pub(super) name: &'static str,
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
        Kind::Berserk => "Berserk",
        Kind::Intoxication => "Intoxication",
        Kind::ImmunityPrevented => "ImmunityPreventedContamination",
        // (A stimulator's are two: `stimulator_icons`.)
        Kind::Stimulator | Kind::LowHealth | Kind::Existence | Kind::MedEffect | Kind::Flash | Kind::Endurance | Kind::DamageModifier => return None,
    })
}

/// The stimulators' icons shown where one is (`StimulatorHelper.
/// GetDisplayableVariations`: one for the buffs at work, `IStimulatorBuff`,
/// one for the side effects, `IStimulatorDebuff`, every stimulator's
/// merged into the first's; each hidden while it lists nothing,
/// `EffectIcon.BuffsUpdatedHandler`), in that order.
pub(crate) fn stimulator_icons(health: &trp_sim::condition::health::Health) -> Vec<&'static str> {
    [("StimulatorBuff", false), ("StimulatorDebuff", true)].into_iter().filter(|(_, side)| health.stimulator_listing(*side).iter().any(|list| !list.is_empty())).map(|(name, _)| name).collect()
}

/// What a stimulators' icon's tooltip says (`EffectIcon.VariationBuffs`):
/// its buffs' lines (`trp_sim::item_attributes::stimulator_lines`) in the
/// install's words, one under another, with the game's colour tags.
fn stimulator_words(health: &trp_sim::condition::health::Health, side: bool, said: &Words) -> String {
    let words = |key: &str| said.word(key, key);
    trp_sim::items::item_attributes::stimulator_lines(&health.stimulator_listing(side), &words).join("\n")
}

/// The cell an effect's icon takes in its panel's grid (`GridLayoutGroup`:
/// `m_CellSize`), in units.
const EFFECT_CELL: [f32; 2] = [24.0, 22.0];

/// Keeps each part's icons as its effects are (`EffectsPanel.UpdateEffects`:
/// the part's effects at work, then those left over, one icon of a kind),
/// and blinks those left over (`EffectIcon.Co_Blinking`: the alpha from 0.2
/// to 1 and back at one a second).
pub(super) fn fill_effect_icons(mut commands: Commands, controller: Res<Controller>, look: Res<HealthLook>, time: Res<Time<Real>>, mut panels: Query<(Entity, &mut EffectIcons)>, mut icons: Query<(&EffectIcon, &mut ImageNode)>) {
    let Some(health) = controller.health.as_ref() else { return };
    let stimulators = stimulator_icons(health);
    for (entity, mut panel) in &mut panels {
        let part = Part::ALL[panel.0];
        let mut wanted: Vec<(&'static str, bool, Kind)> = Vec::new();
        for residual in [false, true] {
            for effect in health.effects.iter().filter(|e| e.part == part && if residual { e.stage == trp_sim::condition::health::Stage::Residual } else { e.existing() }) {
                // A stimulator's two at the first's place; one left over
                // lists nothing (its buffs undone).
                let names: Vec<&'static str> = match effect.kind {
                    Kind::Stimulator if residual => Vec::new(),
                    Kind::Stimulator => stimulators.clone(),
                    kind => effect_icon(kind).into_iter().collect(),
                };
                for name in names {
                    if !wanted.iter().any(|(had, ..)| *had == name) {
                        wanted.push((name, residual, effect.kind));
                    }
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
                grid.spawn((EffectIcon { part: panel.0, kind, residual, name }, node, drawn, RelativeCursorPosition::default()));
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

/// Shows the tooltip of the effect's icon the mouse is on (`EffectIcon`'s
/// hover: `SimpleTooltip.Show`, at once), its words as the game writes
/// them, in the game's simple tooltip (`crate::tooltip`: its lower left 10
/// units up and to the right of the mouse, kept on the screen); and hides
/// it as the mouse leaves.
pub(super) fn show_effect_tooltip(
    controller: Res<Controller>,
    said: Res<Words>,
    icons: Query<(Entity, &EffectIcon, &RelativeCursorPosition, &InheritedVisibility)>,
    mut tooltip: ResMut<crate::ui::tooltip::SimpleTip>,
) {
    let over = icons.iter().find(|(_, _, at, shown)| shown.get() && at.cursor_over()).map(|(entity, icon, ..)| (entity, icon));
    let (Some((entity, icon)), Some(health)) = (over, controller.health.as_ref()) else { return };
    let part = Part::ALL[icon.part];
    let line = if icon.kind == Kind::Stimulator {
        stimulator_words(health, icon.name == "StimulatorDebuff", &said)
    } else {
        let effect = health.effects.iter().find(|e| e.part == part && e.kind == icon.kind && if icon.residual { e.stage == trp_sim::condition::health::Stage::Residual } else { e.existing() });
        let name = said.effect(icon.kind).unwrap_or_else(|| icon.kind.name().to_owned());
        let left = effect.map_or(0.0, |e| e.delay + e.work - e.whole).round();
        if !icon.residual && left > 0.0 && left < 7200.0 { format!("{name} ({left:.0})") } else { name }
    };
    tooltip.show(("effect", entity.to_bits()), line, 0.0);
}
