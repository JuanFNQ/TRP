//! The game's tabs, the opening tween and the screen's sounds.

use super::*;

/// The game's bar of tabs (`TARKOV UI`), drawn into its node as the game's
/// (`health_view::draw_tab_bar`): afresh as the tab chosen or the one under
/// the mouse changes, and with its buttons as the window's scale or width
/// does. A button over each tab, stacked as the tabs are drawn, each over
/// the one after it and the chosen over all (`TabGroup.SelectTab`), the
/// engine giving the mouse to what is drawn last: where two overlap the
/// earlier has it unless the later is chosen (`courses10`'s `inventory-
/// tabs`: OVERALL lit at GEAR's edge with HEALTH chosen, HEALTH pressed at
/// SKILLS' edge with MAP chosen). The
/// game's prestige tab is not worked in a raid (`Tab.SetInteractable`):
/// it is never lit under the mouse. Each tab's fill cut to its mask is
/// made at the window's scale (`health_view::tab_fill_images`).
#[allow(clippy::too_many_arguments)]
pub(super) fn game_tabs(
    mut commands: Commands,
    look: Res<crate::screens::health_view::HealthLook>,
    said: Res<crate::screens::health_view::Words>,
    selected: Res<Selected>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    bar: Query<Entity, With<GameTabBar>>,
    shown: Query<Entity, With<GameTabLook>>,
    mut buttons: Query<(&TabButton, &Interaction, &mut ZIndex), With<GameTab>>,
    (controller, opening, options): (Res<Controller>, Res<Opening>, Option<Res<crate::screens::options_view::OptionsState>>),
    mut drawn: Local<Option<(u32, u32, Option<usize>, Option<usize>, bool)>>,
    (mut fills, mut filled): (Local<HashMap<usize, Handle<Image>>>, Local<Option<(u32, u32)>>),
) {
    let (Ok(bar), Ok(window)) = (bar.single(), window.single()) else { return };
    if window.height() <= 0.0 {
        return;
    }
    // Shut, the bar is not kept: Bevy lays out every node there is, shown
    // or not, each frame. It is drawn whole as the screen
    // opens, its tabs' pictures kept while the window keeps its size.
    if !controller.inventory_open {
        if drawn.take().is_some() {
            commands.entity(bar).despawn_related::<Children>();
        }
        return;
    }
    let s = window.height() / SCREEN_HEIGHT;
    let screen = [window.width() / s, SCREEN_HEIGHT];
    // A tab not built is not lit under the mouse, as the game's prestige
    // tab, which does not work in a raid, is not.
    let hovered = buttons.iter().find(|(_, interaction, _)| **interaction != Interaction::None).map(|(tab, _, _)| tab.0).filter(|&index| TABS[index].1);
    // No tab chosen until the page is shown (`Opening`: the game's tab
    // group is shown with GEAR two frames after the key).
    let chosen = (opening.stage() >= 1).then_some(selected.0);
    // The chosen tab's button over the rest, which are spawned from the
    // last to the first.
    for (tab, _, mut z) in &mut buttons {
        let want = ZIndex(i32::from(Some(tab.0) == chosen));
        if *z != want {
            *z = want;
        }
    }
    // The unused tabs taken off (the HUD page's, at once): the game's
    // first four drawn, OPTIONS after them.
    let hidden = tabs_hidden(options.as_deref());
    let game_tabs_shown = if hidden { UNUSED_TABS.start } else { OPTIONS };
    let now = (s.to_bits(), screen[0].to_bits(), chosen, hovered, hidden);
    if *drawn == Some(now) {
        return;
    }
    let whole = drawn.is_none_or(|was| (was.0, was.1, was.4) != (now.0, now.1, now.4));
    *drawn = Some(now);
    let draw = |look_node: &mut ChildSpawnerCommands, fills: &HashMap<usize, Handle<Image>>| {
        let dimmed: Vec<bool> = TABS.iter().map(|(_, built)| !built).collect();
        crate::screens::health_view::draw_tab_bar(look_node, &look, &said, screen, chosen, hovered, TABS[OPTIONS].0, (&dimmed, game_tabs_shown), fills, s);
    };
    if *filled != Some((now.0, now.1)) {
        *filled = Some((now.0, now.1));
        for (_, old) in fills.drain() {
            images.remove(&old);
        }
        *fills = crate::screens::health_view::tab_fill_images(&look, screen, s, &mut images);
    }
    if whole {
        let places = crate::screens::health_view::tab_places(&look, screen, game_tabs_shown);
        let fills = &*fills;
        commands.entity(bar).despawn_related::<Children>().with_children(|bar| {
            let whole = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
            bar.spawn((GameTabLook, whole)).with_children(|look_node| draw(look_node, fills));
            for (index, [x, y, w, h]) in places.into_iter().enumerate().filter(|(index, _)| *index < TABS.len() && !(hidden && UNUSED_TABS.contains(index))).rev() {
                bar.spawn((TabButton(index), GameTab, Button, placed(x * s, y * s, w * s, h * s), ZIndex(i32::from(Some(index) == chosen))));
            }
            if let Some([x, y, w, h]) = crate::screens::health_view::back_place(&look, screen) {
                bar.spawn((GameBack, Button, placed(x * s, y * s, w * s, h * s)));
            }
        });
        return;
    }
    if let Ok(look_node) = shown.single() {
        commands.entity(look_node).despawn_related::<Children>().with_children(|look_node| draw(look_node, &fills));
    }
}

/// The frames since the screen opened, 0 on the key's (counted to 3), and
/// the clock as the GEAR page's parts came. The game's screen comes in
/// parts (`InventoryScreen.Show`, `ShowWithTab` split in frames): the bar
/// of tabs, none chosen, its glows and BACK on the key's frame; a frame
/// later the tabs are made, a frame after that GEAR is chosen and shown
/// (`ItemsPanel.UpdateEquipment`: the equipment, the health figures and the
/// containers on that frame), and the quick-use bar after a `Task.Yield`,
/// on the next (`courses10`'s `inventory-open-close`, all four openings),
/// with the panel of what is looted (`ItemsPanel.Show`: after a
/// `Task.Yield`, `SimpleStashPanel.Show` or `ComplexStashPanel.Show`;
/// `courses10`'s searches, three frames after the screen's: 320 to 323,
/// 355 to 358, 361 to 364).
#[derive(Resource, Default)]
pub(crate) struct Opening {
    pub(super) frames: Option<u32>,
    page_from: f64,
}

impl Opening {
    /// The page's parts drawn: none (0), the page (1), the quick-use bar,
    /// the panel of what is looted and what lies over the page too (2).
    pub(crate) fn stage(&self) -> u8 {
        match self.frames {
            Some(0 | 1) | None => 0,
            Some(2) => 1,
            Some(_) => 2,
        }
    }

    /// How long the page's parts have been shown at `now` (`Time<Real>`),
    /// counting the frame they came on whole: the health figures' rate
    /// arrows play their clip from there (`courses10`'s `inventory-open-
    /// close`: the same first values at every opening, a frame's time into
    /// the clip). None while they are not shown.
    pub(crate) fn page_shown(&self, now: f64) -> Option<f64> {
        (self.stage() >= 1).then(|| now - self.page_from)
    }
}

/// Counts the frames from the screen's opening, and fades the screen in
/// (`screen_fade`: `UIScreen.ShowGameObject`, a quarter of a second from
/// the key's frame, from 0 again at every opening).
pub(super) fn follow_opening(controller: Res<Controller>, time: Res<Time>, real: Res<Time<Real>>, mut opening: ResMut<Opening>, mut fades: ResMut<crate::ui::screen_fade::ScreenFades>) {
    let open = controller.inventory_open;
    fades.inventory.follow(open, time.delta_secs());
    let frames = match (open, opening.frames) {
        (false, _) => None,
        (true, None) => Some(0),
        (true, Some(frames)) => Some((frames + 1).min(3)),
    };
    if opening.frames != frames {
        if frames == Some(1) {
            opening.page_from = real.elapsed_secs_f64();
        }
        opening.frames = frames;
    }
}

/// A click on a tab that can be opened shows its page: the button let go
/// over the tab it went down on (`Tab.OnPointerClick`; nothing on the
/// press, the page on the release's frame: `courses10`'s `inventory-tabs`,
/// all five switches). The page's parts shown anew then, the rate arrows'
/// clip from its start (their animator made anew with the page: the same
/// take, HEALTH's arrows at alpha 0.5002 on both switches to it; `health-
/// skills`, GEAR's strip's on the switch back to it).
#[allow(clippy::too_many_arguments)]
pub(super) fn choose_tab(
    clicked: Query<(Entity, &Interaction, &TabButton), Changed<Interaction>>,
    features: Res<crate::screens::options_view::Features>,
    real: Res<Time<Real>>,
    (mut selected, mut opening): (ResMut<Selected>, ResMut<Opening>),
    mut tabs: Query<(&TabButton, &mut BackgroundColor), Without<GameTab>>,
    mut pages: Query<(&Page, &mut Node)>,
    mut shown: Local<Option<usize>>,
    mut was: Local<HashMap<Entity, Interaction>>,
) {
    let mut chosen = None;
    for (entity, interaction, tab) in &clicked {
        let before = was.insert(entity, *interaction).unwrap_or(Interaction::None);
        if before == Interaction::Pressed && *interaction == Interaction::Hovered && tab_open(tab.0, &features) {
            chosen = Some(tab.0);
        }
    }
    if let Some(index) = chosen.filter(|index| selected.0 != *index) {
        selected.0 = index;
        opening.page_from = real.elapsed_secs_f64() - real.delta_secs_f64();
    }
    // The tab chosen is shown: by a click, or by the screen having been
    // opened, which puts it on its gear tab (`toggle`).
    let index = selected.0;
    if *shown == Some(index) {
        return;
    }
    *shown = Some(index);
    for (tab, mut back) in &mut tabs {
        back.0 = if tab.0 == index { LINE } else { PANEL };
    }
    for (page, mut node) in &mut pages {
        node.display = if page.0 == index || (page.0 == GEAR && index == HEALTH) { Display::Flex } else { Display::None };
    }
}

/// The screen's sounds as it opens and shuts (`InventoryScreen.Show`, the
/// controller's `CloseScreenInterruption`: the backpack's, where its
/// `_playBackpackSounds` is on, as `courses10`'s `inventory-open-close`
/// shows it, on the key's frame), not as the run begins.
pub(super) fn screen_sounds(controller: Res<Controller>, mut was: Local<Option<bool>>, mut sounds: MessageWriter<Sound>) {
    let open = controller.inventory_open;
    if let Some(before) = was.replace(open).filter(|before| *before != open) {
        sounds.write(Sound::Ui(if before { Ui::BackpackClose } else { Ui::BackpackOpen }));
    }
}

/// A tab's sounds (its `ButtonFeedback`): the mouse come onto it, and a
/// click on it, the button let go over it; neither for a tab that does
/// not work (`Interactable`), as a tab not built here is not.
pub(super) fn tab_sounds(features: Res<crate::screens::options_view::Features>, tabs: Query<(Entity, &TabButton, &Interaction), Changed<Interaction>>, mut was: Local<HashMap<Entity, Interaction>>, mut sounds: MessageWriter<Sound>) {
    for (entity, tab, interaction) in &tabs {
        let before = was.insert(entity, *interaction).unwrap_or(Interaction::None);
        if !tab_open(tab.0, &features) {
            continue;
        }
        match (before, interaction) {
            (Interaction::None, Interaction::Hovered | Interaction::Pressed) => {
                sounds.write(Sound::Ui(Ui::ButtonOver));
            }
            (Interaction::Pressed, Interaction::Hovered) => {
                sounds.write(Sound::Ui(Ui::ButtonClick));
            }
            _ => {}
        }
    }
}
