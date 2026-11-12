//! A text field as the game's are: what it says, its caret and its selection, and what each key and press does to them.
//!
//! The game's fields are TextMeshPro's (`TMP_InputField`, under the game's
//! `ValidationInputField`); the rules are that component's as Unity
//! publishes it, by the inventory spec, "A text field". One line of text,
//! no rich text. The MASTERING page's search field and the tag window's
//! field are this one; each brings its own figures from the install
//! (`screen::input_field`) and its own check of a letter.

mod placed;

pub(crate) use placed::*;

/// How long after a press a second one selects the word under the mouse,
/// in seconds of the real clock (`TMP_InputField.m_DoubleClickDelay`, the
/// component's own figure, not kept in a scene).
const DOUBLE_PRESS: f32 = 0.5;

/// What ends a word for Ctrl with the arrows (`TMP_InputField.
/// kSeparators`).
const SEPARATORS: [char; 6] = [' ', '.', ',', '\t', '\r', '\n'];

/// A key as the field hears it. Home, the up arrow and Page Up all go to
/// the start of a field of one line, End, the down arrow and Page Down to
/// its end.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FieldKey {
    Left,
    Right,
    Start,
    End,
    Backspace,
    Delete,
    SelectAll,
    Enter,
    Typed(String),
}

/// What is held with a key or a press.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Mods {
    pub(crate) shift: bool,
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
}

/// A field's own rules: the most letters it takes (`m_CharacterLimit`; any
/// number where 0), whether the game's check of what it says keeps a
/// letter (`ValidationInputField`), and whether it selects all it says as
/// it takes the keys (`m_OnFocusSelectAll`).
#[derive(Clone, Copy)]
pub(crate) struct Rules {
    pub(crate) limit: usize,
    pub(crate) keeps: fn(char) -> bool,
    pub(crate) select_all: bool,
}

/// Where a press or a drag lies in the field's words: the gap between two
/// letters it goes to (counted in letters from the start), the letter
/// nearest it (as many as the letters where that is the end), and the word
/// it is on, from the gap before the word to the gap after.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Spot {
    pub(crate) gap: usize,
    pub(crate) letter: usize,
    pub(crate) word: Option<(usize, usize)>,
}

/// A text field: what it says; its caret, a gap between letters counted in
/// letters from the start (`stringPosition`); the selection's other end,
/// the one the keys and the mouse move (`stringSelectPosition`), the
/// caret's own gap with nothing selected; whether it has the keys; when
/// its caret's blink last began, on the real clock (`m_BlinkStartTime`);
/// when the left button last went down on it; and where that press was, in
/// the screen's pixels, with whether the mouse has since gone far enough
/// to drag, while the button is held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextField {
    pub(crate) text: String,
    pub(crate) caret: usize,
    pub(crate) select: usize,
    pub(crate) focused: bool,
    pub(crate) blink_since: Option<f32>,
    pressed_at: Option<f32>,
    held: Option<([f32; 2], bool)>,
}

impl TextField {
    /// A field that says that, without the keys.
    pub(crate) fn saying(text: String) -> Self {
        Self { text, ..Self::default() }
    }

    /// Whether the left button is held from a press on it.
    pub(crate) fn is_held(&self) -> bool {
        self.held.is_some()
    }

    /// How many letters it says.
    pub(crate) fn letters(&self) -> usize {
        self.text.chars().count()
    }

    /// What is selected, from its first gap to its last; none with the
    /// two ends at one gap (`hasSelection`).
    pub(crate) fn selection(&self) -> Option<(usize, usize)> {
        (self.caret != self.select).then(|| (self.caret.min(self.select), self.caret.max(self.select)))
    }

    /// Where a gap lies in the text's bytes.
    fn byte(&self, gap: usize) -> usize {
        self.text.char_indices().nth(gap).map_or(self.text.len(), |(at, _)| at)
    }

    /// Takes away what is selected, the caret left where it began
    /// (`Delete`). Whether anything was.
    fn cut(&mut self) -> bool {
        let Some((from, to)) = self.selection() else { return false };
        self.text.replace_range(self.byte(from)..self.byte(to), "");
        (self.caret, self.select) = (from, from);
        true
    }

    /// A letter typed (`Append`, `Insert`): what is selected goes first,
    /// then the letter goes in at the caret unless the field is full, the
    /// caret after it. The game's own check comes after the field's
    /// (`ValidateEnteredString`, on the field's change): a letter it does
    /// not keep is taken out again, so what was selected is gone all the
    /// same and the caret stands a letter further on, the end at most.
    /// `PROVISIONAL`: that last is read from the two classes together and
    /// was not seen in the game.
    fn insert(&mut self, letter: char, rules: &Rules) {
        self.cut();
        if rules.limit > 0 && self.letters() >= rules.limit {
            return;
        }
        if (rules.keeps)(letter) {
            self.text.insert(self.byte(self.caret), letter);
            self.caret += 1;
        } else {
            self.caret = (self.caret + 1).min(self.letters());
        }
        self.select = self.caret;
    }

    /// Where Ctrl with the left arrow goes from a gap (`FindPrevWordBegin`):
    /// after the last separator two letters back or more, else the start.
    fn word_before(&self, gap: usize) -> usize {
        let letters: Vec<char> = self.text.chars().collect();
        let Some(last) = gap.checked_sub(2) else { return 0 };
        (0..=last.min(letters.len().saturating_sub(1))).rev().find(|&at| letters.get(at).is_some_and(|letter| SEPARATORS.contains(letter))).map_or(0, |at| at + 1)
    }

    /// Where Ctrl with the right arrow goes from a gap
    /// (`FindNextWordBegin`): after the first separator a letter on or
    /// more, else the end.
    fn word_after(&self, gap: usize) -> usize {
        let letters: Vec<char> = self.text.chars().collect();
        (gap + 1..letters.len()).find(|&at| SEPARATORS.contains(&letters[at])).map_or(letters.len(), |at| at + 1)
    }

    /// The moving end put at a gap: alone with Shift held, the selection
    /// then running from the caret to it; else the caret with it.
    fn move_to(&mut self, gap: usize, shift: bool) {
        self.select = gap;
        if !shift {
            self.caret = gap;
        }
    }

    /// A key pressed while it has the keys (`KeyPressed`), at `now` on the
    /// real clock. The arrows step the moving end a letter, or with Ctrl a
    /// word; with something selected and no Shift they only put the caret
    /// at that end of it (`MoveLeft`, `MoveRight`). Backspace and Delete
    /// take what is selected, else the letter before or after the caret.
    /// Ctrl with A alone selects all (`SelectAll`: the caret at the end,
    /// the moving end at the start). Enter lets the keys go. After any
    /// other key the caret's blink begins again where the field says
    /// anything (`UpdateLabel`, `SetCaretVisible`).
    pub(crate) fn key(&mut self, key: &FieldKey, mods: Mods, rules: &Rules, now: f32) {
        let letters = self.letters();
        match key {
            FieldKey::Left => match (self.selection(), mods.shift) {
                (Some((from, _)), false) => (self.caret, self.select) = (from, from),
                _ => self.move_to(if mods.ctrl { self.word_before(self.select) } else { self.select.saturating_sub(1) }, mods.shift),
            },
            FieldKey::Right => match (self.selection(), mods.shift) {
                (Some((_, to)), false) => (self.caret, self.select) = (to, to),
                _ => self.move_to(if mods.ctrl { self.word_after(self.select) } else { (self.select + 1).min(letters) }, mods.shift),
            },
            FieldKey::Start => self.move_to(0, mods.shift),
            FieldKey::End => self.move_to(letters, mods.shift),
            FieldKey::Backspace => {
                if !self.cut() && self.caret > 0 {
                    self.text.remove(self.byte(self.caret - 1));
                    self.caret -= 1;
                    self.select = self.caret;
                }
            }
            FieldKey::Delete => {
                if !self.cut() && self.caret < letters {
                    self.text.remove(self.byte(self.caret));
                }
            }
            FieldKey::SelectAll => (self.caret, self.select) = (letters, 0),
            FieldKey::Enter => {
                self.blur();
                return;
            }
            // (A field of one line takes no tab or line's end, and no
            // field a control letter: `KeyPressed`, `IsValidChar`.)
            FieldKey::Typed(typed) => typed.chars().filter(|letter| !letter.is_control()).for_each(|letter| self.insert(letter, rules)),
        }
        if !self.text.is_empty() {
            self.blink_since = Some(now);
        }
    }

    /// The left button pressed on it (`OnPointerDown`), at a place of the
    /// screen's pixels and at `now` on the real clock. Without the keys it
    /// takes them, the blink begun, and where it selects all as it does
    /// the press puts no caret (`ActivateInputFieldInternal`, `OnFocus`).
    /// With them, or where it does not select all, the caret goes to the
    /// press's gap, or with Shift held the moving end alone; and a press
    /// within `DOUBLE_PRESS` of the one before (the one that gave it the
    /// keys counted) selects the word it is on, else the nearest letter.
    pub(crate) fn press(&mut self, spot: Spot, at: [f32; 2], mods: Mods, rules: &Rules, now: f32) {
        let had = self.focused;
        let second = self.pressed_at.is_some_and(|before| before + DOUBLE_PRESS > now);
        (self.pressed_at, self.held) = (Some(now), Some((at, false)));
        if had || !rules.select_all {
            self.move_to(spot.gap, mods.shift);
            if second {
                let letters = self.letters();
                (self.caret, self.select) = spot.word.unwrap_or((spot.letter.min(letters), (spot.letter + 1).min(letters)));
            }
        }
        if !had {
            self.focused = true;
            if rules.select_all {
                (self.caret, self.select) = (self.letters(), 0);
            }
        }
        if !had || !self.text.is_empty() {
            self.blink_since = Some(now);
        }
    }

    /// The mouse moved with the left button held from a press on it
    /// (`OnDrag`): once it has gone `threshold` of the screen's pixels from
    /// the press (the event system's), the moving end follows it, the
    /// caret staying where it was.
    pub(crate) fn drag(&mut self, spot: Spot, at: [f32; 2], threshold: f32) {
        let Some((from, begun)) = self.held.filter(|_| self.focused) else { return };
        if begun || (at[0] - from[0]).hypot(at[1] - from[1]) >= threshold {
            (self.held, self.select) = (Some((from, true)), spot.gap);
        }
    }

    /// The left button let go.
    pub(crate) fn release(&mut self) {
        self.held = None;
    }

    /// The keys let go (`DeactivateInputField`): Enter, a press elsewhere,
    /// its page or its window gone. Neither its caret nor its selection is
    /// drawn then.
    pub(crate) fn blur(&mut self) {
        (self.focused, self.held) = (false, None);
    }
}

/// The words of a line as TextMeshPro counts them, each from its first
/// letter to its last: letters, figures and hyphens together, an
/// apostrophe between two letters or figures with them.
pub(crate) fn words(text: &str) -> Vec<(usize, usize)> {
    let letters: Vec<char> = text.chars().collect();
    let of_word = |at: usize| {
        let letter = letters[at];
        let joins = |at: Option<usize>| at.and_then(|at| letters.get(at)).is_some_and(|letter| letter.is_alphanumeric());
        letter.is_alphanumeric() || ['-', '\u{ad}', '\u{2010}', '\u{2011}'].contains(&letter) || (['\'', '\u{2019}'].contains(&letter) && joins(at.checked_sub(1)) && joins(Some(at + 1)))
    };
    let mut found: Vec<(usize, usize)> = Vec::new();
    for at in (0..letters.len()).filter(|&at| of_word(at)) {
        match found.last_mut() {
            Some((_, last)) if *last + 1 == at => *last = at,
            _ => found.push((at, at)),
        }
    }
    found
}

/// What a key pressed means to a field, with what is held: none for a key
/// it does nothing with.
pub(crate) fn field_key(key: &bevy::input::keyboard::KeyboardInput, mods: Mods) -> Option<FieldKey> {
    use bevy::input::keyboard::{Key, KeyCode};
    Some(match &key.logical_key {
        Key::ArrowLeft => FieldKey::Left,
        Key::ArrowRight => FieldKey::Right,
        Key::Home | Key::ArrowUp | Key::PageUp => FieldKey::Start,
        Key::End | Key::ArrowDown | Key::PageDown => FieldKey::End,
        Key::Backspace => FieldKey::Backspace,
        Key::Delete => FieldKey::Delete,
        Key::Enter => FieldKey::Enter,
        _ if key.key_code == KeyCode::KeyA && mods.ctrl && !mods.shift && !mods.alt => FieldKey::SelectAll,
        _ => FieldKey::Typed(key.text.as_deref().filter(|typed| typed.chars().any(|letter| !letter.is_control()))?.to_owned()),
    })
}

/// What is held now, for a key or a press.
pub(crate) fn mods(keys: &bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>) -> Mods {
    use bevy::input::keyboard::KeyCode::{AltLeft, AltRight, ControlLeft, ControlRight, ShiftLeft, ShiftRight};
    Mods { shift: keys.any_pressed([ShiftLeft, ShiftRight]), ctrl: keys.any_pressed([ControlLeft, ControlRight]), alt: keys.any_pressed([AltLeft, AltRight]) }
}

#[cfg(test)]
mod tests;
