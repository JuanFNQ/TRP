//! Rigs' layouts and their grids.

use super::*;

/// A container's own layout of its grids (`TemplatedGridsView`, the
/// Resource `UI/Rig Layouts/<its layout's name>`): the prefab, and its grid
/// views in the order the container's grids take them (`_presetGridViews`).
pub fn rig_layout(install: &Install, name: &str) -> Result<(ScenePanel, Vec<usize>)> {
    let wanted = format!("ui/rig layouts/{}", name.to_lowercase());
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| *path == wanted).with_context(|| format!("no resource is at {wanted}"))?;
    let scene = Scene::load_prefab(install, file, *id)?;
    let panel = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) };
    let views = layout_grids(&panel).context("the layout's script names no grid views")?;
    Ok((panel, views))
}

/// A layout's grid views in their order (`TemplatedGridsView.
/// _presetGridViews`: a count, then so many pointers to grid views).
fn layout_grids(panel: &ScenePanel) -> Option<Vec<usize>> {
    let fields = panel.script(0, "TemplatedGridsView")?;
    let is_grid = |index: Option<usize>| index.is_some_and(|index| panel.nodes[index].scripts.iter().any(|(class, _)| class == "GridView"));
    (0..fields.len().saturating_sub(4)).step_by(4).find_map(|start| {
        let count = u32::from_le_bytes(fields.get(start..start + 4)?.try_into().ok()?) as usize;
        if count == 0 || count > 64 {
            return None;
        }
        let views: Vec<Option<usize>> = (0..count).map(|n| panel.pointer_at(fields, start + 4 + 12 * n)).collect();
        views.iter().all(|&view| is_grid(view)).then(|| views.into_iter().flatten().collect::<Vec<usize>>())
    })
}

/// Containers' own layouts of their grids by name (`rig_layout`), read at
/// once: each with its size and its grids' rectangles (left, top, width,
/// height, y down) in their order. A name with no layout is left out.
pub fn rig_layouts(install: &Install, names: &[String]) -> HashMap<String, ([f32; 2], Vec<LayoutGrid>)> {
    let Ok(paths) = resource_paths(install) else { return HashMap::new() };
    let mut open: HashMap<String, BuildFile> = HashMap::new();
    let mut layouts = HashMap::new();
    for name in names {
        let wanted = format!("ui/rig layouts/{}", name.to_lowercase());
        let Some((_, file, id)) = paths.iter().find(|(path, ..)| *path == wanted) else { continue };
        if !open.contains_key(file) {
            let Ok(build) = BuildFile::open(install, file) else { continue };
            open.insert(file.clone(), build);
        }
        let Some(build) = open.get_mut(file) else { continue };
        let Ok(scene) = Scene::load_prefab_from(install, build, *id) else { continue };
        let panel = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, 0) };
        let Some(views) = layout_grids(&panel) else { continue };
        let size = panel.nodes[0].layout.size_delta;
        let grids = views.iter().map(|&view| LayoutGrid { rect: panel.rect(view, size), size_delta: panel.nodes[view].layout.size_delta, pivot: panel.nodes[view].layout.pivot }).collect();
        layouts.insert(name.clone(), (size, grids));
    }
    layouts
}

/// A grid's view in a container's own layout: its rectangle as the prefab
/// lays it out (left, top, width, height, the screen's y down), and the
/// size and pivot its own transform keeps, by which the game sizes it to
/// its grid's cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutGrid {
    pub rect: [f32; 4],
    pub size_delta: [f32; 2],
    pub pivot: [f32; 2],
}

impl LayoutGrid {
    /// The view sized to a grid of so many cells (`GridView.OnGridResized`:
    /// its `sizeDelta` set to 63 units a cell and one more,
    /// `ItemViewFactory.GetCellPixelSize`), about its pivot, which stays
    /// where it was whatever its anchors: the Rush 100's main grid, 320 by
    /// 448 in its layout, is 316 by 442 for its 5 by 7 cells.
    pub fn sized(&self, cells: [u32; 2]) -> [f32; 4] {
        let [x, y, w, h] = self.rect;
        let to = [cells[0] as f32 * 63.0 + 1.0, cells[1] as f32 * 63.0 + 1.0];
        let less = [self.size_delta[0] - to[0], self.size_delta[1] - to[1]];
        [x + self.pivot[0] * less[0], y + (1.0 - self.pivot[1]) * less[1], w - less[0], h - less[1]]
    }
}
