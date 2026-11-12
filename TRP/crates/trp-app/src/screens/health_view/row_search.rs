//! The rows' search panels, the dogtag slot and the stash's search fills.

use super::*;

/// A containers' row's search parts (`SearchableItemView._searchableView`
/// on the row's template), by their paths below the template: the
/// unsearched panel and its loader, the button with its loader, icon and
/// words, the stop button, the timer; and the slot's own sign of its thing
/// being searched (`SlotView.SearchIcon`, `SearchableSlotView.
/// SearchOperationsChangedHandler`).
pub const ROW_UNSEARCHED: &str = "Unsearched Panel";

pub const ROW_UNSEARCHED_LOADER: &str = "Unsearched Panel/UnsearchedLabel/Loader";

pub const ROW_UNSEARCHED_TEXT: &str = "Unsearched Panel/UnsearchedLabel/Text";

pub const ROW_SEARCH_PANEL: &str = "Header Panel/SearchPanel";

pub const ROW_SEARCH_BUTTON: &str = "Header Panel/SearchPanel/Search Button";

pub const ROW_SEARCH_LOADER: &str = "Header Panel/SearchPanel/Search Button/Loader";

pub const ROW_SEARCH_ICON: &str = "Header Panel/SearchPanel/Search Button/Search Icon";

pub const ROW_SEARCH_TEXT: &str = "Header Panel/SearchPanel/Search Button/Text";

pub const ROW_STOP_BUTTON: &str = "Header Panel/SearchPanel/Stop Search Button";

pub const ROW_TIMER: &str = "Header Panel/SearchPanel/Timer";

pub const ROW_SLOT_SEARCHING: &str = "Slot Panel/Search Icon";

pub(super) const ROW_SEARCH_PARTS: [&str; 8] = [ROW_UNSEARCHED, ROW_UNSEARCHED_LOADER, ROW_SEARCH_BUTTON, ROW_SEARCH_LOADER, ROW_SEARCH_ICON, ROW_STOP_BUTTON, ROW_TIMER, ROW_SLOT_SEARCHING];

/// Draws a containers' row's search (`TARKOV UI`; `SearchableView.
/// UpdateSearchState`) as `search` has it, the row's slot panel at `at` in
/// a column so large (`left`), the slot's panel shown or not (the pockets'
/// is not, which puts the unsearched panel where the panel would be):
/// the unsearched panel where the container's grids would be while it is
/// not yet opened, the search button and the stop button and timer in the
/// header's row, and the slot's spinner while its thing is searched. Where
/// it takes a click, and where its loaders turn (left to the caller), in
/// the column's units.
#[allow(clippy::too_many_arguments)]
pub fn draw_row_search(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], at: [f32; 2], panel_shown: bool, search: &StashSearch, layer: u8, s: f32) -> (Vec<(SearchHit, [f32; 4])>, Vec<(&'static str, [f32; 4])>) {
    let Some(views) = &look.containers else { return (Vec::new(), Vec::new()) };
    let Some(slot_panel) = views.slot_panel else { return (Vec::new(), Vec::new()) };
    let mut panel = views.panel.clone();
    let template = panel.nodes[views.template].path.clone();
    let find = |panel: &trp_data::interface::screen::ScenePanel, part: &str| panel.find(&format!("{template}/{part}"));
    let [x, y, ..] = panel.rect(slot_panel, left);
    let origin = [at[0] - x, at[1] - y];
    panel.nodes[views.template].active = true;
    panel.nodes[slot_panel].active = panel_shown;
    let shown = [
        (ROW_UNSEARCHED, search.unsearched),
        (ROW_UNSEARCHED_LOADER, search.searching && search.unsearched),
        (ROW_SEARCH_BUTTON, search.button),
        (ROW_SEARCH_LOADER, search.searching),
        (ROW_SEARCH_ICON, !search.searching),
        (ROW_STOP_BUTTON, search.searching),
        (ROW_TIMER, search.searching),
        (ROW_SLOT_SEARCHING, search.searching && panel_shown),
    ];
    let mut set = HashMap::new();
    for (part, on) in shown {
        if let Some(index) = find(&panel, part) {
            panel.nodes[index].active = on;
            set.insert(index, on);
        }
    }
    let loaders: Vec<usize> = [ROW_UNSEARCHED_LOADER, ROW_SEARCH_LOADER, ROW_SLOT_SEARCHING].iter().filter_map(|part| find(&panel, part)).collect();
    for index in &loaders {
        if let Some(on) = set.get_mut(index) {
            *on = false;
        }
    }
    let mut words = HashMap::new();
    if let Some(text) = find(&panel, ROW_SEARCH_TEXT) {
        words.insert(text, if search.searching { said.say("SEARCHING...", "SEARCHING...").to_uppercase() } else { said.say("SEARCH", "SEARCH").to_uppercase() });
    }
    if let Some(timer) = find(&panel, ROW_TIMER) {
        words.insert(timer, search.timer.clone());
    }
    if let Some(text) = find(&panel, ROW_UNSEARCHED_TEXT) {
        words.insert(text, said.say("UNSEARCHED", "UNSEARCHED"));
    }
    // The button's row lays out its words and the timer by what they ask
    // (as the loot panel's header does, `draw_stash`).
    let search_panel = find(&panel, ROW_SEARCH_PANEL);
    look.ask_by_content_under(&mut panel, &look.containers_drawn, &words, search_panel);
    // The unsearched panel's label sized to its word, its loader after it.
    let unsearched_panel = find(&panel, ROW_UNSEARCHED);
    look.ask_by_content_under(&mut panel, &look.containers_drawn, &words, unsearched_panel);
    let within: Vec<usize> = [ROW_SEARCH_PANEL, ROW_UNSEARCHED, ROW_SLOT_SEARCHING].iter().filter_map(|part| find(&panel, part)).collect();
    let marks = Marks { set: Some(&set), words: Some(&words), within: &within, ..Marks::default() };
    draw_nodes(column, look, &panel, &look.containers_drawn, CONTAINERS + ((2 + layer as usize) << 16), marks, said, Frame::Is(left), origin, s);
    let place = |index: usize| {
        let [px, py, pw, ph] = panel.placed(index, left).rect;
        [px + origin[0], py + origin[1], pw, ph]
    };
    let mut hits = Vec::new();
    for (part, hit, on) in [
        (ROW_UNSEARCHED, SearchHit::Begin, search.unsearched && !search.searching && search.can_start),
        (ROW_SEARCH_BUTTON, SearchHit::Begin, search.button && !search.searching && search.can_start),
        (ROW_STOP_BUTTON, SearchHit::Stop, search.searching),
    ] {
        if let (Some(index), true) = (find(&panel, part), on) {
            hits.push((hit, place(index)));
        }
    }
    let spinners = [(ROW_UNSEARCHED_LOADER, search.searching && search.unsearched), (ROW_SEARCH_LOADER, search.searching), (ROW_SLOT_SEARCHING, search.searching && panel_shown)]
        .into_iter()
        .filter_map(|(part, on)| Some((part, place(find(&panel, part).filter(|_| on)?))))
        .collect();
    (hits, spinners)
}

/// A loader of a containers' row's search (`ROW_UNSEARCHED_LOADER`,
/// `ROW_SEARCH_LOADER`, `ROW_SLOT_SEARCHING`) as a spinner, as
/// `stash_loader` gives the loot panel's.
pub(crate) fn row_loader(look: &HealthLook, part: &str) -> Option<(Sprite, [f32; 4])> {
    let views = look.containers.as_ref()?;
    let index = views.panel.find(&format!("{}/{part}", views.panel.nodes[views.template].path))?;
    let picture = look.containers_drawn.pictures.get(index).copied().flatten()?;
    let image = look.images.get(picture.texture)?.clone()?;
    let color = views.panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color));
    Some((Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color }, picture.covers))
}

/// The dog tag's view in a body's containers' column (`ContainersPanel.
/// Show`: `_dogtagTemplate`, laid last in the pockets' row): its slot's
/// panel at `at`, full or empty, in a column so large.
pub fn draw_dogtag_slot(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, left: [f32; 2], at: [f32; 2], full: bool, s: f32) {
    let Some(views) = &look.containers else { return };
    let Some(slot_panel) = views.panel.find(DOGTAG_SLOT) else { return };
    let panel = &views.panel;
    let [x, y, ..] = panel.rect(slot_panel, left);
    let path = &panel.nodes[slot_panel].path;
    let mut set = HashMap::new();
    for (name, on) in [("Full Border", full), ("Empty Border", !full), ("Background", !full), ("BackImage", false)] {
        if let Some(part) = panel.find(&format!("{path}/{name}")) {
            set.insert(part, on);
        }
    }
    let marks = Marks { set: Some(&set), within: &[slot_panel], ..Marks::default() };
    draw_nodes(column, look, panel, &look.containers_drawn, CONTAINERS + (4 << 16), marks, said, Frame::Is(left), [at[0] - x, at[1] - y], s);
}

/// The dog tag's view's slot panel, below the containers' panel.
pub const DOGTAG_SLOT: &str = "Scrollview Parent/Containers Scrollview/Content/DogtagTemplate/Panel/Slot Panel";

/// Its side, in units (`DogtagTemplate`'s `LayoutElement`, 64 square).
pub const DOGTAG_SIDE: f32 = 64.0;

/// A loader of the search's (`SEARCH_LOADER`, `UNSEARCHED_LOADER`) as a
/// spinner: its picture in its colour, and the share of its square the
/// picture covers.
pub(crate) fn stash_loader(look: &HealthLook, path: &str) -> Option<(Sprite, [f32; 4])> {
    let stash = look.stash.as_ref()?;
    let index = stash.panel.find(path)?;
    let picture = look.stash_drawn.pictures.get(index).copied().flatten()?;
    let image = look.images.get(picture.texture)?.clone()?;
    let color = stash.panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color));
    Some((Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color }, picture.covers))
}

/// The search's parts of the panel for a container opened
/// (`SearchableView`: `_unsearchedPanel`, `_unsearchedButtonLoader`,
/// `_searchButton` with its `_loader`, `_searchIcon`, `_buttonCaption` and
/// `_stopSearchButton`, `_searchTimer`), by their paths in the stash panel.
pub const UNSEARCHED_PANEL: &str = "Simple Panel/Unsearched Panel";

pub const UNSEARCHED_LOADER: &str = "Simple Panel/Unsearched Panel/UnsearchedLabel/Loader";

pub const SEARCH_BUTTON: &str = "Simple Panel/TopPanel/SearchPanel/Search Button";

pub const SEARCH_LOADER: &str = "Simple Panel/TopPanel/SearchPanel/Search Button/Loader";

pub const SEARCH_ICON: &str = "Simple Panel/TopPanel/SearchPanel/Search Button/Search Icon";

pub const SEARCH_TEXT: &str = "Simple Panel/TopPanel/SearchPanel/Search Button/Text";

pub const STOP_BUTTON: &str = "Simple Panel/TopPanel/SearchPanel/Stop Search Button";

pub const SEARCH_TIMER: &str = "Simple Panel/TopPanel/SearchPanel/Timer";

/// The unsearched panel's word (`LocalizedText`, `UNSEARCHED`).
pub const UNSEARCHED_TEXT: &str = "Simple Panel/Unsearched Panel/UnsearchedLabel/Text";

pub(super) const SEARCH_PARTS: [&str; 7] = [UNSEARCHED_PANEL, UNSEARCHED_LOADER, SEARCH_BUTTON, SEARCH_LOADER, SEARCH_ICON, STOP_BUTTON, SEARCH_TIMER];

/// How the search of the container shown stands, for its parts of the
/// panel (`SearchableView.UpdateSearchState`): not yet searched (opened),
/// being searched, its button shown (not searched, being searched, or
/// holding what is still to be found), whether a search may begin
/// (`CanStartNewSearchOperation`), and the timer's words while searching.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StashSearch {
    pub unsearched: bool,
    pub searching: bool,
    pub button: bool,
    pub can_start: bool,
    pub timer: String,
}

/// What a click on a part of the search does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchHit {
    Begin,
    Stop,
}

/// The crate panel's stripes cut to their masks (`TARKOV UI`), as images so
/// many pixels to a unit, by the mask's object, in a stash panel so large.
pub fn stash_fill_images(look: &HealthLook, size: [f32; 2], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    let Some(stash) = &look.stash else { return HashMap::new() };
    fill_images(&look.stash_fills, |mask| stash.panel.rect(mask, size), s, images)
}

/// Fills cut to their masks as images, each as large as its mask's object
/// (`rect`, units) at so many pixels to a unit.
pub(super) fn fill_images(fills: &HashMap<usize, std::sync::Arc<MaskedFill>>, rect: impl Fn(usize) -> [f32; 4], s: f32, images: &mut Assets<Image>) -> HashMap<usize, Handle<Image>> {
    fills
        .iter()
        .map(|(&mask, fill)| {
            let [_, _, w, h] = rect(mask);
            let pixels = [((w * s).round() as u32).max(1), ((h * s).round() as u32).max(1)];
            let image = Image::new(
                bevy::render::render_resource::Extent3d { width: pixels[0], height: pixels[1], depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                fill.compose([w, h], pixels),
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            (mask, images.add(image))
        })
        .collect()
}
