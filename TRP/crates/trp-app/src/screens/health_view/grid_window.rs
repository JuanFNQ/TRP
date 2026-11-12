//! The grid window built and drawn, with its search loader.

use super::*;

/// The windows of containers' grids, each window's past the one before it.
const GRID_WINDOW: usize = 29 << 20;

/// The game's window of a container's grids laid out for one container
/// (`GridWindow`; inventory spec, "A container's window"): its panel with
/// the caption's words asking the caption's row for their width, its size
/// in units, each grid's place in it, the caption's words as they fit,
/// the caption's bar (what moves it) and the cross, and whether it is the
/// priority window.
pub(crate) struct GridWindowBuilt {
    pub(super) panel: trp_data::interface::screen::ScenePanel,
    pub(crate) size: [f32; 2],
    pub(crate) grids: Vec<[f32; 4]>,
    pub(super) caption: String,
    pub(crate) bar: [f32; 4],
    pub(crate) close: [f32; 4],
    picked: bool,
    /// Its search's parts on or off, the button's words, where a click
    /// begins or stops it, and where its loaders turn (left to the caller).
    pub(super) search_set: HashMap<usize, bool>,
    pub(super) search_words: Vec<(usize, String)>,
    pub(crate) search_hits: Vec<(SearchHit, [f32; 4])>,
    pub(crate) search_spinners: Vec<(&'static str, [f32; 4])>,
}

/// The window's search's parts (`GridWindow`'s `SearchableView`), by their
/// paths: the caption's button (a `SearchButton`, which stops a search
/// under way when clicked again), its icon, loader and words, and the
/// unsearched panel with its loader, laid where the grids would be.
pub(crate) const WINDOW_SEARCH_BUTTON: &str = "Caption Panel/Search Button";

pub(crate) const WINDOW_SEARCH_ICON: &str = "Caption Panel/Search Button/Search Icon";

pub(crate) const WINDOW_SEARCH_LOADER: &str = "Caption Panel/Search Button/Loader";

pub(crate) const WINDOW_SEARCH_TEXT: &str = "Caption Panel/Search Button/Text";

pub(crate) const WINDOW_UNSEARCHED: &str = "Unsearched Panel";

pub(crate) const WINDOW_UNSEARCHED_LOADER: &str = "Unsearched Panel/Text/Loader";

pub(crate) const WINDOW_UNSEARCHED_TEXT: &str = "Unsearched Panel/Text";

pub(super) const WINDOW_SEARCH_PARTS: [&str; 5] = [WINDOW_SEARCH_BUTTON, WINDOW_SEARCH_ICON, WINDOW_SEARCH_LOADER, WINDOW_UNSEARCHED, WINDOW_UNSEARCHED_LOADER];

/// A loader of the window's search as a spinner, as `stash_loader` gives
/// the loot panel's.
pub(crate) fn window_loader(look: &HealthLook, path: &str) -> Option<(Sprite, [f32; 4])> {
    let window = look.grid_window.as_ref()?;
    let index = window.panel.find(path)?;
    let picture = look.grid_window_drawn.pictures.get(index).copied().flatten()?;
    let image = look.images.get(picture.texture)?.clone()?;
    let color = window.panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color));
    Some((Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color }, picture.covers))
}

/// The window of a container's grids for a container of that name, layout
/// and grids (`GridWindow.Show`): the container's own layout where it has
/// one with as many grids (`TemplatedGridsView`), else the default row
/// (`GeneratedGridsView`), at the window's padding (5 left, 26 down); the
/// window as large as its padding and the grids ask (`ContentSizeFitter`,
/// preferred), at least its `LayoutElement`'s least (128 by 64). The
/// caption is the short name as the locale has it, not in capitals (its
/// text has no capitals' style), cut with an ellipsis where its row is
/// narrower (`TextOverflowModes.Ellipsis`). No slots are drawn: neither
/// view the window makes has any in a raid.
#[allow(clippy::too_many_arguments)]
pub(crate) fn grid_window_built(look: &HealthLook, name: &str, layout: &str, cells: &[[u32; 2]], picked: bool, search: Option<&StashSearch>, said: &Words) -> Option<GridWindowBuilt> {
    let window = look.grid_window.as_ref()?;
    let mut panel = window.panel.clone();
    // A searchable thing's search (`SearchableView.UpdateSearchState`): the
    // button while not searched, being searched or holding what is still to
    // be found, its loader in place of its icon while searching; the
    // unsearched panel, in the window's column, in place of the grids
    // until it has been opened (`OnShowGrids`).
    let unsearched = search.is_some_and(|search| search.unsearched);
    let mut search_set = HashMap::new();
    if let Some(search) = search {
        for (path, on) in [
            (WINDOW_SEARCH_BUTTON, search.button),
            (WINDOW_SEARCH_ICON, !search.searching),
            (WINDOW_SEARCH_LOADER, search.searching),
            (WINDOW_UNSEARCHED, search.unsearched),
            (WINDOW_UNSEARCHED_LOADER, search.searching && search.unsearched),
        ] {
            if let Some(index) = panel.find(path) {
                panel.nodes[index].active = on;
                let loader = [WINDOW_SEARCH_LOADER, WINDOW_UNSEARCHED_LOADER].contains(&path);
                search_set.insert(index, on && !loader);
            }
        }
    }
    // The button's words, and the unsearched panel's in the install's words
    // (`LocalizedText`, `UNSEARCHED`: the prefab keeps another language's).
    let search_words: Vec<(usize, String)> = match search {
        Some(search) => [
            (panel.find(WINDOW_SEARCH_TEXT), if search.searching { said.say("SEARCHING...", "SEARCHING...").to_uppercase() } else { said.say("SEARCH", "SEARCH").to_uppercase() }),
            (panel.find(WINDOW_UNSEARCHED_TEXT), said.say("UNSEARCHED", "UNSEARCHED")),
        ]
        .into_iter()
        .filter_map(|(index, words)| Some((index?, words)))
        .collect(),
        None => Vec::new(),
    };
    let asked: HashMap<usize, String> = search_words.iter().cloned().collect();
    if let Some(button) = panel.find(WINDOW_SEARCH_BUTTON).filter(|_| search.is_some()) {
        look.ask_by_content_under(&mut panel, &look.grid_window_drawn, &asked, Some(button));
    }
    if let Some(unsearched_panel) = panel.find(WINDOW_UNSEARCHED).filter(|_| unsearched) {
        look.ask_by_content_under(&mut panel, &look.grid_window_drawn, &asked, Some(unsearched_panel));
    }
    let rects = if unsearched { Vec::new() } else { container_grids(look, layout, cells) };
    let unsearched_size = panel.find(WINDOW_UNSEARCHED).and_then(|index| panel.nodes[index].element).map_or([0.0, 0.0], |element| element.min);
    let content = match look.layouts.get(layout).filter(|(_, own)| own.len() >= cells.len()) {
        _ if unsearched => unsearched_size,
        Some((own, _)) => *own,
        None => {
            let row = look.containers.as_ref().and_then(|views| views.grids_row).unwrap_or_default();
            let right = rects.iter().fold(0.0f32, |most, [x, _, w, _]| most.max(x + w));
            let bottom = rects.iter().fold(0.0f32, |most, [_, y, _, h]| most.max(y + h));
            [right + row.padding[1], bottom + row.padding[3]]
        }
    };
    let pad = panel.nodes[0].group.map_or([5.0, 4.0, 26.0, 4.0], |group| group.padding);
    let least = panel.nodes[0].element.map_or([128.0, 64.0], |element| element.min);
    let size = [(pad[0] + content[0] + pad[1]).max(least[0]), (pad[2] + content[1] + pad[3]).max(least[1])];
    let grids = rects.iter().map(|[x, y, w, h]| [pad[0] + x, pad[2] + y, *w, *h]).collect();
    // The caption asks its row for its words' own size; what it gets of the
    // row it shows, cut to fit.
    if let Some(asked) = text_size(look, &panel, window.caption, name) {
        prefer_size(&mut panel, window.caption, asked);
    }
    let room = panel.placed(window.caption, size).rect[2];
    let caption = match panel.nodes[window.caption].text.as_ref().and_then(|text| Some((text, look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?))) {
        Some((text, face)) => fitted_words(face, &trp_data::interface::screen::TextLook { wraps: false, overflow: 1, ..text.clone() }, name, room),
        None => name.to_owned(),
    };
    let bar = panel.placed(window.caption_panel, size).rect;
    let close = panel.placed(window.close, size).rect;
    let mut search_hits = Vec::new();
    let mut search_spinners = Vec::new();
    if let Some(search) = search {
        // The button takes no click while it searches (`SearchButton.
        // SetSearchStatus`: `interactable = !active`), and the window has no
        // stop button: only shutting the window stops its search.
        for (path, hit, on) in [
            (WINDOW_UNSEARCHED, SearchHit::Begin, search.unsearched && !search.searching && search.can_start),
            (WINDOW_SEARCH_BUTTON, SearchHit::Begin, search.button && !search.searching && search.can_start),
        ] {
            if let (Some(index), true) = (panel.find(path), on) {
                search_hits.push((hit, panel.placed(index, size).rect));
            }
        }
        for (path, on) in [(WINDOW_SEARCH_LOADER, search.searching && search.button), (WINDOW_UNSEARCHED_LOADER, search.searching && search.unsearched)] {
            if let (Some(index), true) = (panel.find(path), on) {
                search_spinners.push((path, panel.placed(index, size).rect));
            }
        }
    }
    Some(GridWindowBuilt { panel, size, grids, caption, bar, close, picked, search_set, search_words, search_hits, search_spinners })
}

/// The window of a container's grids drawn, the `index`th window, at its
/// top left corner: its frame, shadow and caption's row as the prefab has
/// them, its icon and caption's ground in the colours of a priority window
/// or not (`SetSelectedAsTargetVisual`), and each grid's view; the things
/// in the grids are the caller's.
pub(crate) fn draw_grid_window(column: &mut ChildSpawnerCommands, look: &HealthLook, said: &Words, built: &GridWindowBuilt, index: usize, s: f32) {
    let Some(window) = look.grid_window.as_ref() else { return };
    let mut words: HashMap<usize, String> = [(window.caption, built.caption.clone())].into();
    words.extend(built.search_words.iter().cloned());
    let pick = usize::from(built.picked);
    let colors: HashMap<usize, Color> = [(window.icon, rgba(window.icon_colors[pick])), (window.caption_panel, rgba(window.header_colors[pick]))].into();
    let marks = Marks { words: Some(&words), colors: Some(&colors), set: Some(&built.search_set), ..Marks::default() };
    draw_nodes(column, look, &built.panel, &look.grid_window_drawn, GRID_WINDOW + (index << 12), marks, said, Frame::Is(built.size), [0.0, 0.0], s);
    for [x, y, w, h] in &built.grids {
        draw_grid_view(column, look, said, [*x, *y], [*w, *h], s);
    }
}
