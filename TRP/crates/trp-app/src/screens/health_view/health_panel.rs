//! The HEALTH panel filled from the character's state.

use super::*;

/// Keeps the panel as the game's scripts keep it: each part's view on
/// every frame (`BodyPartView.Update`), the figures once a second and as
/// they are first drawn (`HealthParametersPanel.Update`, on the clock of
/// the world outside).
#[allow(clippy::type_complexity)]
pub(super) fn fill_panel(
    controller: Res<Controller>,
    look: Res<HealthLook>,
    (time, opening): (Res<Time<Real>>, Res<crate::screens::inventory_view::Opening>),
    carried: Res<crate::screens::inventory_view::Carried>,
    added: Query<(), Added<Piece>>,
    mut pieces: Query<(&Piece, &mut Visibility, Option<&mut ImageNode>, Option<&mut Node>, Option<&FullWidth>, Option<&Bob>, Option<&bevy::ui::ComputedNode>)>,
    mut fills: Query<(&PieceFill, &mut ImageNode), Without<Piece>>,
    mut slices: Query<(&SliceOf, &mut ImageNode), (Without<Piece>, Without<PieceFill>)>,
    mut texts: Query<(&PieceText, &mut Text, &mut TextColor, Has<EffectCopy>), (Without<PieceSpan>, Without<EffectWords>)>,
    mut spans: Query<(&PieceSpan, &mut TextSpan, &mut TextColor, Has<EffectCopy>), Without<PieceText>>,
    mut figures: Local<(f64, HashMap<usize, Want>)>,
    mut drawn_texts: Query<(&PieceText, &mut TmpText)>,
) {
    // Not while the inventory is shut: the panel and the strip are on its
    // page, which is drawn afresh as it opens (its pieces new, so filled
    // and their figures taken at once: `added`), and nothing of them shows
    // on the battle screen.
    if !controller.inventory_open && !controller.edit_build && added.is_empty() {
        return;
    }
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
        // skills' carrying factor (`HealthParametersPanel`), as the
        // inventory last told of it (`Carried::told_weight`).
        let weight = carried.told_weight;
        let mut rows: Vec<FigureRow> = figure_rows(health).to_vec();
        if let Some([_, upper]) = look.weight_limits {
            rows.push(("Weight", [weight, upper * carried.carry], 0.0, false, 1, None));
        }
        let mut panels = vec![(panel, views.parameters.as_slice(), 0)];
        if let Some(strip) = &game.strip {
            panels.push((strip, game.strip_figures.as_slice(), STRIP));
        }
        for ((name, [current, maximum], threshold, from_top, places, rate), (panel, list, base)) in rows.iter().flat_map(|row| panels.iter().map(move |at| (*row, *at))) {
            let Some(f) = trp_data::interface::screen::figure_named(list, name) else { continue };
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
    // The arrows of a moving figure bob as their clip has them, the clip
    // begun as the inventory's page is shown, a frame's time into it on
    // that frame (their animator made anew with the screen: `courses10`'s
    // `inventory-open-close`, the same first values, y 1179.34 and alpha
    // 0.5005, at all four openings); a still one's stand whole where they
    // are laid out.
    let played = opening.page_shown(now).unwrap_or(now);
    let bob = look.bob.as_ref().map(|(clip, height, alpha)| {
        let mut values = Vec::new();
        clip.sample((played % clip.duration().max(1e-3) as f64) as f32, &mut values);
        (values.get(*height).copied().unwrap_or(0.0), values.get(*alpha).copied().unwrap_or(1.0))
    });
    for w in wants.values_mut() {
        if let (Some(true), Some((height, alpha))) = (w.moving, bob) {
            w.raised = height;
            w.color = w.color.map(|color| color.with_alpha(alpha));
        }
    }
    for (piece, mut visibility, image, node, full, bob, laid) in &mut pieces {
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
            set_fill_width(&mut node, laid, full.0 * fill);
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
