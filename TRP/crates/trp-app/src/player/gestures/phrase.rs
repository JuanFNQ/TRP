//! The voice's phrase triggers, their names and priorities, and which one a moment of play sets off.
use trp_sim::weapons::malfunction::{Malf, State};
use trp_sim::FirearmEvent;

pub const ON_ENEMY_GRENADE: i32 = 11;
pub const ON_FRIENDLY_DOWN: i32 = 14;
pub const ON_GRENADE: i32 = 22;
pub const ON_WEAPON_RELOAD: i32 = 23;
pub const ON_WEAPON_JAMMED: i32 = 24;
pub const HAND_BROKEN: i32 = 48;
pub const LEG_BROKEN: i32 = 49;
pub const BLEEDING: i32 = 50;
pub const DEHYDRATED: i32 = 51;
pub const EXHAUSTED: i32 = 52;
pub const HURT_LIGHT: i32 = 53;
pub const HURT_MEDIUM: i32 = 54;
pub const HURT_HEAVY: i32 = 55;
pub const HURT_NEAR_DEATH: i32 = 56;
pub const NEED_AMMO: i32 = 87;
pub const EXIT_LOCATED: i32 = 91;
pub const LOOT_KEY: i32 = 92;
pub const LOCKED_DOOR: i32 = 93;
pub const LOOT_BODY: i32 = 94;
pub const LOOT_CONTAINER: i32 = 95;
pub const LOOT_GENERIC: i32 = 96;
pub const LOOT_MONEY: i32 = 97;
pub const LOOT_WEAPON: i32 = 98;
pub const COOPERATION: i32 = 99;
pub const LOOT_NOTHING: i32 = 100;
pub const WEAPON_BROKEN: i32 = 101;
pub const OPEN_DOOR: i32 = 102;
pub const CHECK_HIM: i32 = 103;
pub const MUMBLE: i32 = trp_sim::sound::speaker::trigger::MUMBLE;

/// The names of every phrase the menu and the prompt know.
const NAMES: &[(i32, &str)] = &[
    (11, "OnEnemyGrenade"),
    (14, "OnFriendlyDown"),
    (22, "OnGrenade"),
    (23, "OnWeaponReload"),
    (24, "OnWeaponJammed"),
    (30, "CoverMe"),
    (31, "FollowMe"),
    (32, "GetBack"),
    (33, "GoForward"),
    (34, "Gogogo"),
    (35, "Look"),
    (36, "HoldPosition"),
    (37, "GoLoot"),
    (38, "Stop"),
    (39, "Silence"),
    (40, "OnYourOwn"),
    (41, "Fire"),
    (42, "HoldFire"),
    (43, "Suppress"),
    (44, "Spreadout"),
    (45, "GetInCover"),
    (46, "KnifesOnly"),
    (47, "Regroup"),
    (48, "HandBroken"),
    (49, "LegBroken"),
    (50, "Bleeding"),
    (51, "Dehydrated"),
    (52, "Exhausted"),
    (53, "HurtLight"),
    (54, "HurtMedium"),
    (55, "HurtHeavy"),
    (56, "HurtNearDeath"),
    (57, "DontKnow"),
    (58, "Clear"),
    (59, "Going"),
    (60, "Covering"),
    (61, "BadWork"),
    (62, "Negative"),
    (63, "Ready"),
    (64, "OnPosition"),
    (65, "OnLoot"),
    (66, "GoodWork"),
    (67, "Roger"),
    (68, "Repeat"),
    (69, "LeftFlank"),
    (70, "Scav"),
    (71, "SniperPhrase"),
    (72, "RightFlank"),
    (73, "InTheFront"),
    (74, "OnSix"),
    (75, "EnemyDown"),
    (76, "ScavDown"),
    (77, "LostVisual"),
    (78, "EnemyHit"),
    (79, "KnifeKill"),
    (80, "NoisePhrase"),
    (81, "FriendlyFire"),
    (82, "Rat"),
    (83, "Down"),
    (84, "Hit"),
    (85, "NeedFrag"),
    (86, "NeedSniper"),
    (87, "NeedAmmo"),
    (88, "NeedHelp"),
    (89, "NeedWeapon"),
    (90, "NeedMedkit"),
    (91, "ExitLocated"),
    (92, "LootKey"),
    (93, "LockedDoor"),
    (94, "LootBody"),
    (95, "LootContainer"),
    (96, "LootGeneric"),
    (97, "LootMoney"),
    (98, "LootWeapon"),
    (99, "Cooperation"),
    (100, "LootNothing"),
    (101, "WeaponBroken"),
    (102, "OpenDoor"),
    (103, "CheckHim"),
    (104, "MumblePhrase"),
    (105, "UnderFire"),
    (106, "StartHeal"),
    (107, "Toxic"),
    (108, "Greetings"),
    (109, "LowKarmaAttack"),
    (110, "Provocation"),
    (111, "Warning"),
    (112, "Mine"),
    (113, "Frozen"),
    (114, "LocateHostiles"),
];

/// The game's name for a phrase.
pub fn name(trigger: i32) -> &'static str {
    NAMES.iter().find(|(number, _)| *number == trigger).map_or("", |(_, name)| name)
}

/// The quick prompt's priority of a phrase, where it has one
/// (`GesturesQuickPanel.PhrasePriorities`).
pub fn priority(trigger: i32) -> Option<i32> {
    Some(match trigger {
        ON_ENEMY_GRENADE => 100,
        ON_GRENADE | ON_FRIENDLY_DOWN | ON_WEAPON_RELOAD | NEED_AMMO => 95,
        HURT_LIGHT | HURT_MEDIUM | HURT_HEAVY | HURT_NEAR_DEATH => 90,
        WEAPON_BROKEN => 88,
        COOPERATION => 87,
        ON_WEAPON_JAMMED => 86,
        BLEEDING => 85,
        HAND_BROKEN | LEG_BROKEN => 80,
        EXIT_LOCATED => 77,
        OPEN_DOOR | CHECK_HIM => 75,
        LOOT_KEY | LOCKED_DOOR | LOOT_BODY | LOOT_CONTAINER | LOOT_GENERIC | LOOT_MONEY | LOOT_WEAPON | LOOT_NOTHING | DEHYDRATED | EXHAUSTED => 70,
        _ => return None,
    })
}

/// Whether a phrase is said only while its situation holds
/// (`IsSituationalPhrase`: one with a priority, but NEED AMMO).
pub fn situational(trigger: i32) -> bool {
    priority(trigger).is_some() && trigger != NEED_AMMO
}

/// The hurt phrases, whose priority a fresh situation never restores.
pub fn hurt(trigger: i32) -> bool {
    matches!(trigger, HURT_LIGHT | HURT_MEDIUM | HURT_HEAVY | HURT_NEAR_DEATH)
}

/// A known malfunction's phrase (`NeedRepairMalfPhraseSituation`):
/// jammed for a slide's whose kind is known, else a broken weapon.
pub fn of_malfunction(malf: Malf, known: bool) -> i32 {
    if known && matches!(malf, Malf::SoftSlide | Malf::HardSlide) {
        ON_WEAPON_JAMMED
    } else {
        WEAPON_BROKEN
    }
}

/// The malfunction's situation a firearm's event raises, by the
/// weapon's state after the step that said it (`state`, none without
/// wear): the shot's malfunction made known (`PlayerOwnerInventoryController.
/// ExamineMalfunction`, not known before: its kind known at once only
/// with elite Troubleshooting), every reminder (the same, known), and
/// a weapon drawn with any malfunction left after the draw put a misfire
/// right (`SpawnOperation.WeaponAppeared`). Not the look at its kind
/// (`ExamineMalfunctionType` raises none). The 150 ms re-raise after the
/// shot is `wear`'s (`ProcessMalfunctionExamined`).
pub fn of_hands_event(event: &FirearmEvent, state: Option<&State>) -> Option<i32> {
    match event {
        FirearmEvent::Malfunctioned(malf) => Some(of_malfunction(*malf, state.is_some_and(|state| state.type_known))),
        FirearmEvent::MalfunctionReminder { malf, type_known } => Some(of_malfunction(*malf, *type_known)),
        FirearmEvent::Appeared => state.filter(|state| state.malf != Malf::None).map(|state| of_malfunction(state.malf, state.type_known)),
        _ => None,
    }
}
