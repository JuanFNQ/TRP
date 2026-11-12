//! The stash's places and drawing, the search tint and the complex view.

use super::*;

/// Where the panel for a container opened (`TARKOV UI`) puts things, in the
/// stash panel's frame of that size (units, from its top left): what of the
/// grids shows (its mask), where the wheel scrolls them (its scroll view),
/// the corner the grids are laid from (the scroll view's content, with its
/// column's padding), its bar, and its scrolling.
pub struct StashPlaces {
    pub mask: [f32; 4],
    pub view: [f32; 4],
    pub content: [f32; 2],
    pub bar: Option<([f32; 4], trp_data::interface::screen::ScrollBar)>,
    pub scrolling: trp_data::interface::screen::Scrolling,
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
/// the game's text sizes it (`ContentSizeFitter`). With the search's parts
/// as `search` has them (`SearchableView.UpdateSearchState`); where they
/// take a click, and where its loaders turn (left to the caller, drawn
/// as the app turns a loader), in the panel's units.
#[allow(clippy::too_many_arguments)]
pub fn draw_stash(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], name: &str, present: &[bool], fills: &HashMap<usize, Handle<Image>>, layer: u8, s: f32, search: &StashSearch) -> (Vec<(SearchHit, [f32; 4])>, Vec<(&'static str, [f32; 4])>) {
    let Some(stash) = &look.stash else { return (Vec::new(), Vec::new()) };
    let mut panel = stash.panel.clone();
    // The search's parts on and off (`SearchableView.UpdateSearchState`,
    // `SearchButton.SetSearchStatus`): the unsearched panel while not
    // searched, its loader while it is being searched; the button while
    // not searched, searched, or holding what is still to be found, its
    // loader in place of its icon while searching, the stop button and
    // the timer with it. Their row is laid out by what is on.
    let shown = [
        (UNSEARCHED_PANEL, search.unsearched),
        (UNSEARCHED_LOADER, search.searching && search.unsearched),
        (SEARCH_BUTTON, search.button),
        (SEARCH_LOADER, search.searching),
        (SEARCH_ICON, !search.searching),
        (STOP_BUTTON, search.searching),
        (SEARCH_TIMER, search.searching),
    ];
    let mut search_set = Vec::new();
    for (path, on) in shown {
        if let Some(index) = panel.find(path) {
            search_set.push((index, on));
        }
    }
    for &(index, on) in &search_set {
        panel.nodes[index].active = on;
    }
    let search_words: Vec<(usize, String)> = [
        (panel.find(SEARCH_TEXT), if search.searching { said.say("SEARCHING...", "SEARCHING...").to_uppercase() } else { said.say("SEARCH", "SEARCH").to_uppercase() }),
        // (No words for the timer: the scene's own, as it was never written.)
        (panel.find(SEARCH_TIMER).filter(|_| !search.timer.is_empty()), search.timer.clone()),
        (panel.find(UNSEARCHED_TEXT), said.say("UNSEARCHED", "UNSEARCHED")),
    ]
    .into_iter()
    .filter_map(|(index, words)| Some((index?, words)))
    .collect();
    // The name's box, as wide and high as its words with its padding.
    if let (Some(name_panel), Some(text), Some(font)) = (stash.name_panel, stash.name, look.name_font.as_ref()) {
        if let Some(look_of) = panel.nodes[text].text.clone() {
            let words = if look_of.style & 16 != 0 { name.to_uppercase() } else { name.to_owned() };
            let own = [font.width(&words, look_of.size, look_of.spacing, false) + look_of.margin[0] + look_of.margin[2], font.height(look_of.size)];
            panel.nodes[text].element = Some(trp_data::interface::screen::Element { ignore: false, min: own, preferred: own, flexible: [-1.0, -1.0] });
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
    // The loaders laid out where they are on, drawn by the caller.
    let loaders: Vec<usize> = [UNSEARCHED_LOADER, SEARCH_LOADER].iter().filter_map(|path| panel.find(path)).collect();
    set.extend(search_set.iter().map(|&(index, on)| (index, on && !loaders.contains(&index))));
    words.extend(search_words);
    // The header's toggle lays out its icon (`StashHeaderIcons_Off`, the
    // loot's mark) and its word by what they ask, which the layout here
    // does not ask of them itself: they were drawn at no size (the loot's
    // icon).
    let top_panel = panel.find("Simple Panel/TopPanel");
    look.ask_by_content_under(&mut panel, &look.stash_drawn, &words, top_panel);
    // The unsearched panel's label sized to its word, its loader after it.
    let unsearched_panel = panel.find(UNSEARCHED_PANEL);
    look.ask_by_content_under(&mut panel, &look.stash_drawn, &words, unsearched_panel);
    let over: Vec<usize> = (0..panel.nodes.len()).filter(|&index| panel.nodes[index].parent == Some(stash.simple) && ["Border"].contains(&panel.nodes[index].name.as_str())).chain(stash.filters).chain(stash.name_panel).collect();
    // The column of filters is shown with the grids (`SearchableItemView.
    // ShowGrids`, `GridView.Show`: `FilterPanel.Show`), not while the
    // container is unsearched (`courses10`'s searches: none before the
    // open's frame).
    let unshown: Vec<usize> = stash.filters.filter(|_| search.unsearched).into_iter().collect();
    // (The search button's ground is tinted each frame: `tint_search_button`.)
    let marks = match layer {
        0 => Marks { set: Some(&set), words: Some(&words), within: &[stash.simple], skip: &over, fills: Some(fills), alphas: Some(&alphas), ..Marks::default() },
        _ => Marks { set: Some(&set), words: Some(&words), within: &over, skip: &unshown, fills: Some(fills), alphas: Some(&alphas), ..Marks::default() },
    };
    draw_nodes(column, look, &panel, &look.stash_drawn, STASH + ((layer as usize) << 16), marks, said, Frame::Is(size), [0.0, 0.0], s);
    // Where the search takes a click: the unsearched panel (a search
    // begun, where one may: `_unsearchedPanel.interactable`), the button
    // (begun, while not searching) and the stop button.
    let mut hits = Vec::new();
    for (path, hit, on) in [
        (UNSEARCHED_PANEL, SearchHit::Begin, search.unsearched && !search.searching && search.can_start),
        (SEARCH_BUTTON, SearchHit::Begin, search.button && !search.searching && search.can_start),
        (STOP_BUTTON, SearchHit::Stop, search.searching),
    ] {
        if let (Some(index), true) = (panel.find(path), on) {
            hits.push((hit, panel.placed(index, size).rect));
        }
    }
    let spinners = [(UNSEARCHED_LOADER, search.searching && search.unsearched), (SEARCH_LOADER, search.searching)]
        .into_iter()
        .filter_map(|(path, on)| Some((path, panel.placed(panel.find(path).filter(|_| on)?, size).rect)))
        .collect();
    (hits, spinners)
}

/// The loot panel's search button's tint as it fades (`search_tint`): the
/// container whose button it is, while it is drawn; and the containers
/// whose view was brought up to date on this frame while searching
/// (`SearchableView.UpdateSearchState`, as the container opens:
/// `ItemSearchedHandler`), which `search_view` names.
#[derive(Resource, Default)]
pub(crate) struct SearchTint {
    pub(super) shown: Option<(trp_sim::items::inventory::Id, crate::screens::split_view::Fading)>,
    pub(crate) renewed: Vec<trp_sim::items::inventory::Id>,
}

/// The loot panel's search button tinted by its state, each frame
/// (`SearchButton`, a `Button`: its ground tinted by its `Selectable`
/// colours, the loot panel's in the install idle 1 and off (0.184,
/// 0.184, 0.184, 0.628) over 0.1 s). While it searches it takes no click
/// (`SetSearchStatus`: `interactable = !active`) and is at its off tint;
/// else idle (the mouse's tints over it are not drawn). A change of tint
/// fades straight across in the fade's time from the colour it had, its
/// first step on the frame of the change (`Graphic.CrossFadeColor`,
/// `TweenRunner`); shown, it is at the tint it then has, idle
/// (`Selectable.OnEnable`), so the search the panel begins as it shows is a
/// fade from there. Brought up to date while searching (the open), it is
/// made to work and not again in one go: a step of a fade toward idle,
/// then one back (`courses10`'s `dead-scav-search-a`: 0.628 to 0.656 at the
/// open, 0.628 again 0.1 s on).
pub(super) fn tint_search_button(look: Res<HealthLook>, carried: Res<crate::screens::inventory_view::Carried>, time: Res<Time<Real>>, mut tint: ResMut<SearchTint>, mut slices: Query<(&SliceOf, &mut ImageNode)>) {
    let renewed = std::mem::take(&mut tint.renewed);
    let Some(stash) = look.stash.as_ref() else { return };
    let Some((tints, Some(ground))) = stash.search_tints else { return };
    let piece = STASH + ground;
    if !slices.iter().any(|(slice, _)| slice.0 == piece) {
        tint.shown = None;
        return;
    }
    let id = carried.loot.id;
    let searching = carried.searcher.searching(id).is_some();
    let (idle, wanted) = (search_tint(&tints, false), search_tint(&tints, searching));
    if tint.shown.as_ref().is_none_or(|(shown, _)| *shown != id) {
        tint.shown = Some((id, crate::screens::split_view::Fading { from: idle, to: idle, since: tints.fade }));
    }
    let Some((_, fade)) = tint.shown.as_mut() else { return };
    let tint_now = step_tint(fade, idle, wanted, searching && renewed.contains(&id), time.delta_secs(), tints.fade);
    let base = stash.panel.nodes[ground].image.as_ref().map_or(Color::WHITE, |image| rgba(image.color)).to_srgba();
    let color = Color::srgba(base.red * tint_now[0], base.green * tint_now[1], base.blue * tint_now[2], base.alpha * tint_now[3]);
    for (slice, mut image) in &mut slices {
        if slice.0 == piece && image.color != color {
            image.color = color;
        }
    }
}

/// A frame of a search button's tint: brought up to date (`renewed`), a
/// step toward idle and a fade back begun; a new tint wanted, a fade to it
/// begun from where it is; the fade then moved on by the frame's time, its
/// first step on the frame it began (`TweenRunner`). Its colour now.
pub(super) fn step_tint(fade: &mut crate::screens::split_view::Fading, idle: [f32; 4], wanted: [f32; 4], renewed: bool, delta: f32, length: f32) -> [f32; 4] {
    if renewed {
        *fade = crate::screens::split_view::Fading { from: fade.now(length), to: idle, since: delta };
        *fade = crate::screens::split_view::Fading { from: fade.now(length), to: wanted, since: 0.0 };
    }
    if fade.to != wanted {
        *fade = crate::screens::split_view::Fading { from: fade.now(length), to: wanted, since: 0.0 };
    }
    fade.since += delta;
    fade.now(length)
}

/// The search button's tint, off or idle, by its multiplier.
fn search_tint(tints: &trp_data::interface::screen::Tints, off: bool) -> [f32; 4] {
    (if off { tints.disabled } else { tints.normal }).map(|channel| channel * tints.multiplier)
}

/// The places of the panel for a corpse's gear (`ComplexStashPanel`) in a
/// stash panel so large: its scroll view (`Containers Scrollview`, its own
/// mask), the corner of its column (`_lootPanel`, the scroll view's
/// content) with the column's padding and spacing (a `VerticalLayoutGroup`:
/// the gear's panel first, then the containers' rows), its bar, and its
/// scrolling.
pub struct ComplexPlaces {
    pub view: [f32; 4],
    pub content: [f32; 2],
    pub padding: [f32; 4],
    pub spacing: f32,
    pub bar: Option<([f32; 4], trp_data::interface::screen::ScrollBar)>,
    pub scrolling: trp_data::interface::screen::Scrolling,
}

pub fn complex_places(look: &HealthLook, size: [f32; 2]) -> Option<ComplexPlaces> {
    let stash = look.stash.as_ref()?;
    let complex = stash.complex?;
    let scrolling = complex.scrolling?;
    let mut panel = stash.panel.clone();
    panel.nodes[complex.panel].active = true;
    let view = panel.rect(scrolling.view.or(complex.scroll)?, size);
    let content = scrolling.content.or(complex.content)?;
    let [x, y, ..] = panel.rect(content, size);
    let group = panel.nodes[content].group;
    let bar = scrolling.bar.map(|bar| (panel.rect(bar.object, size), bar));
    Some(ComplexPlaces { view, content: [x, y], padding: group.map_or([0.0; 4], |group| group.padding), spacing: group.map_or(0.0, |group| group.spacing), bar, scrolling })
}

/// Draws the panel for a corpse's gear (`TARKOV UI`; `ComplexStashPanel.
/// Show`) into the stash panel's place of that size: under what it holds
/// (`layer` 0) its scroll view's ground and its header with the install's
/// word for `LOOT` (`_complexGridName`); over it (1) the name's box with
/// the body's name (`_containerName`: the corpse's controller's
/// `ContainerName`), sized to the name as the game's text sizes it.
pub fn draw_complex(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, size: [f32; 2], name: &str, layer: u8, s: f32) {
    let Some(stash) = &look.stash else { return };
    let Some(complex) = stash.complex else { return };
    let mut panel = stash.panel.clone();
    panel.nodes[complex.panel].active = true;
    if let (Some(name_panel), Some(text), Some(font)) = (complex.name_panel, complex.name, look.name_font.as_ref()) {
        if let Some(look_of) = panel.nodes[text].text.clone() {
            let words = if look_of.style & 16 != 0 { name.to_uppercase() } else { name.to_owned() };
            let own = [font.width(&words, look_of.size, look_of.spacing, false) + look_of.margin[0] + look_of.margin[2], font.height(look_of.size)];
            panel.nodes[text].element = Some(trp_data::interface::screen::Element { ignore: false, min: own, preferred: own, flexible: [-1.0, -1.0] });
            let padding = panel.nodes[name_panel].group.map_or([0.0; 4], |group| group.padding);
            panel.nodes[name_panel].layout.size_delta = [own[0] + padding[0] + padding[1], own[1] + padding[2] + padding[3]];
        }
    }
    let mut words = HashMap::new();
    if let Some(grid_name) = complex.grid_name {
        words.insert(grid_name, said.say("LOOT", "LOOT"));
    }
    if let Some(text) = complex.name {
        words.insert(text, name.to_owned());
    }
    look.ask_by_content_under(&mut panel, &look.stash_drawn, &words, complex.header);
    let set: HashMap<usize, bool> = [(complex.panel, true)].into_iter().collect();
    let over: Vec<usize> = complex.name_panel.into_iter().collect();
    let marks = match layer {
        0 => Marks { set: Some(&set), words: Some(&words), within: &[complex.panel], skip: &over, ..Marks::default() },
        _ => Marks { set: Some(&set), words: Some(&words), within: &over, ..Marks::default() },
    };
    draw_nodes(column, look, &panel, &look.stash_drawn, STASH + ((2 + layer as usize) << 16), marks, said, Frame::Is(size), [0.0, 0.0], s);
}
