//! The asking window's state and how a press on it is handled.

use super::*;

/// What the window that asks how many is asking for: a stack split into a
/// place or put onto another, or a share of food or drink to be used
/// (`ItemUiContext.ShowSplitMenuDialog`), of `whole`, its most.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Asked {
    Move(Target),
    Use { whole: u32 },
}

/// The window that asks how many of a stack are to go (`SplitDialog`):
/// which stack, where to, how many at the most and how many now. For a
/// use, how much of what is left, which is `most`.
#[derive(Clone, Copy, PartialEq)]
pub struct Asking {
    pub(super) id: Id,
    pub(super) asked: Asked,
    /// The install's word it is headed by: a split into a place
    /// (`Split`), a transfer to a stack (`Transfer`), or a use (`Use`, in
    /// capitals).
    pub(super) title: &'static str,
    pub(super) most: u32,
    pub(super) count: u32,
    /// For a stack, the stepped slider's value where its knob stands (1 to
    /// the number of its stops): the stop slid to, or, for a figure the
    /// window came up at or that was typed, the place in the stops of the
    /// one nearest it, from nought (`StepSlider.GetIndexOfClosest` given to
    /// the slider, which counts from one: a stop short of the figure's
    /// own), and the last for the most.
    pub(super) knob: u32,
    /// Where its middle is asked to be, in the window's pixels: at the
    /// mouse as it came up (`_window.position`), then where its caption
    /// has moved it (`UIDragComponent`).
    pub(super) at: Vec2,
    /// Whether its field has the keys (taken with the window, given up to
    /// a press on anything else, given back by a press on it), and whether
    /// all it says is selected, so that the first key typed takes its
    /// place (`m_OnFocusSelectAll`). With the app's own look the field
    /// keeps the keys.
    pub(super) focused: bool,
    pub(super) selected: bool,
    /// The whole-number slider's field emptied: it says nothing, and the
    /// figure stands (`IntSlider.TextValueChanged` passes over words it
    /// cannot read).
    blank: bool,
    /// The slider chosen by a press on it (tinted so) until a press on
    /// something else (`EventSystem.SetSelectedGameObject`).
    slider_chosen: bool,
}

impl Asking {
    /// The window for part of a stack, of `most` at the most, headed as a
    /// transfer onto a stack or a split into a place, coming up at half and
    /// one (`SplitDialog.Show`), its knob where the stepped slider puts
    /// that, its field with the keys and all of it selected.
    pub(super) fn of_stack(id: Id, target: Target, onto: bool, most: u32, at: Vec2) -> Self {
        let count = most / 2 + 1;
        let title = if onto { "Transfer" } else { "Split" };
        Self { id, asked: Asked::Move(target), title, most, count, knob: Self::knob_of(most, count), at, focused: true, selected: true, blank: false, slider_chosen: false }
    }

    /// Its caption in the install's words (`"Split".Localized()`;
    /// `"Use".Localized().ToUpper()`), else the app's.
    pub(super) fn caption(&self, said: &crate::screens::health_view::Words) -> String {
        let words = said.word(self.title, &self.title.to_uppercase());
        if self.title == "Use" {
            words.to_uppercase()
        } else {
            words
        }
    }

    /// What its field says: the figure, or nothing where emptied.
    pub(super) fn field(&self) -> String {
        if self.blank {
            String::new()
        } else {
            self.count.to_string()
        }
    }

    /// The stepped slider's value for a figure set from outside the
    /// slider (as it comes up, or typed): the place of the stop nearest it
    /// in the stops from nought, the later of two as near, as the slider's
    /// value from one, so no lower than one; past the stops' end for the
    /// most (`StepSlider.GetIndexOfClosest`, `SetCurrentValue`).
    pub(super) fn knob_of(most: u32, count: u32) -> u32 {
        let stops = Self::stops(most);
        if count >= most {
            return stops.len() as u32;
        }
        let nearest = stops.iter().copied().reduce(|x, y| if x.abs_diff(count) >= y.abs_diff(count) { y } else { x }).unwrap_or(1);
        let at = stops.iter().position(|stop| *stop == nearest).unwrap_or(0) as u32;
        at.clamp(1, stops.len() as u32)
    }

    /// For each notch the slider's script copies into its row (every place
    /// but the last, which the row's own last notch marks), whether the
    /// knob may stand there: all of a stack's (`StepSlider.CreateNotch`);
    /// for food or drink, not those before the knob's first place
    /// (`IntSlider.Show`: not available where `minValue - 1 > i`).
    pub(super) fn notches(&self) -> Vec<bool> {
        match self.asked {
            Asked::Move(_) => vec![true; Self::stops(self.most).len().saturating_sub(1)],
            Asked::Use { whole } => {
                let first = whole + 1 - self.most;
                (0..whole.saturating_sub(1)).map(|i| first <= i + 1).collect()
            }
        }
    }

    /// A figure typed into its field (`StepSlider`'s and `IntSlider`'s
    /// `onValueChanged`): the digit after what it says, or in place of it
    /// where all is selected; kept to 1 and the most (the field then says
    /// that); the stepped slider's knob put at the nearest stop again.
    pub(super) fn typed(&mut self, digit: u32) {
        let so_far = if self.selected || self.blank { 0 } else { self.count };
        self.count = (so_far.saturating_mul(10).saturating_add(digit)).clamp(1, self.most);
        (self.selected, self.blank) = (false, false);
        if matches!(self.asked, Asked::Move(_)) {
            self.knob = Self::knob_of(self.most, self.count);
        }
    }

    /// Backspace in its field: all selected, or its last figure, taken
    /// away. The stepped slider's field emptied reads as its least, 1,
    /// and says so where that changes the figure, else says nothing; the
    /// whole-number one's says nothing, its figure as it was.
    pub(super) fn rubbed_out(&mut self) {
        let left = if self.selected || self.blank { 0 } else { self.count / 10 };
        self.selected = false;
        match (left, self.asked) {
            (0, Asked::Use { .. }) => self.blank = true,
            (0, Asked::Move(_)) if self.count == 1 => self.blank = true,
            (0, Asked::Move(_)) => (self.count, self.knob) = (1, Self::knob_of(self.most, 1)),
            (left, _) => {
                self.count = left.clamp(1, self.most);
                if matches!(self.asked, Asked::Move(_)) {
                    self.knob = Self::knob_of(self.most, self.count);
                }
            }
        }
    }
    /// The values the slider stops at (`StepSlider.Show`): a notch every
    /// twenty-fourth of the most, rounded up, the first notch at one, and
    /// the most itself at the end.
    pub(super) fn stops(most: u32) -> Vec<u32> {
        let notch = most.div_ceil(24).max(1);
        let notches = if most / notch < 24 { most.div_ceil(notch) } else { 24 };
        let first = if notch == 1 { 1 } else { 0 };
        (first..notches).map(|at| (notch * at).max(1)).chain([most]).collect()
    }

    /// The window that asks how much of food or drink to use
    /// (`ShowSplitMenuDialog`, an `IntSlider`): its track runs from 1 to
    /// the whole, what is already gone lies before the first place the
    /// knob may stand (`(int)(whole - left + 1)`), and the figure is how
    /// much is used, from 1 to what is left: all of that to begin with.
    pub(super) fn of_use(id: Id, whole: f32, left: f32, at: Vec2) -> Option<Self> {
        let whole_units = whole as u32;
        let first = ((whole - left + 1.0) as u32).max(1);
        let most = (whole_units + 1).checked_sub(first).filter(|most| *most >= 1)?;
        Some(Self { id, asked: Asked::Use { whole: whole_units }, title: "Use", most, count: most, knob: 0, at, focused: true, selected: true, blank: false, slider_chosen: false })
    }

    /// For a use, the knob's place on the track as a share of it (the
    /// track running from 1 to the whole), and what is gone before the
    /// knob's first place, the same way; for a split, the stop reached.
    pub(super) fn shares(&self) -> (f32, f32) {
        match self.asked {
            Asked::Use { whole } => {
                let span = whole.saturating_sub(1).max(1) as f32;
                let first = whole + 1 - self.most;
                let knob = first + self.count.clamp(1, self.most) - 1;
                ((knob - 1) as f32 / span, (first - 1) as f32 / span)
            }
            Asked::Move(_) => {
                let stops = Self::stops(self.most).len() as u32;
                (if stops > 1 { (self.knob.clamp(1, stops) - 1) as f32 / (stops - 1) as f32 } else { 1.0 }, 0.0)
            }
        }
    }

    /// The slider held at a share of its track (`Slider.normalizedValue`:
    /// its value from its least to its most, a whole number, half to even):
    /// for a stack, the stop at that value and the knob there, unless the
    /// knob stands at that value already, which changes nothing, the figure
    /// a stop off it kept (`Slider.Set` passes over the value it has, so the
    /// stepped slider is not told: `courses10`'s `how-many`, "30" kept from
    /// the press on the handle until 11 px of travel gave 24); for food or
    /// drink, the figure, the knob kept from its first place
    /// (`IntSlider.UpdateValue`).
    pub(super) fn slide_to(&mut self, share: f32) {
        let share = share.clamp(0.0, 1.0);
        match self.asked {
            Asked::Use { whole } => {
                let first = whole + 1 - self.most;
                let knob = (1.0 + share * whole.saturating_sub(1) as f32).round_ties_even() as u32;
                self.count = knob.clamp(first, whole) + 1 - first;
            }
            Asked::Move(_) => {
                let stops = Self::stops(self.most);
                let value = ((1.0 + share * (stops.len() - 1) as f32).round_ties_even() as usize).clamp(1, stops.len());
                if value as u32 == self.knob {
                    return;
                }
                (self.count, self.knob) = (stops[value - 1], value as u32);
            }
        }
        (self.selected, self.blank) = (false, false);
    }

    /// The figure for the slider held at a share of its track.
    #[cfg(test)]
    pub(super) fn slid_to(&self, share: f32) -> u32 {
        let mut slid = *self;
        slid.slide_to(share);
        slid.count
    }
}

/// The window that asks how many of a stack (`SplitDialog`): the slider
/// is slid with the mouse, a figure may be typed, Enter or its button
/// goes through with it (`ItemUiContext.TranslateCommand`), and Escape or
/// its cross drops it.
///
/// Drawn as the game's (`TARKOV UI`), a press goes to what lies under it
/// in the window (`AskPlaces`; its ground over the rest of the screen
/// takes any other): the slider's track or handle hold the slider, the
/// knob jumping to the mouse from the track, kept where the handle was
/// held from the handle (`Slider.OnPointerDown`, `UpdateDrag`; no drag
/// threshold, `OnInitializePotentialDrag`); the cross and the button act
/// when let go over them as they were pressed (`Button.OnPointerClick`);
/// the caption bar moves the window once the mouse has gone the drag
/// threshold from the press, held where it was then
/// (`UIDragComponent`). The field has the keys from the start with all it
/// says selected; a press anywhere but on it takes them, and a press on
/// it gives them back with all selected (`TMP_InputField.OnPointerDown`,
/// `m_OnFocusSelectAll`); figures typed while it lacks them go nowhere.
#[allow(clippy::too_many_arguments)]
pub(super) fn ask(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut clicks: ResMut<Clicks>,
    bar: Query<&RelativeCursorPosition, With<AskBar>>,
    accept: Query<&Interaction, (With<AskAccept>, Changed<Interaction>)>,
    close: Query<&Interaction, (With<AskClose>, Changed<Interaction>)>,
    mut wanted: ResMut<AskedFor>,
    (window, places): (Query<&Window, With<PrimaryWindow>>, Query<&AskPlaces>),
) {
    let Some(mut asking) = carried.asking else {
        (clicks.sliding, clicks.ask_press) = (false, None);
        return;
    };
    let pressed = |interaction: &Interaction| *interaction == Interaction::Pressed;
    let left = MouseButton::Left;
    let screen = window.single().ok();
    let cursor = screen.and_then(Window::cursor_position);
    let game = places.iter().next().map(|places| places.0);
    let inside = |[x, y, w, h]: [f32; 4]| cursor.is_some_and(|c| c.x >= x && c.x < x + w && c.y >= y && c.y < y + h);
    let before = asking;
    // The press and what it went to; the cross and the button.
    let (mut accepted, mut dropped) = (false, false);
    match game {
        None => {
            (accepted, dropped) = (accept.iter().any(pressed), close.iter().any(pressed));
            // The slider: held, it goes to the stop nearest the mouse.
            let over = bar.iter().next();
            if buttons.just_pressed(left) && over.is_some_and(RelativeCursorPosition::cursor_over) {
                clicks.sliding = true;
            }
            if !buttons.pressed(left) {
                clicks.sliding = false;
            }
            if let (true, Some(at)) = (clicks.sliding, over.and_then(share_of)) {
                asking.slide_to(at.x);
            }
        }
        Some(at) => {
            if let (true, Some(cursor)) = (buttons.just_pressed(left), cursor) {
                let press = if inside(at.close) {
                    Some(AskPress::Close)
                } else if inside(at.accept) {
                    Some(AskPress::Accept)
                } else if inside(at.handle) || inside(at.track) {
                    let offset = if inside(at.handle) { cursor.x - (at.handle[0] + at.handle[2] / 2.0) } else { 0.0 };
                    if !inside(at.handle) {
                        asking.slide_to((cursor.x - at.slide[0]) / at.slide[2].max(1e-3));
                    }
                    Some(AskPress::Slider(offset))
                } else if inside(at.caption) {
                    Some(AskPress::Caption(cursor, None))
                } else {
                    None
                };
                asking.slider_chosen = matches!(press, Some(AskPress::Slider(_)));
                if inside(at.field) {
                    // Given the keys with all selected; with them, the
                    // caret put at the press (`PROVISIONAL`: at the end).
                    asking.selected = !asking.focused;
                    asking.focused = true;
                } else {
                    asking.focused = false;
                }
                (clicks.ask_press, clicks.sliding) = (press, matches!(press, Some(AskPress::Slider(_))));
            }
            if buttons.just_released(left) {
                match clicks.ask_press {
                    Some(AskPress::Close) => dropped = inside(at.close),
                    Some(AskPress::Accept) => accepted = inside(at.accept),
                    _ => {}
                }
                (clicks.ask_press, clicks.sliding) = (None, false);
            }
            if !buttons.pressed(left) && !buttons.just_released(left) {
                (clicks.ask_press, clicks.sliding) = (None, false);
            }
            match (clicks.ask_press, cursor) {
                (Some(AskPress::Slider(offset)), Some(cursor)) if buttons.pressed(left) => {
                    asking.slide_to((cursor.x - offset - at.slide[0]) / at.slide[2].max(1e-3));
                }
                // The caption held: past the drag threshold, the window held
                // where it was then, and moved with the mouse.
                (Some(AskPress::Caption(from, held)), Some(cursor)) if buttons.pressed(left) => {
                    let held = held.or_else(|| {
                        (cursor.distance(from) >= DRAG_THRESHOLD).then(|| {
                            cursor - Vec2::new(at.window[0] + at.window[2] / 2.0, at.window[1] + at.window[3] / 2.0)
                        })
                    });
                    if let Some(delta) = held {
                        asking.at = cursor - delta;
                        clicks.ask_press = Some(AskPress::Caption(from, Some(delta)));
                    }
                }
                _ => {}
            }
        }
    }
    // Escape drops it, unless a window over the screen is open: that has
    // the key first, and this one the press after.
    let escape = keys.just_pressed(KeyCode::Escape) && carried.windows.is_empty();
    if !controller.inventory_open || escape || dropped {
        (carried.asking, clicks.sliding, clicks.ask_press) = (None, false, None);
        clicks.escape_taken |= escape && controller.inventory_open;
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) || accepted {
        let carried = &mut *carried;
        let name = carried.find(asking.id).map(|item| carried.name(item)).unwrap_or_default();
        let count = asking.count.clamp(1, asking.most);
        match asking.asked {
            // Food or drink: that share of its whole, to the body whole
            // (`ApplyItem(foodDrink, Common, amount / max)`).
            Asked::Use { whole } => {
                wanted.1 = Some((asking.id, None, Some(count as f32 / whole.max(1) as f32)));
                carried.note = format!("{name}: {count} of {whole} to be used");
            }
            // Silent: its sound was the drop's, as the window opened
            // (`SplitResult.ExecuteWithNewCount` plays none).
            Asked::Move(target) => {
                let done = act(&mut [&mut carried.equipment.item, &mut carried.loot], asking.id, target, &carried.rules, Some(count));
                carried.note = match done {
                    Ok(what) => format!("{name}: {count} {what}"),
                    Err(refusal) => format!("{name}: {}", why(&refusal, &carried.texts)),
                };
                carried.tidy();
            }
        }
        (carried.asking, clicks.sliding, clicks.ask_press) = (None, false, None);
        return;
    }
    // A figure typed, while the field has the keys (always, with the
    // app's look).
    const DIGITS: [[KeyCode; 2]; 10] = [
        [KeyCode::Digit0, KeyCode::Numpad0],
        [KeyCode::Digit1, KeyCode::Numpad1],
        [KeyCode::Digit2, KeyCode::Numpad2],
        [KeyCode::Digit3, KeyCode::Numpad3],
        [KeyCode::Digit4, KeyCode::Numpad4],
        [KeyCode::Digit5, KeyCode::Numpad5],
        [KeyCode::Digit6, KeyCode::Numpad6],
        [KeyCode::Digit7, KeyCode::Numpad7],
        [KeyCode::Digit8, KeyCode::Numpad8],
        [KeyCode::Digit9, KeyCode::Numpad9],
    ];
    if asking.focused || game.is_none() {
        for (digit, codes) in DIGITS.iter().enumerate() {
            if codes.iter().any(|code| keys.just_pressed(*code)) {
                asking.typed(digit as u32);
            }
        }
        if keys.just_pressed(KeyCode::Backspace) {
            asking.rubbed_out();
        }
    }
    if asking != before {
        carried.asking = Some(asking);
    }
}

/// The window that asks how many as the game draws it: its cross, its
/// button and its slider's handle tinted by their states
/// (`split_view::tint_controls`): held, then the slider chosen, then under
/// the mouse (for the slider, its track or its handle: what of it takes
/// the mouse).
pub(super) fn tint_asking(
    time: Res<Time<Real>>,
    carried: Res<Carried>,
    clicks: Res<Clicks>,
    window: Query<&Window, With<PrimaryWindow>>,
    roots: Query<(&AskPlaces, &crate::screens::split_view::SplitTints)>,
    mut pieces: Query<(&crate::screens::health_view::Piece, &mut ImageNode)>,
    mut fades: Local<HashMap<crate::screens::split_view::SplitControl, crate::screens::split_view::Fading>>,
) {
    use crate::screens::split_view::{SplitControl, SplitState};
    let (Some(asking), Ok((places, tints))) = (carried.asking, roots.single()) else {
        fades.clear();
        return;
    };
    let cursor = window.single().ok().and_then(Window::cursor_position);
    let inside = |[x, y, w, h]: [f32; 4]| cursor.is_some_and(|c| c.x >= x && c.x < x + w && c.y >= y && c.y < y + h);
    let at = places.0;
    let state = |held: bool, chosen: bool, over: bool| match (held, chosen, over) {
        (true, ..) => SplitState::Pressed,
        (_, true, _) => SplitState::Selected,
        (_, _, true) => SplitState::Highlighted,
        _ => SplitState::Normal,
    };
    let states = HashMap::from([
        (SplitControl::Close, state(clicks.ask_press == Some(AskPress::Close), false, inside(at.close))),
        (SplitControl::Accept, state(clicks.ask_press == Some(AskPress::Accept), false, inside(at.accept))),
        (SplitControl::Slider, state(clicks.sliding, asking.slider_chosen, inside(at.track) || inside(at.handle))),
    ]);
    crate::screens::split_view::tint_controls(tints, &states, &mut fades, time.delta_secs(), &mut pieces);
}
