//! The carried things' state and how each thing is named and described.

use super::*;

/// What the character carries, what the crate on the range holds, and the
/// game's own short names for items (from the install's English texts).
#[derive(Resource)]
pub struct Carried {
    pub equipment: Equipment,
    pub names: HashMap<String, String>,
    /// The container last searched (at first the airdrop crate), with what
    /// is in it: the one the screen shows beside the equipment.
    pub loot: Item,
    /// Whether it is open in the screen.
    pub loot_open: bool,
    /// The range's other containers with what is in them, by their place
    /// in this list (`range::Placed::holds`); the one in `loot` has none here.
    pub away: Vec<Option<Item>>,
    /// Which of them is in `loot`.
    pub loot_of: usize,
    /// A loose container being searched (`SEARCH` on a thing on the
    /// ground): lifted into `away` at the first number, shown in place of
    /// the second, to go back where it lay (the third) when the screen
    /// shuts (`search_ground`, `put_back_ground`).
    pub searched_ground: Option<(usize, usize, Vec3)>,
    /// The screen opened from the workbench's `REPAIR GEAR` (the app's): a
    /// repair kit let go on what it mends opens its repair window
    /// (`repair_view`), and nothing is moved.
    pub repairing: bool,
    /// Each repair window's own state, by its thing (`repair_view::Repair`).
    pub repairs: Vec<crate::screens::repair_view::Repair>,
    /// The repair's figures (`trp_sim::repair::Settings`), read as the run
    /// begins.
    pub repair_settings: Option<std::sync::Arc<trp_sim::items::repair::Settings>>,
    /// What moving things goes by besides the places' own rules.
    pub rules: Rules,
    /// What is drawn on the body whatever the equipment holds (`--wear`).
    pub fixed_wear: Vec<String>,
    /// What the last move came to, for the screen's bottom line.
    pub note: String,
    /// The game's own equipment panel as the install lays it out; empty
    /// if it could not be read.
    pub panel: trp_data::interface::screen::GearPanel,
    /// What has been put down, each thing where it lies (the engine's axes).
    pub ground: Vec<(Item, Vec3)>,
    /// Things shown in a window that are not the character's: the edit
    /// build screen's parts inspected from it (a copy of each), until it
    /// shuts. Found by number as what is carried is; never moved.
    pub observed: Vec<Item>,
    /// A picture for each sort of thing, by the sort's name (`icon_of`):
    /// what a thing shows until its own picture is made.
    pub icons: HashMap<&'static str, Picture>,
    /// Each item's own picture, by its template, as the game makes it
    /// (`icons`: its model drawn once and kept). Written without telling
    /// of a change to what is carried: a picture come is put in its
    /// spinner's place (`PicturesArrived`), not drawn with the screen
    /// afresh.
    pub pictures: HashMap<String, Picture>,
    /// Whether the pictures are made at all (`ICON GENERATION`), those that
    /// could not be, and the spinner shown in a box while its picture is
    /// still to come (where the game's view shows its loader).
    pub pictures_on: bool,
    pub picture_failed: std::collections::HashSet<String>,
    pub spinner: Handle<Image>,
    /// What is bound to the quick-use keys.
    pub bound: Bound,
    /// The grenade the grenade's key takes and its view shows (`quick_keys`).
    pub top_grenade: Option<Id>,
    /// A magazine being loaded or unloaded a round at a time
    /// (`work_magazine`), and the base times a round takes.
    pub magazine_work: Option<MagazineWork>,
    /// The thing being used, medicine, food or drink, while the health is
    /// at work with it (`settle`): the game's view of it shows its drain
    /// loader and takes no clicks (`ItemView.IsBeingDrained`, from the
    /// health's `DrainItemEventArgs`, Begin to Succeed); here it also
    /// blinks as a magazine being loaded does.
    pub drained: Option<Id>,
    /// A thing moved out of a slot whose move waits on the hands, and the
    /// slot it left (its holder and the slot): it is shown there as well,
    /// blinking, until the move is done (`ItemView`s of an operation from
    /// `Begin` to `Succeed`: `courses10`'s `drag-and-drop`, frames 1082 to
    /// 1176 and 1627 to 1732). Kept by `settle` while the thing blinks.
    pub leaving: Vec<(Id, Id, usize)>,
    /// The picture's key a magazine at work is shown by (`shown_key`): as it
    /// began, asked again only where the game asks (`LoadMagazineProcess`:
    /// a round that adds or takes away a stack, the load's last round, an
    /// unload that leaves it empty; inventory spec, "A magazine's picture
    /// at work").
    pub frozen_keys: HashMap<Id, String>,
    /// The weapon whose magazine a reload is exchanging, and its picture's
    /// key as the reload began. Its views keep that picture until the
    /// reload is over: the game's refresh on the move's success, which a
    /// reload raises as it ends (`QuickSlotItemView.OnItemAdded` with
    /// `CommandStatus.Succeed`; `ReloadExternalMagOperation.
    /// SwitchToIdlingState`).
    pub reload_key: Option<(Id, String)>,
    /// A refused quick move's warning in the game's words, until the
    /// notices take it (`dragging::menu_clicks`).
    pub warning: Option<String>,
    /// Switches flipped since the hands last heard (the part, how it
    /// stands now): `settle` asks the gesture and the sound for one worn on
    /// the head (inventory spec, "A switch on the head").
    pub switched: Vec<(Id, bool)>,
    /// A fold of the weapon in hand (or of a part of it) asked from the
    /// menu, for its hands to take or not (`settle`; inventory spec, "A
    /// fold from the menu").
    pub fold_asked: Option<Id>,
    pub round_times: RoundTimes,
    /// The windows open over the screen, the last on top.
    pub windows: Vec<Win>,
    /// The container whose window was opened or picked last: where Ctrl
    /// and a click sends a thing (the game's priority window, which the
    /// install's `PriorityWindowMode` has as `Auto`).
    pub priority: Option<Id>,
    /// Whether the install has that setting as `Auto`.
    pub priority_auto: bool,
    /// The window that asks how many of a stack, while it is up.
    pub asking: Option<Asking>,
    /// The backpack's drop has been asked for, by its key or by the
    /// screen's key for putting a thing down: `toggle` begins it, and the
    /// pack leaves the shoulders at the gesture's own mark.
    pub pack_asked: bool,
    /// A magazine out of the weapon with nowhere to go, until it leaves
    /// the hand and falls (firing spec, "The quick reload").
    pub falling: Vec<Item>,
    /// What has been put on the ground this frame and how it left the
    /// character, for the physics to set going (`loose`).
    pub thrown: Vec<(Id, crate::world::loose::How)>,
    /// The game's own views of a thing in a slot and in a grid, which a
    /// thing is drawn by with `TARKOV UI` on (none with it off).
    pub item_looks: [Option<std::sync::Arc<crate::screens::health_view::ItemViewLook>>; 2],
    /// The magazines whose rounds the character knows (the profile's
    /// `CheckedMagazines`): every one carried as the run begins
    /// (`LocalPlayer.Create`), and the weapon's own as it is checked in the
    /// hands (`FirearmController.CheckAmmo`); a magazine looted later is
    /// not, and its view writes `?` for its count (`GetAmmoCountByLevel`).
    pub checked: std::collections::HashSet<Id>,
    /// The weapons whose chambers the character knows (the profile's
    /// `CheckedChambers`; inventory spec, "A chamber not checked"): none as
    /// the run begins (`PROVISIONAL`: the profile's not read; `courses10`'s
    /// `weapon-inspect-menu` began with the M4's unknown), each as the game
    /// adds or takes it away (`checked_chamber`).
    pub checked_chambers: std::collections::HashSet<Id>,
    /// What Strength makes of carrying (`skills`): at its elite level the
    /// weapons weigh nothing (`TotalWeightEliteSkill`), and the overweight
    /// limits are times one and its lift (`CarryingWeightRelativeModifier`).
    pub strength_elite: bool,
    pub carry: f32,
    /// What the character's weight was last worked out from, as the game's
    /// inventory tells of it (`weigh_carried`): what the hands do unheard
    /// (a shot's rounds, a launcher's grenade fired) passed over
    /// (`Carried::unheard`), counted at the next event.
    pub weighed: trp_sim::items::inventory::Weighed,
    /// The carried weight as the inventory last told of it (`Inventory.
    /// TotalWeight`, a sum kept until the next event), which the screens
    /// show (`HealthParametersPanel`): a shot's rounds not out of it yet.
    pub told_weight: f32,
    /// The character's weapon mastering as it stands (`skills::Mastery`:
    /// each weapon its family's), which a weapon's view in a slot, the
    /// inspect window and the repair window show.
    pub mastery: crate::player::skills::Mastery,
    /// The words of the keys of the four slots that have one of their own
    /// (the sheath, the holster, the sling, the back), by slot, which a
    /// thing in one is labelled with as a thing bound to a key is
    /// (`BindPanel`); kept as the bindings change (`name_slot_keys`).
    pub slot_keys: Vec<(&'static str, String)>,
    /// What the window that shows a thing says of it: its name, its kind
    /// and its description (`trp_data::inventory::item_texts`).
    pub texts: trp_data::character::inventory::ItemTexts,
    /// The builds whose name a weapon's takes (`named_builds`).
    pub named_builds: Vec<(String, Vec<String>)>,
    /// The game's window that shows a thing, its size in units, where it
    /// is drawn (`TARKOV UI`): opened where the game opens one.
    pub info_size: Option<Vec2>,
    /// Whether a container's window is the game's (`TARKOV UI`), which
    /// opens where the game opens one.
    pub game_grid_window: bool,
    /// The same of the game's window that edits a tag, and the most letters
    /// its field takes; and each tag being edited in one.
    pub tag_size: Option<Vec2>,
    pub tag_limit: u32,
    pub tag_edits: Vec<TagEdit>,
    /// The wish list (inventory spec, "The wish list"): kinds of thing,
    /// each in a group; empty as the run begins (the profile's not read).
    pub wishlist: trp_sim::items::item_menu::Wishlist,
    /// The examine under way (inventory spec, "Examining"), and the thing
    /// asked to be examined this frame (`examine_view`).
    pub examiner: trp_sim::items::examine::Examiner,
    pub examine_asked: Option<Id>,
    /// What is still to be searched and found, and the searches under way
    /// (inventory spec, "Searching a container"; `search_view`); a search
    /// asked for or stopped by a click this frame (the container, and
    /// whether it is begun); the real clock as the screen was last drawn
    /// for a search's timer.
    pub searcher: trp_sim::items::search::Searcher,
    pub search_asked: Option<(Id, bool)>,
    pub search_clock: f64,
    /// The words the loot panel's timer was left with (`TimerText`: one
    /// for every container the panel shows), none before its first search
    /// (the scene's own).
    pub search_timer_said: Option<String>,
    /// Each dummy's gear by its number (inventory spec, "Searching a
    /// body"): a copy of what the player wore as it was dressed, none
    /// before then or while the screen shows it.
    pub bodies: Vec<Option<Item>>,
    /// A dead dummy searched (`SEARCH` on a body): its number, where its
    /// gear is in `away` while the screen shows it, and the container
    /// shown before (`search_body`, `put_back_body`).
    pub searched_body: Option<(usize, usize, usize)>,
    /// Search's elite (`SearchDouble`) as the skills stand: two searches
    /// may be under way at once.
    pub search_double: bool,
}

impl Carried {
    /// Whether nothing the inventory tells of has changed since the weight
    /// was last worked out (`weigh_carried`): what the hands do next may
    /// then go unheard (`unheard`).
    pub(super) fn as_weighed(&self) -> bool {
        self.weighed.as_told(&self.equipment.item)
    }

    /// What the hands have just done, which the game's inventory is not
    /// told of (the rounds shots take out of the magazine and the chamber,
    /// a launcher's grenade fired): passed over where nothing it is told of
    /// had changed before (`quiet`, from `as_weighed`), so that the weight
    /// counts it at the next event; else that event has come first, and
    /// counts it with its own. Written without telling of a change: the
    /// hands' own writes have told of it.
    pub(super) fn unheard(this: &mut ResMut<Self>, quiet: bool) {
        if quiet && !this.as_weighed() {
            let carried = this.bypass_change_detection();
            carried.weighed.pass(&carried.equipment.item);
        }
    }

    /// The thing leaving that slot of that holder while the hands finish its
    /// move (`leaving`), where the slot is empty.
    pub(super) fn leaving_from(&self, holder: Id, slot: usize) -> Option<&Item> {
        let (id, ..) = self.leaving.iter().find(|(_, from, at)| *from == holder && *at == slot)?;
        self.equipment.item.find(*id)
    }

    /// Mag Drills' elite (`MagDrillsInstantCheck`; `ItemController`,
    /// `MoveOperation`): a magazine put into the character's own things is
    /// known at once.
    pub(super) fn known_on_arrival(&mut self, id: Id) {
        let mine = self.equipment.item.find(id).is_some_and(|item| item.kind.class_names.iter().any(|class| class == "Magazine"));
        if self.round_times.drills.instant_check && mine {
            self.checked.insert(id);
        }
    }

    /// The game's short name for an item, or the database's own.
    pub(crate) fn name(&self, item: &Item) -> String {
        self.names.get(&item.kind.template).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| item.kind.name.clone())
    }

    /// The words an item's view writes as its caption
    /// (`GridItemView.ItemShortName`): an armour's plate or insert in an
    /// armour's slot by its slot's name in the install's words (an
    /// insert's own short name is empty), anything else by its short name.
    pub(crate) fn view_name(&self, item: &Item) -> String {
        // A thing not known: "???" (`GridItemView.GetItemName`).
        if !trp_sim::items::examine::known(item) {
            return "???".to_owned();
        }
        if item.kind.class_names.iter().any(|name| name == "ArmorPlate") {
            if let Some((holder, Spot::Slot(index))) = self.lies(item.id) {
                if let Some(slot) = self.find(holder).and_then(|holder| holder.kind.slots.get(index)).filter(|slot| slot.armour) {
                    return self.texts.say(&slot.name).unwrap_or_else(|| slot.name.clone());
                }
            }
        }
        // A weapon's with its build's name (`GridItemView.GetItemName`:
        // `ItemFactory.BriefItemName` of its short name).
        self.brief_name(item, self.name(item))
    }

    /// The words of a thing's tooltip (`GridItemView.GetItemName`): "Unknown
    /// item" in the install's words for a thing not known, else its full name
    /// in them (`Item.Name`), a weapon's with its build's name
    /// (`ItemFactory.BriefItemName`).
    pub(crate) fn tooltip_name(&self, item: &Item) -> String {
        if !trp_sim::items::examine::known(item) {
            return self.texts.say("Unknown item").unwrap_or_else(|| "Unknown item".to_owned());
        }
        self.brief_name(item, self.full_name(item))
    }

    /// A thing's whole name in the install's words (`Item.Name.Localized()`),
    /// known or not; its template's own where the install has none.
    pub(crate) fn full_name(&self, item: &Item) -> String {
        self.texts.names.get(&item.kind.template).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| item.kind.name.clone())
    }

    /// The words of a thing's tooltip and how long the mouse rests on its
    /// view before they show (`GridItemView.ShowTooltip`): an armour plate
    /// in its armour's slot (an inspect window's) by the plate's words
    /// (`ShowTooltipArmorPlate`, with no reason: its slot is not locked, or
    /// its part would be out of the mouse's reach), after 0.6 s; a weapon's
    /// red words at once (`weapon_warning`, the simple tooltip's `Show(text)`
    /// with no delay); anything else by its name (`tooltip_name`) after 0.6 s.
    pub(crate) fn tooltip_words(&self, item: &Item) -> (String, f32) {
        if is(item, "ArmorPlate") {
            if let Some((holder, Spot::Slot(index))) = self.lies(item.id) {
                if let Some(slot) = self.find(holder).and_then(|holder| holder.kind.slots.get(index)).filter(|slot| slot.armour) {
                    return (self.plate_words(item, &slot.name, ""), ITEM_TIP_DELAY);
                }
            }
        }
        if let Some(warning) = self.weapon_warning(item) {
            return (warning, 0.0);
        }
        (self.tooltip_name(item), ITEM_TIP_DELAY)
    }

    /// A weapon's tooltip in red in place of its name (`ShowTooltip`, known
    /// or not): with a malfunction the shooter knows of, "Malfunction!
    /// Examine your weapon" while its kind is not known, else "Fix
    /// malfunction: {kind}" (the notices' words with no key after them:
    /// `MalfunctionExamineTypeNotification`, `MalfunctionRepairRequiredNotification.
    /// GetLocalizedDescription(withKey: false)`); else, with a magazine whose
    /// top round its chamber does not take (`Weapon.CompatibleAmmo`), "Ammo
    /// ({round}) is not compatible. Need: {caliber}", the round's whole name
    /// and the weapon's caliber (`AmmoCaliber`) each in white. None else.
    pub(crate) fn weapon_warning(&self, item: &Item) -> Option<String> {
        if !is(item, "Weapon") {
            return None;
        }
        let say = |key: &str, otherwise: &str| self.texts.say(key).unwrap_or_else(|| otherwise.to_owned());
        let state = item.malfunction;
        if state.known && state.malf != trp_sim::weapons::malfunction::Malf::None {
            let words = if state.type_known {
                let kind = say(state.malf.text_key(), state.malf.text_key());
                say("Notification/MalfunctionRepairRequired", "Fix malfunction: {0}").replace("{0}", &kind)
            } else {
                say("Notification/MalfunctionExaminationRequired", "Malfunction! Examine your weapon")
            };
            return Some(format!("<color=red>{words}</color>"));
        }
        if !trp_sim::items::inventory::ammo_incompatible(item) {
            return None;
        }
        let top = item.in_slot(trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|magazine| trp_sim::items::inventory::magazine_round_at(magazine, 0));
        let round = top.map(|kind| self.texts.names.get(&kind.template).filter(|name| !name.is_empty()).cloned().unwrap_or_else(|| kind.name.clone())).unwrap_or_default();
        let line = say(crate::weapons::wear::INCOMPATIBLE_AMMO, crate::weapons::wear::INCOMPATIBLE_AMMO);
        let line = format!("{line}</color>").replace("{0}", &format!("<color=white>{round}</color>")).replace("{1}", &format!("<color=white>{}</color>", item.kind.caliber));
        Some(format!("<color=red>{line}"))
    }

    /// An armour plate's words in its armour's slot (`ArmorPlatesFormatter.
    /// FormatArmorPlateTooltip`): its class's mark (none under class 1), a
    /// space, its slot's name in the install's words in bold with a colon,
    /// its whole name, " - ", its durability and its greatest each to a
    /// place (`Math.Round(_, 1)`, halves to the even: "40/40", "37.5/40");
    /// then two line breaks and the reason, where there is one.
    pub(crate) fn plate_words(&self, plate: &Item, slot: &str, reason: &str) -> String {
        let mark = if plate.kind.armor_class > 0 { format!("<sprite name=\"armor_classes_{}\">", plate.kind.armor_class) } else { String::new() };
        let slot = self.texts.say(slot).unwrap_or_else(|| slot.to_owned());
        let [now, most] = plate.durability();
        let round = |value: f32| (value as f64 * 10.0).round_ties_even() / 10.0;
        let mut words = format!("{mark} <b>{slot}:</b> {} - {}/{}", self.full_name(plate), round(now), round(most));
        if !reason.is_empty() {
            words.push_str("\n\n");
            words.push_str(reason);
        }
        words
    }

    /// A weapon's chamber known or not from now on (`InventoryController.
    /// CheckChamber(weapon, status)`, the profile's `CheckedChambers`).
    pub(crate) fn know_chamber(&mut self, gun: Id, known: bool) {
        if known {
            self.checked_chambers.insert(gun);
        } else {
            self.checked_chambers.remove(&gun);
        }
    }

    /// A name as the game's views give a thing (`ItemFactory.BriefItemName`):
    /// a weapon's with, after a space, the name of the first named build all
    /// of whose parts it has (`BriefWeaponPresetName`), cut to 30 letters
    /// with "..." for the last two where longer (`ItemCachedName.BriefName`,
    /// `SubstringIfNecessary`); anything else's as it is. (A build saved by
    /// the player, which the game looks for first, the app has none of.)
    pub(crate) fn brief_name(&self, item: &Item, name: String) -> String {
        brief_name(&self.named_builds, item, name)
    }
}
