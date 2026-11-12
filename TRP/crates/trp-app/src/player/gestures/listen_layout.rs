//! The menu's frames and layout, and the keys heard while it is open.

use super::*;

/// The screen's scale and centre, from the window (1080 units high).
#[derive(Clone, Copy)]
pub(super) struct Frame {
    pub(super) s: f32,
    pub(super) w: f32,
    pub(super) h: f32,
}

impl Frame {
    pub(super) fn of(window: &Window) -> Self {
        Self { s: (window.height() / 1080.0).max(0.1), w: window.width(), h: window.height() }
    }

    /// A point from the centre, y up, in the window's pixels.
    pub(super) fn at(&self, [x, y]: [f32; 2]) -> Vec2 {
        Vec2::new(self.w / 2.0 + x * self.s, self.h / 2.0 - y * self.s)
    }
}

/// A rectangle in the window's pixels: its left, top, width and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Area {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
}

impl Area {
    pub(super) fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    pub(super) fn node(&self) -> Node {
        Node { position_type: PositionType::Absolute, left: Val::Px(self.x), top: Val::Px(self.y), width: Val::Px(self.w), height: Val::Px(self.h), ..default() }
    }
}

/// How a phrase hangs from its place by which side of its group's centre
/// it is on (`GestureAudioBindAlignment`): right of it by its left edge,
/// words left, the key's tab on its right; left of it by its right edge,
/// words right, the tab on its left; on the line by its middle moved 11
/// right, words centred, the tab on its left. Its area, its words'
/// alignment, and whether the tab is on its left.
fn hang(frame: &Frame, group: [f32; 2], at: [f32; 2], width: f32) -> (Area, JustifyContent, bool) {
    let (s, p) = (frame.s, frame.at([group[0] + at[0], group[1] + at[1]]));
    let (w, h) = (width * s, 15.0 * s);
    if at[0] > 0.0 {
        (Area { x: p.x, y: p.y - h / 2.0, w, h }, JustifyContent::FlexStart, false)
    } else if at[0] < 0.0 {
        (Area { x: p.x - w, y: p.y - h / 2.0, w, h }, JustifyContent::FlexEnd, true)
    } else {
        (Area { x: p.x - w / 2.0 + 11.0 * s, y: p.y - h / 2.0, w, h }, JustifyContent::Center, true)
    }
}

/// Everything the menu shows, where, as the frame lays it out.
pub(super) struct Layout {
    pub(super) signs: Vec<(Area, Area, bool)>,
    /// Each group: its hover box, its ground, its phrases shown (trigger,
    /// area, alignment, tab on the left).
    pub(super) groups: Vec<(Area, Area, Vec<(i32, Area, JustifyContent, bool)>)>,
    pub(super) slots: Vec<(Area, bool)>,
}

/// The menu laid out: which phrases a group shows (the voice's; a
/// situational one only while active), each at the next of its places.
pub(super) fn layout(frame: &Frame, gestures: &Gestures, voiced: &HashSet<i32>, open_group: Option<usize>) -> Layout {
    let s = frame.s;
    let signs = Sign::GESTURES
        .iter()
        .enumerate()
        .map(|(n, _)| {
            let top = frame.at(SIGN_AT[n]);
            let hexagon = Area { x: top.x - 41.0 * s, y: top.y, w: 82.0 * s, h: 94.0 * s };
            // The label: beside the hexagon at mid height, below it on the
            // centre line, above it for the eleventh (`GestureItemBindAlignment`).
            let (lw, lh) = (96.0 * s, 15.0 * s);
            let x = SIGN_AT[n][0];
            let label = if n == 10 {
                Area { x: top.x - lw / 2.0 + 11.0 * s, y: top.y - 3.0 * s - lh, w: lw, h: lh }
            } else if x < 0.0 {
                Area { x: hexagon.x - 3.0 * s - lw, y: hexagon.y + hexagon.h / 2.0 - lh / 2.0, w: lw, h: lh }
            } else if x > 0.0 {
                Area { x: hexagon.x + hexagon.w + 3.0 * s, y: hexagon.y + hexagon.h / 2.0 - lh / 2.0, w: lw, h: lh }
            } else {
                Area { x: top.x - lw / 2.0 + 11.0 * s, y: hexagon.y + hexagon.h + 3.0 * s, w: lw, h: lh }
            };
            (hexagon, label, x <= 0.0)
        })
        .collect();
    let groups = GROUPS
        .iter()
        .enumerate()
        .map(|(g, (_, situational, phrases))| {
            let centre = frame.at(GROUP_AT[g]);
            let hover = Area { x: centre.x - 165.0 * s, y: centre.y - 120.0 * s, w: 330.0 * s, h: 240.0 * s };
            let ground = Area { x: centre.x - 80.0 * s, y: centre.y - 85.0 * s, w: 160.0 * s, h: 170.0 * s };
            let shown: Vec<i32> = phrases.iter().copied().filter(|p| voiced.contains(p) || *p == phrase::MUMBLE).filter(|p| !phrase::situational(*p) || gestures.quick.contains_key(p)).collect();
            let open = *situational || open_group == Some(g);
            let placed = if !open {
                Vec::new()
            } else if *situational {
                // A column from its place down, spaced 3, centred.
                shown
                    .iter()
                    .enumerate()
                    .map(|(n, p)| {
                        let area = Area { x: centre.x - 55.0 * s, y: centre.y + n as f32 * 18.0 * s, w: 110.0 * s, h: 15.0 * s };
                        (*p, area, JustifyContent::Center, true)
                    })
                    .collect()
            } else {
                shown
                    .iter()
                    .zip(PHRASE_AT)
                    .map(|(p, at)| {
                        let (area, justify, left) = hang(frame, GROUP_AT[g], at, 120.0);
                        (*p, area, justify, left)
                    })
                    .collect()
            };
            (hover, ground, placed)
        })
        .collect();
    // The bind panel: its foot 450 below the centre, two columns of six,
    // 95 wide, 55 apart, F1 at the top of the right.
    let (cw, ch, gap) = (95.0 * s, 15.0 * s, 55.0 * s);
    let foot = frame.h / 2.0 + 450.0 * s;
    let top = foot - 6.0 * ch - 5.0 * 2.0 * s;
    let slots = (0..12)
        .map(|key| {
            let right = key % 2 == 0;
            let row = (key / 2) as f32;
            let x = if right { frame.w / 2.0 + gap / 2.0 } else { frame.w / 2.0 - gap / 2.0 - cw };
            (Area { x, y: top + row * (ch + 2.0 * s), w: cw, h: ch }, right)
        })
        .collect();
    Layout { signs, groups, slots }
}

/// The binding menu's lines: under the pointer, one a key.
pub(super) fn bind_lines(frame: &Frame, menu: &BindMenu) -> Vec<Area> {
    let s = frame.s;
    (0..12).map(|n| Area { x: menu.at.x, y: menu.at.y + n as f32 * 16.0 * s, w: 115.0 * s, h: 15.0 * s }).collect()
}

/// The keys and the mouse: Y's three presses, the function keys, the
/// menu's clicks and the held list's wheel (`GamePlayerOwner`'s handlers).
#[allow(clippy::too_many_arguments)]
pub(super) fn listen(
    mut gestures: ResMut<Gestures>,
    mut controller: ResMut<Controller>,
    mut bindings: ResMut<crate::platform::keys::Bindings>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Query<&Window, With<PrimaryWindow>>,
    voiced: Res<Voiced>,
    time: Res<Time<Real>>,
    (mut asked, mut told, mut sounds): (MessageWriter<PhraseAsked>, MessageReader<PhraseTold>, MessageWriter<crate::audio::ui_sound::Sound>),
    mut options: Option<ResMut<crate::screens::options_view::OptionsState>>,
) {
    let now = time.elapsed_secs_f64();
    let gestures = &mut *gestures;
    // The options' state keeps the binds (and their file); the menu reads
    // them from it, and is the only place they are made.
    if let Some(binds) = options.as_deref().map(|state| state.gesture_binds).filter(|binds| *binds != gestures.binds) {
        gestures.binds = binds;
    }
    // The prompt's phrase begun: it blinks for its clip (`TryNotifyPhrasePlayed`),
    // and one a situation raised is used up.
    for said in told.read() {
        if gestures.last_quick == Some(said.trigger) {
            gestures.blink = Some((now, now + said.length as f64));
            if gestures.timed.iter().any(|(t, _)| *t == said.trigger) {
                gestures.timed.retain(|(t, _)| *t != said.trigger);
                if !gestures.held.contains(&said.trigger) {
                    gestures.deactivate(said.trigger);
                }
            }
            gestures.last_quick = None;
        }
    }
    let playing = !controller.inventory_open && !controller.edit_build;
    let Ok(window) = window.single() else { return };
    let frame = Frame::of(window);
    let cursor = window.cursor_position();
    if gestures.open {
        // The inventory taken through the menu shuts it, as the battle
        // screen goes.
        let escape = keys.just_pressed(KeyCode::Escape);
        let dead = controller.health.as_ref().is_some_and(|health| !health.alive);
        if !playing || escape || dead {
            if escape {
                keys.clear_just_pressed(KeyCode::Escape);
                sounds.write(crate::audio::ui_sound::Sound::Ui(crate::audio::ui_sound::Ui::MenuEscape));
            }
            close(gestures, &mut controller, &mut bindings);
            return;
        }
        let lay = layout(&frame, gestures, &voiced.0, gestures.group_open);
        // What is under the pointer: the binding menu first, then a hand
        // sign, an open group's phrase, a slot; the group open under it.
        let mut pick = None;
        if let (Some(menu), Some(p)) = (gestures.bind_menu.as_mut(), cursor) {
            menu.hovered = bind_lines(&frame, menu).iter().position(|line| line.contains(p));
        }
        if let Some(p) = cursor {
            pick = lay.signs.iter().position(|(hexagon, label, _)| hexagon.contains(p) || label.contains(p)).map(Pick::Sign);
            if pick.is_none() {
                for (g, (_, _, placed)) in lay.groups.iter().enumerate() {
                    if let Some((trigger, ..)) = placed.iter().find(|(_, area, ..)| area.contains(p)) {
                        pick = Some(Pick::Phrase(g, *trigger));
                    }
                }
            }
            if pick.is_none() {
                pick = lay.slots.iter().position(|(area, _)| area.contains(p)).map(Pick::Slot);
            }
            // A common group opens under the pointer (its whole box), and
            // stays open while the binding menu came from it.
            if gestures.bind_menu.is_none() {
                gestures.group_open = lay.groups.iter().enumerate().filter(|(g, _)| !GROUPS[*g].1).find(|(_, (hover, ..))| hover.contains(p)).map(|(g, _)| g);
            }
        }
        gestures.pick = pick;
        // The binding menu: a click on a line binds, any button let go
        // elsewhere shuts it (`SimpleContextMenu`).
        if let Some(menu) = gestures.bind_menu {
            if buttons.just_released(MouseButton::Left) || buttons.just_released(MouseButton::Right) || buttons.just_released(MouseButton::Middle) {
                if let Some(key) = menu.hovered.filter(|_| buttons.just_released(MouseButton::Left)) {
                    gestures.bind(key, menu.number);
                    if let Some(state) = options.as_deref_mut() {
                        state.gesture_binds = gestures.binds;
                        if state.file.is_some() {
                            keep(&crate::screens::options_view::options_file(), &gestures.binds);
                        }
                    }
                    info!("gestures: F{} now {}", key + 1, menu.number);
                }
                gestures.bind_menu = None;
            }
            return;
        }
        let number = |pick: Pick| match pick {
            Pick::Sign(n) => Sign::GESTURES[n].game_number() as i32,
            Pick::Phrase(_, trigger) => trigger,
            Pick::Slot(key) => gestures.binds[key],
        };
        // Its key again shuts it, whatever is under the pointer chosen
        // (`CloseWithSelected`).
        if bindings.fired("Mumble") {
            let chosen = pick.map(number);
            close(gestures, &mut controller, &mut bindings);
            if let Some(chosen) = chosen {
                play(chosen, false, gestures, &mut controller, &mut asked);
            }
            return;
        }
        if let Some(pick) = pick {
            let left = buttons.just_released(MouseButton::Left);
            let middle = buttons.just_released(MouseButton::Middle) && !matches!(pick, Pick::Slot(_));
            if left || middle {
                let chosen = number(pick);
                close(gestures, &mut controller, &mut bindings);
                play(chosen, middle, gestures, &mut controller, &mut asked);
                return;
            }
            // A right click on a hand sign or a phrase that is not
            // situational opens the binding menu.
            let bindable = match pick {
                Pick::Sign(_) => true,
                Pick::Phrase(_, trigger) => !phrase::situational(trigger),
                Pick::Slot(_) => false,
            };
            if bindable && buttons.just_released(MouseButton::Right) {
                if let Some(p) = cursor {
                    gestures.bind_menu = Some(BindMenu { number: number(pick), at: p, hovered: None });
                }
            }
        }
        return;
    }
    if !playing || !controller.health.as_ref().is_none_or(|health| health.alive) {
        gestures.list_wait = None;
        gestures.list = None;
        controller.gestures_list = false;
        bindings.wheel_taken = false;
        return;
    }
    // Y twice: the menu (`ToggleBindPanel`).
    if bindings.fired("Mumble") {
        gestures.open = true;
        gestures.list = None;
        gestures.list_wait = None;
        controller.gestures_open = true;
        controller.gestures_list = false;
        bindings.only = Some(&["Mumble", "Inventory"]);
        bindings.wheel_taken = false;
        info!("gestures: the menu opened");
        return;
    }
    // Y held: the list, after its wait; let go, its row said
    // (`MumbleDropdown`).
    if bindings.fired("MumbleDropdown") {
        gestures.list_wait = Some(now);
    }
    if gestures.list_wait.is_some_and(|began| now - began >= LIST_WAIT) && gestures.list.is_none() {
        gestures.list_wait = None;
        if !gestures.quick.is_empty() {
            gestures.list = Some(0);
        }
    }
    if bindings.ended("MumbleDropdown") {
        gestures.list_wait = None;
        if let Some(row) = gestures.list.take() {
            if let Some(chosen) = gestures.rows().get(row).copied().filter(|p| gestures.quick.contains_key(p)) {
                play(chosen, false, gestures, &mut controller, &mut asked);
            }
            gestures.quick_barred_until = now + QUICK_BAR;
        }
    }
    if let Some(row) = gestures.list.as_mut() {
        // The wheel moves the choice, wrapping: down a row a notch down.
        let rows = gestures.quick.len().max(1);
        let notch = crate::platform::keys::notch(scroll.delta.y);
        if notch < 0 {
            *row = (*row + 1) % rows;
        } else if notch > 0 {
            *row = (*row + rows - 1) % rows;
        }
        *row = (*row).min(rows - 1);
    }
    controller.gestures_list = gestures.list.is_some();
    bindings.wheel_taken = gestures.list.is_some();
    // A tap: the prompt's phrase, its priority lowered by 10
    // (`ActivateCommand`).
    let blinking = gestures.blink.is_some_and(|(_, until)| now < until);
    if bindings.fired("MumbleQuick") && gestures.list.is_none() && now >= gestures.quick_barred_until && !blinking {
        if let Some(top) = gestures.top() {
            gestures.last_quick = Some(top);
            if let Some(priority) = gestures.quick.get_mut(&top) {
                *priority -= 10;
            }
            play(top, false, gestures, &mut controller, &mut asked);
        }
    }
    // The function keys' binds; pressed twice, aggressively.
    for key in 0..12 {
        let (plain, twice) = (bindings.fired(&format!("F{}", key + 1)), bindings.fired(&format!("DoubleF{}", key + 1)));
        if plain || twice {
            play(gestures.binds[key], twice, gestures, &mut controller, &mut asked);
        }
    }
}

/// Shuts the menu (`GesturesMenu.Close`): the bindings heard again, the
/// cursor and the blur let go.
fn close(gestures: &mut Gestures, controller: &mut Controller, bindings: &mut crate::platform::keys::Bindings) {
    gestures.open = false;
    gestures.pick = None;
    gestures.group_open = None;
    gestures.bind_menu = None;
    controller.gestures_open = false;
    bindings.only = None;
    info!("gestures: the menu shut");
}
