//! Placing a panel's nodes: sizes, rows and what each node asks for.

use super::*;

impl ScenePanel {
    /// Where an object is drawn with the top that size, by the engine's
    /// layout: each object's rectangle laid out in its parent's, unscaled,
    /// then scaled about its pivot with all under it.
    pub fn placed(&self, index: usize, size: [f32; 2]) -> Placed {
        // The object's own rectangle in its own frame, and that frame's
        // origin and scale in the top's.
        fn frame(panel: &ScenePanel, index: usize, size: [f32; 2]) -> (Rect, [f32; 2], [f32; 2]) {
            let Some(parent) = panel.nodes[index].parent else { return (Rect { at: [0.0, 0.0], size }, [0.0, 0.0], [1.0, 1.0]) };
            let (outer, origin, scale) = frame(panel, parent, size);
            let layout = panel.nodes[index].layout;
            // Laid out by its parent's row while the game runs, or by its
            // own anchors.
            let own = match panel.in_row(parent, index, outer) {
                Some(own) => own,
                None => {
                    // Sized to what it holds where a fitter says so and it
                    // lays out what it holds (`ContentSizeFitter`: least 1,
                    // preferred 2), about its pivot.
                    let mut own = place(Layout { scale: [1.0, 1.0], ..layout }, outer);
                    if let (Some(_), Some(fitter)) = (panel.nodes[index].group, panel.script(index, "ContentSizeFitter")) {
                        for axis in 0..2 {
                            let mode = fitter.get(4 * axis..4 * axis + 4).map_or(0, |word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
                            if mode == 0 {
                                continue;
                            }
                            let (least, preferred, _) = panel.asks(index, axis);
                            let fitted = if mode == 1 { least } else { preferred };
                            let pivot = own.at[axis] + layout.pivot[axis] * own.size[axis];
                            own.size[axis] = fitted;
                            own.at[axis] = pivot - layout.pivot[axis] * fitted;
                        }
                    }
                    own
                }
            };
            let pivot = [own.at[0] + layout.pivot[0] * own.size[0], own.at[1] + layout.pivot[1] * own.size[1]];
            let local = Rect { at: [-layout.pivot[0] * own.size[0], -layout.pivot[1] * own.size[1]], size: own.size };
            let origin = [origin[0] + scale[0] * pivot[0], origin[1] + scale[1] * pivot[1]];
            (local, origin, [scale[0] * layout.scale[0], scale[1] * layout.scale[1]])
        }
        let (local, origin, scale) = frame(self, index, size);
        let corner = |axis: usize, at: f32| origin[axis] + scale[axis] * at;
        let (x0, x1) = (corner(0, local.at[0]), corner(0, local.at[0] + local.size[0]));
        let (y0, y1) = (corner(1, local.at[1]), corner(1, local.at[1] + local.size[1]));
        let (left, right, bottom, top) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
        Placed { rect: [left, size[1] - top, right - left, top - bottom], flip: [scale[0] < 0.0, scale[1] < 0.0] }
    }

    /// An object's rectangle with the top that size: its left, top, width
    /// and height from the top's top left corner, y down.
    pub fn rect(&self, index: usize, size: [f32; 2]) -> [f32; 4] {
        self.placed(index, size).rect
    }

    /// Where an object is drawn with the top laid out by its own anchors in
    /// what it hangs from, of that size: from that one's top left corner.
    pub fn placed_in(&self, index: usize, outer: [f32; 2]) -> Placed {
        let top = place(self.nodes[0].layout, Rect { at: [0.0, 0.0], size: outer });
        let mut placed = self.placed(index, top.size);
        placed.rect[0] += top.at[0];
        placed.rect[1] += outer[1] - top.at[1] - top.size[1];
        placed
    }

    /// The file a pointer of the panel's leads into, by the number it
    /// gives it: its own file for 0, else one of its outside files.
    pub fn file_of(&self, outside: i32) -> Option<String> {
        match outside {
            0 => Some(self.file.clone()).filter(|file| !file.is_empty()),
            n => self.externals.get(n as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()),
        }
    }

    /// An object's own size along an axis where no group sets it: what its
    /// fitter makes it where it has one for that axis and lays out what it
    /// holds (`ContentSizeFitter`: least 1, preferred 2; a fitter sizes its
    /// object in a group that leaves that axis to it, as the quick-use
    /// panel's header in its column: its words and its padding, where it
    /// was kept at its saved width), else the size it is kept at.
    fn own_size(&self, index: usize, axis: usize) -> f32 {
        let node = &self.nodes[index];
        if let (Some(_), Some(fitter)) = (node.group, self.script(index, "ContentSizeFitter")) {
            let mode = fitter.get(4 * axis..4 * axis + 4).map_or(0, |word| u32::from_le_bytes(word.try_into().unwrap_or_default()));
            if mode != 0 {
                let (least, preferred, _) = self.asks(index, axis);
                return if mode == 1 { least } else { preferred };
            }
        }
        node.layout.size_delta[axis]
    }

    /// What an object asks of a layout along an axis (`LayoutUtility`): its
    /// least, preferred and flexible sizes; from its own layout group's
    /// children where it has a group (`CalcAlongAxis`: along the group
    /// their sum with the spacing, across it the largest, and the padding),
    /// and as its `LayoutElement` says where that says; each preferred at
    /// least its least. A picture's or a text's own sizes are not had: none.
    pub fn asks(&self, index: usize, axis: usize) -> (f32, f32, f32) {
        let node = &self.nodes[index];
        let (mut least, mut preferred, mut flexible) = (0.0f32, 0.0f32, 0.0f32);
        if let Some(group) = node.group {
            let main = if group.vertical { 1 } else { 0 };
            let laid: Vec<usize> = (0..self.nodes.len())
                .filter(|&child| self.nodes[child].parent == Some(index) && self.nodes[child].active && !self.nodes[child].element.is_some_and(|e| e.ignore))
                .collect();
            for (n, &child) in laid.iter().enumerate() {
                let (l, p, f) = if group.control[axis] {
                    let (l, p, f) = self.asks(child, axis);
                    (l, p, if group.expand[axis] { f.max(1.0) } else { f })
                } else {
                    let own = self.own_size(child, axis);
                    (own, own, 0.0)
                };
                if axis == main {
                    let gap = if n > 0 { group.spacing } else { 0.0 };
                    least += l + gap;
                    preferred += p + gap;
                    flexible += f;
                } else {
                    least = least.max(l);
                    preferred = preferred.max(p);
                    flexible = flexible.max(f);
                }
            }
            let pad = if axis == 0 { group.padding[0] + group.padding[1] } else { group.padding[2] + group.padding[3] };
            least += pad;
            preferred += pad;
        }
        if let Some(e) = node.element {
            if e.min[axis] >= 0.0 {
                least = e.min[axis];
            }
            if e.preferred[axis] >= 0.0 {
                preferred = e.preferred[axis];
            }
            if e.flexible[axis] >= 0.0 {
                flexible = e.flexible[axis];
            }
        }
        (least, preferred.max(least), flexible)
    }

    /// A child's own rectangle in its parent's as the parent's row or
    /// column lays it out (`HorizontalOrVerticalLayoutGroup`: `CalcAlongAxis`
    /// and `SetChildrenAlongAxis`), the children that are on and not left
    /// out in their order: along the group, each from its least towards its
    /// preferred size as far as the group's length allows, and what is left
    /// over shared by how much each may grow, from the start (left, or the
    /// top); across it, the group's breadth within each's least and, unless
    /// it may grow, preferred. None where the parent has no group.
    fn in_row(&self, parent: usize, index: usize, outer: Rect) -> Option<Rect> {
        let group = self.nodes[parent].group?;
        let (main, cross) = if group.vertical { (1, 0) } else { (0, 1) };
        let laid: Vec<usize> = (0..self.nodes.len())
            .filter(|&child| self.nodes[child].parent == Some(parent) && self.nodes[child].active && !self.nodes[child].element.is_some_and(|e| e.ignore))
            .collect();
        let at = laid.iter().position(|&child| child == index)?;
        let sizes = |child: usize, axis: usize| -> (f32, f32, f32) {
            if !group.control[axis] {
                let own = self.own_size(child, axis);
                return (own, own, 0.0);
            }
            let (least, preferred, flexible) = self.asks(child, axis);
            (least, preferred, if group.expand[axis] { flexible.max(1.0) } else { flexible })
        };
        let [left, right, top, bottom] = group.padding;
        // Each axis's padding before and after, and how its surplus is
        // shared out by the group's corner (`childAlignment`).
        let pad = |axis: usize| if axis == 0 { (left, right) } else { (top, bottom) };
        let align = |axis: usize| if axis == 0 { (group.alignment % 3) as f32 * 0.5 } else { (group.alignment / 3) as f32 * 0.5 };
        let start = |axis: usize, needed: f32| {
            let (before, after) = pad(axis);
            before + (outer.size[axis] - needed - before - after) * align(axis)
        };
        // Along the group.
        let (before, after) = pad(main);
        let (mut least, mut preferred, mut flexible) = (before + after, before + after, 0.0);
        for &child in &laid {
            let (l, p, f) = sizes(child, main);
            least += l + group.spacing;
            preferred += p + group.spacing;
            flexible += f;
        }
        if !laid.is_empty() {
            least -= group.spacing;
            preferred -= group.spacing;
        }
        let preferred = preferred.max(least);
        let (mut position, mut share) = (before, 0.0);
        let surplus = outer.size[main] - preferred;
        if surplus > 0.0 {
            if flexible == 0.0 {
                position = start(main, preferred - before - after);
            } else {
                share = surplus / flexible;
            }
        }
        let towards = if least != preferred { ((outer.size[main] - least) / (preferred - least)).clamp(0.0, 1.0) } else { 0.0 };
        let mut along = (0.0, 0.0);
        for (n, &child) in laid.iter().enumerate() {
            let (l, p, f) = sizes(child, main);
            let size = l + (p - l) * towards + f * share;
            if n == at {
                let own = self.own_size(child, main);
                along = if group.control[main] { (position, size) } else { (position + (size - own) * align(main), own) };
            }
            position += size + group.spacing;
        }
        // Across it.
        let (l, p, f) = sizes(index, cross);
        let (before, after) = pad(cross);
        let inner = outer.size[cross] - before - after;
        let most = if f > 0.0 { outer.size[cross] } else { p };
        let needed = if inner < l { l } else if inner > most { most } else { inner };
        let from = start(cross, needed);
        let own = self.own_size(index, cross);
        let across = if group.control[cross] { (from, needed) } else { (from + (needed - own) * align(cross), own) };
        // From the top left, y down; then as the engine keeps it, y up.
        let ((x, width), (y, height)) = if main == 0 { (along, across) } else { (across, along) };
        Some(Rect { at: [outer.at[0] + x, outer.at[1] + outer.size[1] - y - height], size: [width, height] })
    }
}
