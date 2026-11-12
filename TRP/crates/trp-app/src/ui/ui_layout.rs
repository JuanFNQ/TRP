//! The interface laid out only on a frame where something it is laid out
//! from has changed. The engine's layout (`bevy_ui`'s `ui_layout_system`,
//! alone in `UiSystems::Layout`) otherwise goes over every node of every
//! screen on every frame, changed or not: with the inventory open, some
//! 2,000 nodes, 0.4 ms of a 4.2 ms frame for a screen that stood still.
//!
//! What the layout reads (Bevy 0.18.1): each node's `Node`, its
//! `ContentSize` (a text's or a picture's measure) and its render target
//! (`ComputedUiRenderTargetInfo`: the window's size and scale, and the
//! interface's scale); the hierarchy (a node's `Children`, nodes added or
//! removed); and, to place each node, its `UiTransform`, `LayoutConfig`,
//! `Outline`, `ScrollPosition` and `IgnoreScroll`. Run on a frame where
//! none of these changed it gives what it gave the frame before, so that
//! frame keeps the last one's sizes and places. When it does run it sees
//! every change since its last run, however many frames it was skipped:
//! a system that a condition skips keeps the tick of its own last run.
//!
//! With `TARKOV UI`, `health_view::texts_on_pixels` sets each text on
//! whole pixels after the layout; the layout put them back on its
//! fractions every frame and they were set again. Now they stay set until
//! the layout next runs, and are set again then: drawn the same.
//!
//! `TRP_LAYOUT_EVERY_FRAME` in the environment lays out every frame, as
//! before, to set the two side by side.

use bevy::prelude::*;
use bevy::ui::{ComputedUiRenderTargetInfo, ContentSize, IgnoreScroll, LayoutConfig, UiSystems};

pub struct UiLayoutPlugin;

impl Plugin for UiLayoutPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("TRP_LAYOUT_EVERY_FRAME").is_some() {
            info!("the interface laid out every frame (TRP_LAYOUT_EVERY_FRAME)");
            return;
        }
        app.configure_sets(PostUpdate, UiSystems::Layout.run_if(wanted));
    }
}

/// Whether anything the layout reads has changed since this was last
/// asked (each frame, as the layout's turn comes, after the texts and
/// pictures are measured). Every removal is read, so none is counted
/// again on the next frame.
#[allow(clippy::type_complexity)]
fn wanted(
    changed: Query<
        (),
        Or<(
            Changed<Node>,
            Changed<ContentSize>,
            Changed<ComputedUiRenderTargetInfo>,
            Changed<UiTransform>,
            Changed<LayoutConfig>,
            Changed<Outline>,
            Changed<ScrollPosition>,
            Changed<IgnoreScroll>,
        )>,
    >,
    children: Query<(), (Changed<Children>, With<Node>)>,
    nodes: Query<(), With<Node>>,
    mut removed_nodes: RemovedComponents<Node>,
    mut removed_sizes: RemovedComponents<ContentSize>,
    mut removed_children: RemovedComponents<Children>,
) -> bool {
    let removed = removed_nodes.read().count() + removed_sizes.read().count() + removed_children.read().filter(|entity| nodes.contains(*entity)).count();
    removed > 0 || !changed.is_empty() || !children.is_empty()
}
