//! Item kinds, the places items go and the refusals of a move.

use super::*;

/// A kind of item: what the database says of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Kind {
    pub template: String,
    /// The database's own name for it (not the game's localised one).
    pub name: String,
    /// Its template and the classes it belongs to, nearest first.
    pub classes: Vec<String>,
    /// The database's names for the same, nearest first.
    pub class_names: Vec<String>,
    /// In kilograms, for one.
    pub weight: f32,
    /// Width and height in cells.
    pub cells: [u32; 2],
    /// The cells it adds to what it is a part of: left, right, up, down.
    pub extra: [u32; 4],
    /// Whether those cells are added beside its fellow parts' rather than
    /// only where they reach further (`ExtraSizeForceAdd`).
    pub forced_extra: bool,
    /// Whether its parts' extra cells count toward its size.
    pub merges: bool,
    pub slots: Vec<Holder>,
    pub grids: Vec<Holder>,
    /// What a magazine's rounds go into, with what it takes (they lie in
    /// `Item::loose`); none for anything else.
    pub cartridges: Vec<Holder>,
    /// The equipment slots it blocks while worn.
    pub blocks: Vec<String>,
    /// How many of it one stack holds; 0 or 1 for what does not stack.
    pub stack_max: u32,
    /// A part that may not be put on or taken off in a raid.
    pub not_raid_moddable: bool,
    /// The equipment slots it is not taken out of in a raid.
    pub kept_in: Vec<String>,
    /// The colour of its view's ground in the inventory, by the game's name
    /// for it (`BackgroundColor`); empty where not read.
    pub color: String,
    /// Its own layout of its grids in the inventory, by name; empty for
    /// the default row.
    pub layout: String,
    /// What its view in the inventory writes (`GridItemView.UpdateInfo`):
    /// a medkit's whole resource (`MaxHpResource`), food's, drink's or a
    /// barter thing's (`MaxResource`), a key's uses (`MaximumNumberOfUsage`),
    /// a weapon's round (`ammoCaliber`) and how a weapon's or a knife's
    /// durability stands as made and at most (`Durability`, `MaxDurability`).
    pub max_hp_resource: f32,
    pub max_resource: f32,
    pub key_uses: u32,
    /// It names a set of buffs (`StimulatorBuffs`): a knife so made has a
    /// poison of so many doses (`Knife`'s `SideEffectComponent`, its
    /// `MaxResource`), which a quick-use view writes.
    pub side_effect: bool,
    pub caliber: String,
    pub durability: [f32; 2],
    /// How many rounds a magazine or a box of rounds holds (its
    /// `Cartridges` or `StackSlots`' `_max_count`); 0 for anything else.
    pub capacity: u32,
    /// A repair kit's: the classes of what it mends (`TargetItemFilter`),
    /// and the resource it is made with (`MaxRepairResource`); empty and
    /// naught for anything else.
    pub repair_targets: Vec<String>,
    pub repair_resource: f32,
    /// A magazine's share, in per cent, added to the time a round takes to
    /// load or unload (`LoadUnloadModifier`; inventory spec, "Loading and
    /// unloading a magazine").
    pub load_unload_modifier: f32,
    /// The same added to the time it takes to check (`CheckTimeModifier`;
    /// inventory spec, "Checking a magazine").
    pub check_modifier: f32,
    /// How well a look at it in the hands tells its count at the least
    /// (`CheckOverride`, held within 0 and 2; inventory spec, "What the
    /// ammunition's panel says"): 1 on 51 magazines of the install.
    pub check_override: u32,
    /// A magazine's `magAnimationIndex`: which of a weapon's reload and look
    /// clips it plays (firing spec, "Which magazine"); 0 for anything else.
    pub magazine_animation: i32,
    /// A magazine that comes out of the weapon (`ReloadMagType`
    /// `ExternalMagazine`): with a weapon whose own `ReloadMode` is
    /// `InternalMagazine` (the Model 700's, a Mosin's ProMag) the weapon
    /// reloads as `ExternalMagazineWithInternalReloadSupport`
    /// (`Weapon.ReloadMode`).
    pub magazine_external: bool,
    /// Not shown in the slot it is in (`NotShownInSlot`: the pockets).
    pub not_shown_in_slot: bool,
    /// Its sounds' group (`ItemSound`: `weap_ar`, `gear_generic`), which
    /// names its clips (`<group>_drop` and the rest).
    pub sound: String,
    /// A searched container's sound while it is searched (`SearchSound`:
    /// `woodbox_looting`, a clip of the things' sounds); empty for others.
    pub search_sound: String,
    /// What the right click's menu goes by (`item_menu`): a weapon's way
    /// of being loaded (`ReloadMode`), whether it is a quest's, whether it
    /// has a switch on a hinge (`HasHinge`) or is a face shield
    /// (`FaceShieldComponent`), whether it folds or slides in (`Foldable`,
    /// `Retractable`) and the slot that must hold something for it to
    /// (`FoldedSlot`).
    pub reload_mode: String,
    pub quest: bool,
    pub hinge: bool,
    pub face_shield: bool,
    /// A face shield's mask over the first person's view
    /// (`FaceShieldMask`; `visor`).
    pub face_shield_mask: trp_data::character::visor::MaskKind,
    pub foldable: bool,
    pub folded_slot: String,
    /// The cells a fold takes off the right (`SizeReduceRight`), and
    /// whether, fitted to a weapon, it keeps the weapon from folding
    /// (`BlocksFolding`).
    pub fold_reduce: u32,
    pub blocks_folding: bool,
    /// For a magazine whose rounds are drawn in its model
    /// (`SpringMagazineVisual`: neither a belt's nor a stationary
    /// weapon's), its template's visible ranges (`magazine_view`); empty
    /// for anything else. Whether a picture leaves its contents out
    /// (`HideEntrails`).
    pub visible_rounds: Vec<[u32; 2]>,
    pub hide_entrails: bool,
    /// How much of a flash it keeps from the eyes (`BlindnessProtection`:
    /// a face shield, a visor), 0 to 1.
    pub blindness_protection: f32,
    /// Its armour class (`armorClass`, a number or its words), 0 for a
    /// thing of none: a plate's mark on its view goes by it (`ItemViewStats.
    /// GetArmorPlateClassIconName`).
    pub armor_class: u32,
    /// How much it cuts what its wearer hears (`DeafStrength`; `hearing`).
    pub deaf: crate::sound::hearing::Deaf,
    /// For a weapon of the install with a stock build, that build's key
    /// (`trp_data::weapon::build_key`): a weapon so built is asked for by
    /// its template alone (`rig_key`). For a weapon the database has no
    /// stock build of (the RSP-30s, the SP-81), the key of it alone
    /// (`trp_data::weapon::alone`): bare, it is asked for by its template;
    /// with parts, by its build. A weapon of one's own's, its stock build
    /// (its magazine; the install's parts in its own slots go on it,
    /// `trp_data::custom::SlotDef`). None for anything else.
    pub stock_key: Option<String>,
    /// The templates it cannot be on a weapon with (`ConflictingItems`;
    /// `Slot.CheckConflictingItems`).
    pub conflicting: Vec<String>,
    /// It goes only into the pockets' special slots (`IsSpecialSlotOnly`;
    /// `Slot.CheckConditions`).
    pub special_only: bool,
    /// Whether it is an item the game makes, not a class of them (`_type`
    /// `Item`, not `Node`).
    pub is_item: bool,
    /// A tactical device's modes (`LightComponent`, on a `Flashlight`, a
    /// `TacticalCombo` or a `LightLaser`: its template's `ModesCount`);
    /// none for a thing that carries no light (tactical-devices spec, "The
    /// items").
    pub light_modes: Option<u32>,
    /// Known without being examined (`Item.ExaminedByDefault`: the
    /// template's `ExaminedByDefault`, or a quest's item); how long an
    /// examine of it takes, in seconds, before Attention (`ExamineTime`);
    /// the experience it gives (`ExamineExperience`). `examine`.
    pub examined_by_default: bool,
    pub examine_time: f32,
    pub examine_experience: u32,
    /// What the head things' fittings go by (`PlayerClippingRules`; body
    /// animator spec, "The head's things fitted to one another"): its
    /// template's `MaterialType` is `Helmet` (`HeadwearTemplate`), and it
    /// is a half mask (`FaceCoverTemplate.IsHalfMask`, which no template of
    /// the install names: false for every face cover).
    pub helmet_material: bool,
    pub half_mask: bool,
}

impl Kind {
    pub fn of(db: &Database, template: &str) -> Result<Self> {
        let item = db.items.get(template).ok_or_else(|| anyhow!("the database has no item {template}"))?;
        let flag = |name: &str| item.props.get(name).and_then(|on| on.as_bool()).unwrap_or(false);
        let class_names = db.class_names(template);
        let class = |name: &str| class_names.iter().any(|class| class == name);
        Ok(Self {
            template: item.id.clone(),
            name: item.name.clone(),
            classes: db.classes(template),
            class_names: class_names.clone(),
            weight: item.prop_f32("Weight").unwrap_or(0.0),
            cells: item.cells(),
            extra: item.extra_cells(),
            forced_extra: flag("ExtraSizeForceAdd"),
            merges: item.merges_with_parts(),
            slots: item.slots(),
            grids: item.grids(),
            cartridges: item.cartridges(),
            blocks: item.blocked_slots(),
            stack_max: item.stack_max(),
            not_raid_moddable: !item.raid_moddable(),
            kept_in: item.kept_in_slots(),
            color: item.background_color().to_owned(),
            layout: item.grid_layout_name().to_owned(),
            max_hp_resource: item.prop_f32("MaxHpResource").unwrap_or(0.0),
            max_resource: item.prop_f32("MaxResource").unwrap_or(0.0),
            key_uses: item.prop_f32("MaximumNumberOfUsage").unwrap_or(0.0).max(0.0) as u32,
            side_effect: item.props.get("StimulatorBuffs").and_then(|buffs| buffs.as_str()).is_some_and(|buffs| !buffs.is_empty()),
            caliber: item.props.get("ammoCaliber").and_then(|caliber| caliber.as_str()).unwrap_or_default().to_owned(),
            durability: [item.prop_f32("Durability").unwrap_or(0.0), item.prop_f32("MaxDurability").unwrap_or(0.0)],
            repair_targets: item
                .props
                .get("TargetItemFilter")
                .and_then(|list| list.as_array())
                .map(|list| list.iter().filter_map(|class| class.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
            repair_resource: item.prop_f32("MaxRepairResource").unwrap_or(0.0),
            capacity: ["Cartridges", "StackSlots"]
                .iter()
                .find_map(|list| item.props.get(*list)?.get(0)?.get("_max_count")?.as_u64())
                .unwrap_or(0) as u32,
            load_unload_modifier: item.prop_f32("LoadUnloadModifier").unwrap_or(0.0),
            check_modifier: item.prop_f32("CheckTimeModifier").unwrap_or(0.0),
            check_override: item.prop_f32("CheckOverride").unwrap_or(0.0).clamp(0.0, 2.0) as u32,
            magazine_animation: item.props.get("magAnimationIndex").and_then(|index| index.as_i64()).unwrap_or(0) as i32,
            magazine_external: item.props.get("ReloadMagType").and_then(|mode| mode.as_str()) == Some("ExternalMagazine"),
            not_shown_in_slot: item.props.get("NotShownInSlot").and_then(|shown| shown.as_bool()).unwrap_or(false),
            sound: item.props.get("ItemSound").and_then(|sound| sound.as_str()).unwrap_or_default().to_owned(),
            search_sound: item.props.get("SearchSound").and_then(|sound| sound.as_str()).unwrap_or_default().to_owned(),
            reload_mode: item.props.get("ReloadMode").and_then(|mode| mode.as_str()).unwrap_or_default().to_owned(),
            quest: item.props.get("QuestItem").and_then(|quest| quest.as_bool()).unwrap_or(false),
            hinge: item.props.get("HasHinge").and_then(|hinge| hinge.as_bool()).unwrap_or(false),
            face_shield: item.props.get("FaceShieldComponent").and_then(|shield| shield.as_bool()).unwrap_or(false),
            face_shield_mask: item.props.get("FaceShieldMask").and_then(|mask| mask.as_str()).and_then(trp_data::character::visor::MaskKind::of_word).unwrap_or_default(),
            // A weapon folds or slides in by its own word, a stock only
            // by `Foldable` (`Weapon`, `Stock`: the only two that are made
            // with a `FoldableComponent`).
            foldable: if class("Weapon") { flag("Foldable") || flag("Retractable") } else { class("Stock") && flag("Foldable") },
            folded_slot: item.props.get("FoldedSlot").and_then(|slot| slot.as_str()).unwrap_or_default().to_owned(),
            fold_reduce: item.prop_f32("SizeReduceRight").unwrap_or(0.0).max(0.0) as u32,
            blocks_folding: flag("BlocksFolding"),
            visible_rounds: if class("Magazine") && !flag("MagazineWithBelt") && !flag("IsMagazineForStationaryWeapon") {
                crate::items::magazine_view::visible_ranges(item.props.get("VisibleAmmoRangesString").and_then(|ranges| ranges.as_str()))
            } else {
                Vec::new()
            },
            hide_entrails: flag("HideEntrails"),
            blindness_protection: item.prop_f32("BlindnessProtection").unwrap_or(0.0),
            armor_class: item.props.get("armorClass").and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(0.0).max(0.0) as u32,
            deaf: item.props.get("DeafStrength").and_then(|word| word.as_str()).map(crate::sound::hearing::Deaf::of_word).unwrap_or_default(),
            stock_key: if class("Weapon") { Some(db.default_preset(template).map_or_else(|_| trp_data::weapons::weapon::build_key(&trp_data::weapons::weapon::alone(template)), trp_data::weapons::weapon::build_key)) } else { None },
            conflicting: item.props.get("ConflictingItems").and_then(|list| list.as_array()).into_iter().flatten().filter_map(|id| id.as_str().map(str::to_owned)).collect(),
            special_only: flag("IsSpecialSlotOnly"),
            is_item: item.node_type == "Item",
            light_modes: trp_data::weapons::tactical::DeviceClass::of(db, template).map(|_| trp_data::weapons::tactical::modes_of(db, template)),
            examined_by_default: flag("ExaminedByDefault") || flag("QuestItem"),
            examine_time: item.prop_f32("ExamineTime").unwrap_or(0.0).max(0.0),
            examine_experience: item.prop_f32("ExamineExperience").unwrap_or(0.0).max(0.0) as u32,
            helmet_material: item.props.get("MaterialType").and_then(|material| material.as_str()) == Some("Helmet"),
            half_mask: flag("IsHalfMask"),
        })
    }

    /// Whether a filter's list names this kind or a class of it.
    pub fn among(&self, list: &[String]) -> bool {
        list.iter().any(|id| self.classes.contains(id))
    }
}

/// Where an item lies in a grid: the column and row of its top left
/// cell, and whether it lies turned a quarter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub x: u32,
    pub y: u32,
    pub turned: bool,
}

/// Why a slot or a grid does not take an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// There is no such slot or grid.
    NoSuchPlace,
    Taken,
    /// The filter does not take this kind of item.
    Filter,
    /// Something worn blocks the slot: the slot that item is in.
    Blocked(String),
    /// The item would block a slot that has something in it.
    WouldBlock(String),
    /// A slot takes a single item, not a stack.
    Stack,
    OutsideGrid,
    NoRoom,
    /// It is there already.
    AlreadyThere,
    /// It would go into itself.
    Cycle,
    /// Backpacks would lie in one another deeper than the game allows.
    TooDeep,
    /// The slot holds a part the item cannot do without, which stays
    /// where it is in a raid: the slot.
    Vital(String),
    /// A part that may not be changed in a raid.
    NotInRaid,
    /// A weapon that lacks a vital part does not go onto the character.
    MissingParts,
    /// The dogtag's slot takes nothing by hand.
    Dogtag,
    /// It is not taken out of that equipment slot in a raid: the slot.
    KeptOn(String),
    /// Nothing can be done with the one on the other.
    NoAction,
    /// The slot is locked (`SlotLockedError`): nothing goes into it or
    /// comes out of it by hand (the pockets worn, an armour's built-in
    /// inserts).
    Locked,
    /// Two things that may not be on one item together
    /// (`ConflictingItemError`): the template of the one there, and of the
    /// one coming (or of the thing on it that names, or is named by, the
    /// other).
    Conflicting { existing: String, new: String },
    /// A plate's slot of armour worn takes nothing in a raid
    /// (`ArmorSlot.CanAcceptRaid`, `EquippedLockedSlot`).
    PlateWorn,
    /// More of a kind than the character may carry in a raid
    /// (`ItemManipulator.CountLimitError`): its template and the limit.
    OverLimit { template: String, limit: u32 },
    /// The weapon in hand cannot have its parts moved now: its hands are
    /// busy (`FirearmController.CanExecute`: only while its logic idles),
    /// or would not take a round into its chamber (`Rules::chamber_closed`).
    HandsBusy,
    /// The weapon has a malfunction that keeps it (`ItemManipulator.MalfunctionError`,
    /// "You must fix malfunctions first": `Weapon.IncompatibleByMalfunction`).
    Malfunction,
    /// A thing not known (`examine`) into a slot of the character's own
    /// (`Slot.InstallNotExaminedError`; `equipment`, one of the equipment's
    /// own slots), or dropped on a slot, a weapon or a magazine
    /// (`ItemNotExaminedError`, `equipment` false).
    NotExamined { equipment: bool },
    /// A thing not known out of a slot of the character's own
    /// (`Slot.TakeNotExaminedError`).
    TakeNotExamined { equipment: bool },
    /// A magazine with no room for one more round
    /// (`Magazine.FullMagazineError`).
    MagazineFull,
    /// A round a magazine's cartridges do not take
    /// (`Magazine.ItemFiltersWontAllowError`).
    WrongAmmo,
}

/// Whether a slot of the character's own refuses to take or to give up
/// that thing as unknown (`Slot.Examined`: only where the slot's item is
/// the player's own, its equipment and what is in it; rounds never).
pub(super) fn unknown_to_own(root: &Item, item: &Item, rules: &Rules) -> bool {
    own(root, rules) && !is(item, "Ammo") && !crate::items::examine::known(item)
}

/// Whether a root is the character's own equipment: an equipment, and the
/// one `Rules::own` names where one is named (a body's is another).
pub(super) fn own(root: &Item, rules: &Rules) -> bool {
    root.kind.template == EQUIPMENT_TEMPLATE && rules.own.is_none_or(|own| own == root.id)
}
