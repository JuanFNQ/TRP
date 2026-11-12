//! The sight panels and their details.

use super::*;

/// The fields every element of the game's screens begins its own with
/// (`UIElement`: its disposables and the rest, all empty in a scene), in
/// bytes.
pub(super) const UI_ELEMENT: usize = 40;

/// One of the battle screen's two panels of a sight (optics spec,
/// "Zeroing", the HUD): where it stands (its own layout, on the screen of
/// 1920 by 1080), its ground's colour, its row's padding (left, right, top,
/// bottom), and its words' look (its script's text: `AmmoCountPanel.
/// _ammoCount`, `AnimatedTextPanel._currentCrate`) with the name of the file
/// its font asset is in.
#[derive(Debug, Clone)]
pub struct SightPanel {
    pub layout: Layout,
    pub ground: [f32; 4],
    pub padding: [f32; 4],
    pub text: TextLook,
    pub font_file: Option<String>,
    /// The ammunition's panel's details line (`AmmoCountPanel._ammoDetails`,
    /// the round's name under a look at the magazine), where it has one.
    pub details: Option<SightDetails>,
}

/// A panel's details line: its words' look and their font asset's file,
/// its own layout (it hangs from the words), and its icon (`AmmoImage`):
/// the sprite's file and number and the icon's layout, which hangs from
/// the line.
#[derive(Debug, Clone)]
pub struct SightDetails {
    pub text: TextLook,
    pub font_file: Option<String>,
    pub layout: Layout,
    pub icon: Option<(String, i64, Layout)>,
}

/// The ammunition's panel (`AmmoPanel`, whose words a zeroing distance, a
/// fire mode and a look at the magazine set) and the optic's
/// (`OpticCratePanel`, a magnification's), as the battle screen has them.
#[derive(Debug, Clone)]
pub struct SightPanels {
    pub ammo: SightPanel,
    pub optic: SightPanel,
    /// The tactical devices' activation mode's (`TacticalInteractionPanel`,
    /// an `AnimatedTextPanel`: "Press" or "Hold" as T+B switches it), where
    /// the scene has it.
    pub tactical: Option<SightPanel>,
}

impl SightPanels {
    /// Reads them from the scene that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const SCREEN: &str = "/EFTBattleUIScreen Variant";
        let scene = Scene::load(install, file, true)?;
        let panel_of = |name: &str, class: &str| -> Result<SightPanel> {
            let end = format!("{SCREEN}/{name}");
            let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(&end)).with_context(|| format!("{file} has no {name}"))?;
            let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
            let fields = panel.script(0, class).with_context(|| format!("{name}'s {class}"))?;
            let words = panel.pointer_at(fields, UI_ELEMENT).with_context(|| format!("{name}'s words"))?;
            let text = panel.nodes[words].text.clone().with_context(|| format!("{name}'s words have no text"))?;
            let font_file = panel.file_of(text.font.0);
            // The second pointer, where the class has one: the details.
            let details = (class == "AmmoCountPanel").then(|| panel.pointer_at(fields, UI_ELEMENT + 12)).flatten().and_then(|line| {
                let text = panel.nodes[line].text.clone()?;
                let icon = (0..panel.nodes.len()).find(|&i| panel.nodes[i].parent == Some(line) && panel.nodes[i].image.is_some_and(|image| image.sprite.is_some())).and_then(|i| {
                    let (file, id) = panel.nodes[i].image?.sprite?;
                    Some((panel.file_of(file)?, id, panel.nodes[i].layout))
                });
                Some(SightDetails { font_file: panel.file_of(text.font.0), text, layout: panel.nodes[line].layout, icon })
            });
            Ok(SightPanel {
                layout: panel.nodes[0].layout,
                ground: panel.nodes[0].image.map_or([0.0; 4], |image| image.color),
                padding: panel.nodes[0].group.map_or([0.0; 4], |group| group.padding),
                text,
                font_file,
                details,
            })
        };
        Ok(Self { ammo: panel_of("AmmoPanel", "AmmoCountPanel")?, optic: panel_of("OpticCratePanel", "AnimatedTextPanel")?, tactical: panel_of("TacticalInteractionPanel", "AnimatedTextPanel").ok() })
    }
}
