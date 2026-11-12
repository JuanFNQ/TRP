//! What a sound is: the told and heard, the screen's sounds and their ways.

use super::*;

/// A sound played, as the game's recorder writes the interface's
/// (`hosts\spt-oracle\Interface.cs`): the interface's by its
/// `EUISoundType`, a thing's by its group and way, and each clip played
/// (`GUISounds.PlaySound`: its name and whether it is a common sound, a
/// thing's or a round's; never `single`, its volume always 1).
#[derive(Clone, Debug, PartialEq)]
pub enum Told {
    Ui(&'static str),
    Item(String, &'static str),
    Clip(String, bool),
}

/// The sounds played on this frame, as told, for the interface's dump
/// (`ui_dump`, which adds it and takes them); kept nowhere else.
#[derive(Resource, Default)]
pub struct Heard(pub Vec<Told>);

/// The interface's sounds the app plays, by the game's `EUISoundType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ui {
    BackpackClose = 0,
    BackpackOpen = 1,
    ButtonClick = 3,
    ButtonOver = 4,
    /// A box of the settings ticked or not (`UpdatableToggle.UpdateValue`).
    MenuCheckBox = 9,
    MenuDropdown = 11,
    MenuDropdownSelect = 12,
    MenuEscape = 13,
    InspectorClose = 14,
    InspectorOpen = 15,
    InstallMag = 16,
    InstallModFunc = 17,
    InstallModGear = 18,
    InstallModVital = 19,
    MenuModdingClose = 21,
    MenuModdingOpen = 22,
    OpenContainer = 23,
    MenuStock = 24,
    /// A repair made (`RepairComplete`, the repair window's).
    RepairComplete = 31,
    MenuWeaponAssemble = 33,
    MenuWeaponDisassemble = 34,
    /// A malfunction made known (`MalfunctionExamined`, with its glow).
    MalfunctionExamined = 40,
}

impl Ui {
    pub const ALL: [Ui; 22] = [
        Ui::RepairComplete,
        Ui::MalfunctionExamined,
        Ui::BackpackClose,
        Ui::BackpackOpen,
        Ui::ButtonClick,
        Ui::ButtonOver,
        Ui::MenuCheckBox,
        Ui::MenuDropdown,
        Ui::MenuDropdownSelect,
        Ui::MenuEscape,
        Ui::InspectorClose,
        Ui::InspectorOpen,
        Ui::InstallMag,
        Ui::InstallModFunc,
        Ui::InstallModGear,
        Ui::InstallModVital,
        Ui::OpenContainer,
        Ui::MenuStock,
        Ui::MenuWeaponDisassemble,
        Ui::MenuModdingClose,
        Ui::MenuModdingOpen,
        Ui::MenuWeaponAssemble,
    ];

    /// Its name in the game (`EUISoundType`).
    pub fn game_name(self) -> &'static str {
        match self {
            Ui::BackpackClose => "BackpackClose",
            Ui::BackpackOpen => "BackpackOpen",
            Ui::ButtonClick => "ButtonClick",
            Ui::ButtonOver => "ButtonOver",
            Ui::MenuCheckBox => "MenuCheckBox",
            Ui::MenuDropdown => "MenuDropdown",
            Ui::MenuDropdownSelect => "MenuDropdownSelect",
            Ui::MenuEscape => "MenuEscape",
            Ui::InspectorClose => "MenuInspectorWindowClose",
            Ui::InspectorOpen => "MenuInspectorWindowOpen",
            Ui::InstallMag => "MenuInstallMag",
            Ui::InstallModFunc => "MenuInstallModFunc",
            Ui::InstallModGear => "MenuInstallModGear",
            Ui::InstallModVital => "MenuInstallModVital",
            Ui::MenuModdingClose => "MenuModdingClose",
            Ui::MenuModdingOpen => "MenuModdingOpen",
            Ui::OpenContainer => "MenuOpenContainer",
            Ui::MenuStock => "MenuStock",
            Ui::RepairComplete => "RepairComplete",
            Ui::MenuWeaponAssemble => "MenuWeaponAssemble",
            Ui::MenuWeaponDisassemble => "MenuWeaponDisassemble",
            Ui::MalfunctionExamined => "MalfunctionExamined",
        }
    }
}

/// A way a thing is handled, which names its clip after its group
/// (`EInventorySoundType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Drop,
    Pickup,
    Use,
    /// A container opened in a window: its group's own clip if it has
    /// one, at the interface's volume (`GUISounds.PlaySound`), else the
    /// interface's `MenuOpenContainer` (`ItemUiContext.OpenItem`).
    Open,
    /// Anything else (`EInventorySoundType.other`: a search's luck,
    /// `looting_luck2_other`).
    Other,
}

impl Way {
    pub(super) fn name(self) -> &'static str {
        match self {
            Way::Drop => "drop",
            Way::Pickup => "pickup",
            Way::Use => "use",
            Way::Open => "open",
            Way::Other => "other",
        }
    }
}

/// A sound asked for: the interface's, a thing's by its kind's group, or a
/// round loaded into a magazine or unloaded from it (`PlayUILoadSound`,
/// `PlayUIUnloadSound`: one of the interface's lists for each).
#[derive(Message, Clone, Debug)]
pub enum Sound {
    Ui(Ui),
    Item(String, Way),
    Load,
    Unload,
    /// A thing taken up off the ground, by its kind's group (sound spec,
    /// "A thing taken up"; `LootItem.RemoveLootItem`): its group's pickup
    /// clip from where it lay, on a source of the `Character` group with
    /// its spatialising off, so plain stereo (`forceStereo`), at a volume
    /// of 1 (the group's `OverallVolume` 1), within 10 m: in the world's
    /// group, not the interface's.
    Taken(String),
    /// A notice shown (`GUISounds.PlayNotificationSound`), at the
    /// interface's volume.
    Notification,
    /// A thing's clip by its own name, looping from now in the world's
    /// group (`Group::CommonSounds`, the `Character` source group's) at the overall
    /// volume times its own, following its own level besides, until its
    /// cut (a search's sound: `Player.InventoryControllerOnSearchEvent`).
    Looping(String, f32, Cut, Level),
}
