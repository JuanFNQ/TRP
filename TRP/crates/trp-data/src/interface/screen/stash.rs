//! The stash views, complex parts and filter tabs.

use super::*;

/// The inventory's panel on the right for a container opened in a raid
/// (`Stash Panel`'s `Simple Panel`, `SimpleStashPanel`; a corpse's gear
/// has another, `Complex Loot Panel`), read from the scene with what its
/// scripts name: its header (`_header`) and the text that says what it
/// holds (`_simpleGridName`), its column of filters (`_filterPanel`) and
/// their tabs, the money's count, the container's name in its box
/// (`_containerNamePanel`, `_containerName`), what is only in the lobby
/// (`_lobbyOnlyElements`), the sorting, the sorting table's and the
/// search's tabs, its scroll view (`_stashScroll`) and how it scrolls, the
/// mark of what is had from outside the raid, the pin, lock and priority
/// buttons; and where the container's grids are put (its
/// `SearchableItemView._gridsContainer`). The panel's top is `Stash
/// Panel`.
#[derive(Debug, Clone, Default)]
pub struct StashViews {
    pub panel: ScenePanel,
    pub simple: usize,
    pub header: Option<usize>,
    pub grid_name: Option<usize>,
    pub filters: Option<usize>,
    pub filter_tabs: Vec<FilterTabParts>,
    pub money: Option<usize>,
    pub name_panel: Option<usize>,
    pub name: Option<usize>,
    pub lobby_only: Vec<usize>,
    pub sort: Option<usize>,
    pub sorting_table: Option<usize>,
    pub search: Option<usize>,
    pub scroll: Option<usize>,
    pub external: Option<usize>,
    pub pin: Option<usize>,
    pub lock: Option<usize>,
    pub priority: Option<usize>,
    pub grids: Option<usize>,
    pub scrolling: Option<Scrolling>,
    /// The panel for a corpse's gear (`ComplexStashPanel`), where it is
    /// read.
    pub complex: Option<ComplexParts>,
    /// The search button's tints (`SearchButton`, a `Button`: its
    /// `Selectable` colours) and what it tints (its ground), where they
    /// are the tint.
    pub search_tints: Option<(Tints, Option<usize>)>,
}

/// The parts of the panel for a corpse's gear (`ComplexStashPanel`, by its
/// fields' order after Odin's data: `_complexGridName`, `_lootPanel`,
/// `_containersPanel`, `_equipmentPanelSource`, `_complexPanel`,
/// `_containerNamePanel`, `_containerName`): the panel, its header's toggle
/// and the header's words, the column its gear and its containers are laid
/// in (the scroll view's content), the scroll view (the containers' panel,
/// whose mask cuts them), the name's box and its words, and the scroll
/// view's scrolling.
#[derive(Debug, Clone, Copy)]
pub struct ComplexParts {
    pub panel: usize,
    pub header: Option<usize>,
    pub grid_name: Option<usize>,
    pub content: Option<usize>,
    pub scroll: Option<usize>,
    pub name_panel: Option<usize>,
    pub name: Option<usize>,
    pub scrolling: Option<Scrolling>,
}

impl ComplexParts {
    pub(super) fn read(panel: &ScenePanel) -> Option<Self> {
        let at = (0..panel.nodes.len()).find(|&index| panel.script(index, "ComplexStashPanel").is_some())?;
        let fields = panel.script(at, "ComplexStashPanel")?;
        let scroll = panel.pointer_at(fields, 64);
        Some(Self {
            panel: at,
            header: panel.find(&format!("{}/Header", panel.nodes[at].path)),
            grid_name: panel.pointer_at(fields, 40),
            content: panel.transform_at(fields, 52),
            scroll,
            name_panel: panel.pointer_at(fields, 100),
            name: panel.pointer_at(fields, 112),
            scrolling: scroll.and_then(|scroll| Scrolling::of(panel, scroll, "ScrollRectNoDrag")),
        })
    }
}

/// A filter's tab (`FilterTab`, a `Tab`): its versions idle and chosen,
/// the picture its working or not is shown on (`_targetImage`), and its
/// icon (`_filterIcon`), both drawn at 0.15 of their alpha when it does
/// not work (`SetInteractable`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterTabParts {
    pub object: usize,
    pub normal: Option<usize>,
    pub selected: Option<usize>,
    pub target: Option<usize>,
    pub icon: Option<usize>,
}

impl StashViews {
    /// Reads it from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const PANEL: &str = "/InventoryScreen/Items Panel/Stash Panel";
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(PANEL)).with_context(|| format!("{file} has no stash panel"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let simple = (0..panel.nodes.len()).find(|&index| panel.script(index, "SimpleStashPanel").is_some()).context("the stash panel has no simple panel")?;
        let fields = panel.script(simple, "SimpleStashPanel").context("the simple panel's script")?;
        // Odin's data and the input node's own field, then the pointers.
        let mut r = Reader::new(fields);
        skip_odin(&mut r)?;
        r.skip(4)?;
        let mut at = fields.len() - r.remaining();
        let mut next = || {
            let found = panel.pointer_at(fields, at);
            at += 12;
            found
        };
        let (header, _simple_view, grid_name, filters, money, name_panel, name) = (next(), next(), next(), next(), next(), next(), next());
        let count = u32::from_le_bytes(fields.get(at..at + 4).context("the lobby's list")?.try_into()?) as usize;
        at += 4;
        let lobby_only = (0..count).filter_map(|n| panel.pointer_at(fields, at + 12 * n)).collect();
        at += 12 * count;
        let mut next = || {
            let found = panel.pointer_at(fields, at);
            at += 12;
            found
        };
        let (sort, sorting_table, search, scroll, external, pin, lock, priority) = (next(), next(), next(), next(), next(), next(), next(), next());
        // Where the grids go: the simple panel's `SearchableItemView`, after
        // Odin's data, its searchable view and its grids' template.
        let grids = panel.script(simple, "SearchableItemView").and_then(|fields| {
            let mut r = Reader::new(fields);
            skip_odin(&mut r).ok()?;
            panel.transform_at(fields, fields.len() - r.remaining() + 24)
        });
        // The filters' tabs, in their order (`_filterTabs`).
        let filter_tabs = filters
            .and_then(|filters| panel.script(filters, "FilterPanel"))
            .and_then(|fields| {
                let mut r = Reader::new(fields);
                skip_odin(&mut r).ok()?;
                let count = r.u32().ok()? as usize;
                let at = fields.len() - r.remaining();
                Some(
                    (0..count)
                        .filter_map(|n| panel.pointer_at(fields, at + 12 * n))
                        .map(|object| {
                            let tab = panel.script(object, "FilterTab").unwrap_or_default();
                            FilterTabParts {
                                object,
                                normal: panel.pointer_at(tab, 0),
                                selected: panel.pointer_at(tab, 12),
                                target: panel.pointer_at(tab, 36),
                                icon: tab.len().checked_sub(12).and_then(|end| panel.pointer_at(tab, end)),
                            }
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
        let scrolling = scroll.and_then(|scroll| Scrolling::of(&panel, scroll, "ScrollRectNoDrag"));
        header.context("the simple panel's script names no header")?;
        let complex = ComplexParts::read(&panel);
        let search_tints = panel.find("Simple Panel/TopPanel/SearchPanel/Search Button").and_then(|button| control_tints(&panel, button, "SearchButton"));
        Ok(Self { panel, simple, header, grid_name, filters, filter_tabs, money, name_panel, name, lobby_only, sort, sorting_table, search, scroll, external, pin, lock, priority, grids, scrolling, complex, search_tints })
    }
}
