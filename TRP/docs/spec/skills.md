# Skills

What Tarkov's skills change in the character and how they are earned, where each figure lives, and how far the app has them. Written 2026-10-22 and filled 2026-10-23 (0.102.1) from three sources, each named where it is used. **The client's code** (`local\decomp\src`: `SkillManager`, `Skill`, `BaseSkill`, `WeaponSkill`, `Mastering`, and every place a buff is read), which holds the per-level figures and the rules, and **wins wherever the sources differ**. **The install's own figures** (`globals.json`: `config.SkillsSettings` and the `Skill*` keys), read from the user's install on 2026-10-23 and the same as the `4.1.6` tag of SPT's published `server-csharp`, read the day before. **The Tarkov wiki** (the user, 2026-10-23: "the Tarkov wiki's Character skills page and each skill's page"), whose host refuses automated reading (HTTP 402 on 2026-10-23; shut by the network policy on 2026-10-22), so what is had of it came through a search engine's summaries of its pages and of guides that quote it: set beside the code in "The wiki beside the code"; a figure with no code behind it is `PROVISIONAL`. **Compared with the game: nothing.** The user's words: "the audio is actually being modified by my installs levels. For example, covert movement lowers footstep sounds by 49 percent. Other skills affect other things" (2026-10-22); "Fill spec/skills.md with what each skill changes, and how it is earned ... The goal is to make the earnable skills a switch. By default, it should be off" (2026-10-23).

## What a skill is

**A level** is the skill's progress over 100, whole (`BaseSkill.Level`; `SkillExpPerLevel` 100), the progress held between 0 and 5,100: levels 0 to 51, and 51 (above 50: `Skill.IsEliteLevel`) is **elite**. The character's progress lives in the profile (`Skills.Common`, each `Progress`). **A buff** is a figure the level sets (`Skill.UpdateRules`), by one of three rules (`SkillManager.FloatBuff`): *a level* (`PerLevel(f)`: the level times `f`), *to a top* (`Max(f)`: the level times `f / 50`, so `f` at level 50) or *its own* (`Custom`: a formula of the level). The rule is worked with the level held at 50; above 50 an **elite rule** runs after it, which either puts a figure in its place (`Elite(x)`) or turns on a yes-or-no buff (`BooleanBuff`, off below elite). A buff with no rule stays 0 (or its `Default`). A skill with no buffs is **locked** (`Skill.Locked`): shown, never earned.

**A boost to a skill** (`BaseSkill.Buff`, levels added for a while by the game's stimulants: built since 0.135.0, `health.md`, "Buffs") is added to the level the rules use (`SummaryLevel`: the level plus the boost, each buff held to what takes the skill from nought to 60 and rounded, at most 60 where the boosts add up above nought and 51 otherwise), so a boost can make a skill elite for its time. In the app the boosts at work are taken from the health each frame (`skills::stimulated`) and every buff is worked from the summary levels; with `SKILLS` off they do nothing.

**What the skill page shows** is each buff at the character's level, the figure the user reads ("49 percent": Covert Movement's 1.2% a level is 49% at level 41; the user's profile has the skill at level 40, which is 48%, so the two are not yet settled). So what is carried is the per-level figure and the level, never the page's number.

**The skills the client has** (`SkillManager`'s list, `DisplayList`, 43, and two for bots; `GetSkill` looks only there): Physical (Endurance, Strength, Vitality, Health, Stress Resistance, Metabolism, Immunity), Mental (Perception, Intellect, Attention, Charisma), Combat (twelve weapon classes, Throwing, Aim Drills, Troubleshooting), Practical (Surgery, Covert Movement, Search, Mag Drills, Light Vests, Heavy Vests, Weapon Maintenance, Crafting, Hideout Management; and First Aid, Field Medicine, Night Ops, Silent Ops, Auctions, Clean Operations, Shadow Connections, Task Performance, all locked). **Made but left out of the list**, so that no profile's level reaches them and nothing earns them: Recoil Control (whose buff therefore stays 0), Sniping, Prone Movement, Lockpicking, Weapon Modding, Advanced Modding, Free Trading, Barter, and the five BEAR and five USEC skills (locked). **Memory** is gone from the client; the install still holds its figures (`Memory`: `AnySkillUp` 5, `SkillProgress` 0.4), which nothing reads.

## The skills system (built 0.103.0)

The user (2026-10-23): "Build the skills system: a page in the options tab where I set each skill's level 0 to 51 (decisions.md: from the tab; never earning), a switch for the system like the others, and the buffs applied everywhere spec/skills.md says a skill reaches"; "Earned points should be saved. The options tab should update when a skill is leveled. Again, we will disable that option and leave it for host games to alter levels on runtime." So: **the levels are set, never earned** here; a host game may change them in play, and the tab shows the change and keeps it.

- **The levels** (`trp_sim::skills::SkillLevels`): each skill's progress, 0 to 5,100, a level a hundred, whole levels on the tab, a host's fractions kept. Set on the options tab's `SKILLS` page (a bar a skill, 0 to 51, `ELITE` at 51; `ALL TO 0`), kept in the app's file (`"skills"`: each skill not at 0 by the game's name, with its progress), never read from the profile. For a run: `--weapon-skill <0..51>` (every weapon class), `--strength`, `--endurance` (0 to 51), `--skill Name=level` (any, by the game's name); `--skill-at frame:Name=level` sets one in play on a drawn frame, as a host would.
- **In play** (the app's `skills::Skills`, a resource a host game changes): a change is put on at once on what reads it as it goes (`Skill.LevelChanged`), and on a weapon as it is next drawn, as the game's (`SyncWithCharacterSkills`, `UpdateWeaponVariables`); the tab shows it and the app's file keeps it (the tab's own changes are the run's at once, kept as the bar is let go). A run begun again after a death keeps the levels as they stand.
- **The switch** (`SKILLS`, under `SIMPLE`, on as the app comes, restart required): off, every buff is an untrained character's whatever the levels say.
- **The buffs, where they act** (`trp_sim::skills::Buffs`, each by the game's rule, held at 50, its elite above): Endurance on stamina, its two elites too (`StaminaSettings::set_skill_buffs`: the legs', the arms' and the breath's capacity, their recovery, a jump's cost); Strength on the top speeds and the jump (`set_strength`), the weight's limits (`WeightLimits`: the overweight limits times 1 and its lift, the inertia's raised by `InertiaLimitsStep` a level; `Movement::set_weight_limits`), aiming's cost, a high throw's strength, a knife's blow (its damage, as the log writes it), and at elite the weapons weighing nothing (the carried sum, the GEAR and HEALTH tabs' figure, whose maximum takes the lift too); the health skills on health (`HealthSkills::of`, Vitality's and Metabolism's elites too: a bleeding stopping by itself after its `EliteVitalityDuration`, no damage from strong dehydration or exhaustion); Covert Movement on the steps and the kit (`StepSounds::set_covert`: each place the game multiplies by it, with the game's soft surfaces and how quietly one walks; and the stance panel's noise level); a weapon's class on its reload and draw speeds, its ergonomics (the aiming speed, the sway and the ergonomic weight with them) and its recoil (`RecoilForce`: the forces less its share), Aim Drills on the aiming speed, from the draw (`HandsRig::take_skills`); Throwing's elite on a throw's strength; Mag Drills on a round's and a check's time, its elites (rounds quicker after the first; a magazine known on being put into one's own things); Surgery on health (as before). Every buff is 0, or leaves its system as it was, at level 0: the kept regression is unchanged to the byte (`after133`, `after132`'s).
- **Not acting yet**, what they act on not being built: Throwing's cost (a throw's stamina is not paid), Aim Drills' and the classes' stiff breath (`DrawElite`), Perception (the look's finding sphere), Attention and Search (searching and examining), Troubleshooting (malfunctions), Light and Heavy Vests (nothing hits the player's armour), Weapon Maintenance (wear), Stress Resistance's berserk, Immunity's poison and its elite chances (no poisoning, no stimulant ill effects), Metabolism's ill effects' time, the mounted ergonomics (no mounting); and out of a raid, Intellect, Charisma, Crafting, Hideout Management. Mastering stays a flag (`--mastering`).

## The SKILLS tab (built 0.104.0)

The user (2026-10-23): "Build the skills tab in the UI to reflect what skills are enabled"; asked, "enabled" is a skill that acts in the app, and the look is the game's with `TARKOV UI` (`decisions.md`).

- **The page** (`SkillsAndMasteringScreen`; the inventory's scene, `/InventoryScreen/SkillsAndMasteringPanel`): its caption's tabs SKILLS (chosen) and MASTERING (not built: dimmed, 0.3, and does nothing); the experience above (`PlayerExperiencePanel`) as for a character with none (levels 01 and 02, each its rank `rank5`, "current: 0", "remaining:" the second level's experience from `exp_table`, "+0", the bar's fills closed at its left end, `TwoValueBar` at 0), dimmed as the OVERALL tab's statistics; then `SkillsScreen`'s options and its scroll view.
- **The list and the grid** (`SkillList`, `SkillThumbs`, two tabs, the list chosen as the page shows): every skill of `DisplayList` as a view made from its template: in the list (`SkillPanel`) a flexible grid of two columns (cells 824 wide where the content is 1,679, 100 high, 25 apart across and 34 down), in the grid (`SkillIcon`) cells of 96, 24 apart, as many to a row as fit; the content 20 down from the view's top and 20 under its last row; the wheel moves it by the view's step (63), the bar's handle drags it, the bar hidden while all of it shows where the scroll hides it.
- **A skill's view** (`SkillPanel.Show`, `OnSkillLevelChanged`, `SkillIcon.Show`, `SkillLevelPanel.SetLevel`, `SkillClassIcon.Set`): its name; its class's mark and colour (Physical `Ph` 103,177,184; Combat `C` 184,87,57; Mental `M` 220,215,185; Practical `P` 175,220,169; Special `S` 184,156,71); "Level: N" (the install's `Level `) or at elite `ELITE Level` in its colour; its points into the level of what the level takes (`CalculateLevelExp`: 10 a level up to 90 below level 9, then 100; rounded to hundredths) or "maxed"; the fill (`level_fill_bar_new`, the elite's picture at elite, full) and the base fill (the same share, none being earned; none at elite); the icon (`StaticIcons.SkillIdSprites`); the border white, at elite 183,112,0; the level in its corner, the elite's mark in its place at elite; the levelling's rate's arrow at the bar's end in a raid below elite (`_effectivenessUp` over 1, `_effectivenessDown` under). **The rate** (`Skill.Effectiveness`): at none earned, as every skill is at a raid's start (`GetEffectiveness(0)`), the install's `SkillFreshEffectiveness` (1.3) while below `SkillFreshPoints` (1), then 1 for `SkillPointsBeforeFatigue` (1) more, then `SkillFatiguePerPoint` (0.6) to the power of the points past those, at least `SkillMinEffectiveness`; nothing earns here, so every skill below elite shows the arrow up, as the user's capture does on every row. In the list, its buffs' icons (`BuffIcon`): those not hidden from the player, in the order of their kinds (simple, elite, switching, plebian), each shown as `BuffThumb.UpdateVisibility` has it (none at level 0; an elite one at elite only, a plebian one below it only), on the elite's ground where a buff not simple is at elite.
- **Locked, and not acting here**: a skill the game locks (Field Medicine, First Aid, Night Ops, Silent Ops, Auctions, Clean Operations, Shadow Connections, Task Performance) is drawn at 0.3, its view and again its icon (`SetUnlockStatus` on the view's and the icon's `CanvasGroup`), and its icon says "Not available in alpha" in the simple tooltip (`HoverTooltipArea`) instead of the skill's tooltip. By the user's choice a skill whose effects the app does not have (`skills_view::acts_here`: Hmg, Launchers, Melee, Perception, Intellect, Attention, Charisma, Search, Troubleshooting, the vests, Weapon Maintenance, Crafting, Hideout Management) is drawn the same way, its tooltip shown with the reason at the description's end; with the options' `SKILLS` off, every skill.
- **The tooltip** (`SkillTooltip.Show`, the preloader's scene): its corner 25 right of the mouse and 25 down, following it, moved back onto the screen where it would leave it (`CorrectPositionResolution`: the left edge before the right, the bottom before the top, on whole pixels); the skill's name, class, level panel; its description (`<Id>Description`, a blank line, `<Id>LevelingUpDescription`; in a raid below elite a blank line and the rate, "Leveling speed increased: 130%" in `#54C1FF` over 1, `SKILLS_SPEED_UP` and the percent cut to a whole, `SKILLS_SPEED_DOWN` in `#C40000` under it) its lines broken at 400; its fills, level and points; a line for every buff that shows (all of them, hidden ones too, in the order of their kinds, by `UpdateVisibility`): its icon on its ground, its words (`BuffIcon.GetBuffDescription`: the buff's name's words, brackets blue `#54c1ff`, the value in its hole as .NET writes a `Single`, its seven figures rounded half away from zero) with "ELITE" before an elite buff's and a switching one's at elite (`BuffPanel.UpdateBuff`; a buff's icon on the page says "ELITE" for any not simple at elite, `BuffIcon.OnPointerEnter`). Its size is what its groups ask (`ContentSizeFitter`). One buff is saved under another name than its own (`CharismaFenceRepPenaltyReduction`, `JsonEnumName` `CharismaFenceReducePenalty`): its icon is found by that.
- **The sort and the filter** (`SkillContainer`): "Default" (the game's order), "Most leveled" (by level from the most, the game's order among equals), "Most recent" (by when a skill last earned, `LastCall`: nothing earns here, so the game's order, `PROVISIONAL`); "All types" and the five classes in `ESkillClass`'s order (Physical, Combat, Special, Practical, Mental; Special holds none of the list's). Both are put back to their first lines as the page shows, each with its sound (`UpdateValue`), and work as OVERALL's dropdowns (`MenuDropdown` opening, `MenuDropdownSelect` choosing; a list 120 high at most, `_maxVisibleHeight`).
- **Without `TARKOV UI`**: the app's own list, by class in three columns, each skill's level and points in the game's words and under it its buffs' words as they stand, a locked or idle one faint with its reason; the wheel scrolls it.
- **Compared with the game** (0.104.1): the user's capture of the list at 2560x1440 (every skill near 40, the experience at level 70), by eye at 0.96 of its size: the rows, icons, names, bars, levels, points, buffs' icons and their counts, the options' row and the scroll bar where the app draws them; the bars' dark band, the arrows and the caption's icons were missing and are drawn since (the band: a sliced sprite whose middle has no pixels, which the engine stretches from the line between its borders). The chosen view's button is lighter in the capture than in the app: its picture is see-through, over the game's dark backdrop there and the range's sky here. Not compared: the tooltip, the grid, the dropdowns (no capture of them).

## What each skill changes (the code's figures)

"A level" is the buff's growth a level (in brackets, its figure at 50); "Elite" is its figure at 51 where an elite rule sets one, "yes" for a buff that elite alone turns on. "Read nowhere" means no code of the client reads the buff: the skill page shows it, and what it does, if anything, is the server's or nobody's (`PROVISIONAL` as an effect). The spec named last in a row has the effect's detail.

**Endurance**

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `EnduranceBuffEnduranceInc` | 0.01 (0.5 at 50) | 0.7 | the legs' stamina capacity, times 1 + the buff (`stamina.md`) |
| `EnduranceHands` | 0 | 0.5 | the arms' stamina capacity, times 1 + the buff (`stamina.md`) |
| `EnduranceBuffJumpCostRed` | 0.006 (0.3) | | a jump's stamina cost, times 1 − the buff (`stamina.md`) |
| `EnduranceBuffBreathTimeInc` | 0.02 (1) | | the breath's capacity (`stamina.md`, the oxygen's formula) |
| `EnduranceBuffRestoration` | 0.01 (0.5) | 0.75 | the legs' and the breath's recovery, times 1 + the buff (`stamina.md`) |
| `EnduranceBreathElite` | 0 | 1 | the breath's capacity no longer falls with the energy (`stamina.md`: `max(energy, elite)`) |

**Strength** (also: the top walking and sprinting speeds and the speed cap go by Strength's level over 60, through the globals' `WalkSpeed` 0.625 to 0.717 and `SprintSpeed` 0.5 to 1, not by a buff: `movement.md`)

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `StrengthBuffJumpHeightInc` | 0.004 (0.2) | | a jump's push, times 1 + the buff (`JumpPlayerState`; `movement.md`) |
| `StrengthBuffLiftWeightInc` | 0.006 (0.3) | | every overweight limit, times 1 + the buff (`PhysicalBase`; the figure's maximum on the tabs: `inventory.md`, "Weight") |
| `StrengthBuffMeleePowerInc` | 0.006 (0.3) | | a melee blow's damage, times 1 + the buff (`grenades-and-knife.md`) |
| `StrengthBuffThrowDistanceInc` | 0.004 (0.2) | | a high throw's strength, times 1 + the buff (a low throw is 0.66 whatever the level; `grenades-and-knife.md`) |
| `StrengthBuffAimFatigue` | 0.004 (0.2) | | aiming's stamina drain, times 1 − the buff (`stamina.md`) |
| `StrengthBuffElite` | | yes | the weapons, the scabbard, the dogtag and the armband weigh nothing (`inventory.md`, "Weight") |
| `StrengthBuffSprintSpeedInc` | 0.004 (0.2) | | read nowhere: the sprint's speed goes by the level, above |
| `StrengthBuffMeleeCrits` | 0 | 0.5 | read nowhere |

**Vitality**

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `VitalityBuffBleedChanceRed` | 0.012 (0.6) | | the cap on a hit's bleeding odds, 1 + the buff (`health.md`, "Damage") |
| `VitalityBuffBleedStop` | | yes | a bleeding stops by itself, light after 22 s, heavy after 33 s (each bleeding's `EliteVitalityDuration` in the install; `health.md`) |
| `VitalityBuffSurviobilityInc` | 0.004 | | read nowhere |
| `VitalityBuffRegeneration` | | yes | read nowhere |

**Health** (`HealthOfflineRegenerationInc` and `HealthElitePosion` exist but are not the skill's: never set)

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `HealthBreakChanceRed` | 0.012 (0.6) | | the cap on a fracture's odds, from a fall or a hit, 1 + the buff (`health.md`) |
| `HealthEnergy` | 0.006 (0.3) | | existence's energy drain, times 1 − the buff (`health.md`) |
| `HealthHydration` | 0.006 (0.3) | | existence's hydration drain, times 1 − the buff (`health.md`) |
| `HealthEliteAbsorbDamage` | | yes | read nowhere |

**Stress Resistance**

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `StressPain` | 0.01 (0.5) | | a pain's strength on the screen and its time, times 1 − the buff, at least 0.01 (`health.md`) |
| `StressBerserk` | | yes | low health starts a berserk (strength 2) (`health.md`) |
| `StressTremor` | 0.012 | | read nowhere |

**Metabolism** (`MetabolismPoisonTime` exists but is not the skill's: never set)

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `MetabolismRatioPlus` | 0.01 (0.5) | | the energy and hydration food and drink give, times 1 + the buff (`health.md`) |
| `MetabolismMiscDebuffTime` | 0.01 (0.5) | | the time of an effect that is no buff, times 1 − the buff (`ActiveHealthController`; `health.md`) |
| `MetabolismEliteBuffNoDyhydration` | | yes | strong dehydration and exhaustion do no damage (`health.md`) |

**Immunity**

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `ImmunityMiscEffects` | 0.01 (0.5) | | the energy and hydration food and drink take away, times 1 − the buff (`health.md`) |
| `ImmunityPoisonBuff` | 0.01 (0.5) | | an intoxication's damage a loop, times 1 − the buff |
| `ImmunityPainKiller` | 0.006 (0.3) | | a painkiller's time, times 1 + the buff (`health.md`) |
| `ImmunityAvoidPoisonChance` | 0 | 0.9 | a poisoning is avoided nine times in ten |
| `ImmunityAvoidMiscEffectsChance` | 0 | 0.9 | a stimulant's ill effect is avoided nine times in ten |

**Perception** (`PerceptionHearing`, which the sound reads, is not the skill's: never set, so 0)

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `PerceptionLootDot` | 0.02 (1) | | the radius of the sphere the look finds things with, 0.1 m times 1 + the buff (`Player`; `inventory.md`, "Reach") |
| `PerceptionEliteNoIdea` | | yes | that sphere is cast down from 3 m above the character, 1.1 m wide and 2.35 m long (what it then finds: `PROVISIONAL`, not read) |

**Intellect**: `IntellectLearningSpeed` 0.02 a level and `IntellectWeaponMaintance` 0.02, and the elite `IntellectEliteAmmoCounter`, `IntellectEliteContainerScope` and `IntellectEliteNaturalLearner`: read nowhere. `IntellectRepairPointsCostReduction` (`Intellect.RepairPointsCostReduction` 0.004 a level): a repair's cost, out of a raid.

**Attention**

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `AttentionLootSpeed` | 0.02 (1) | | searching a container or a body, faster by the buff (`ActiveSearchContentOperation`) |
| `AttentionExamine` | 0.02 (1) | | examining a thing takes its time over 1 + the buff (`ExamineOperation`) |
| `AttentionEliteLuckySearch` | 0 | 0.5 | a container's first search is over at once, one time in two |
| `AttentionEliteExtraLootExp` | | yes | more experience for what is found (the raid's statistics; out of scope) |

**Charisma**: traders' discounts, 0.001 a level each (`Charisma.BonusSettings.LevelBonusSettings`: healing, insurance, a paid exit, a repeatable task's change), and at elite the scav case 10% cheaper, Fence's standing lost halved, one more repeatable task. All out of a raid.

**The weapon classes** (each class's figures apply to its own weapons, by the weapon's own class: the assault rifles' takes every assault carbine too, `GetWeaponInfo`; the class the game calls `Launcher`, "Grenade Launchers" in its words, is the underbarrel launcher's, a part, `Launcher : GearMod`; a grenade launcher of its own, as the M32A1 (`GrenadeLauncher`), a rocket launcher or a special weapon is of no class and takes none)

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `WeaponReloadBuff` | 0.004 (0.2) | | the reload's animation speed, 1 + the buff (`firing.md`) |
| `WeaponSwapBuff` | 0.008 (0.4) | | the draw's and the put-away's animation speed, 1 + the buff (`firing.md`) |
| `WeaponRecoilBuff` | 0.003 (0.15) (`WeaponSkillRecoilBonusPerLevel`) | | the recoil's suppression, its first part (`ShotEffector`; `weapon-handling.md`) |
| `WeaponErgonomicsBuff` | 0.002 (0.1) | | the weapon's ergonomics, times 1 + the buff (`TotalErgonomicsFunc`) |
| `WeaponDoubleActionRecoilReduceBuff` | 0.01 (0.5), hidden | | a double-action pull's recoil |
| `MountingErgonomicsGainPerLevel`, `BipodErgonomicsGainPerLevel` | 0.2, 0.5 (`MountingErgonomicsBonusPerLevel`, `BipodErgonomicsBonusPerLevel`), hidden | | added to the ergonomics' factor while mounted, on a bipod the second (as read, 0.2 a level would multiply the ergonomics many times over at a high level: `PROVISIONAL` until a mounted recording at a known level shows it) |
| `WeaponDoubleMastering` | 1 | 2 | each shot counts twice for the weapon's mastering (and for Aim Drills, below) |
| `DrawElite` | | yes | with the sights brought up, the breath held stiff for 3 s (`ProceduralWeaponAnimation`: `Breath.StiffUntill`); the wiki calls it a steady aim at any stamina (`PROVISIONAL`: the breath's side not read) |

Mastering's level (0 to 2) adds 0.05 a level to how fast one walks with the sights up (`SetAimingSlowdown`: 0.33 plus it) and picks the reload's clips (`firing.md`). The bots' two skills (`BotReload` 0.1 a level, `BotSound` 0.02) are the bots' only.

**Throwing**: `ThrowingEnergyExpenses` 0.01 a level (0.5): a throw's stamina cost, times 1 − the buff; elite `ThrowingEliteBuff`: no cost, and the throw's strength no longer cut by the arms' stamina (`grenades-and-knife.md`). `ThrowingStrengthBuff` 0.005: read nowhere.

**Aim Drills**: `DrawSpeed` 0.01 a level (0.5): the speed of bringing the sights up, times 1 + the buff (`ProceduralWeaponAnimation`); `DrawSound` 0.01 (0.5): the sound of it, as others hear it; elite `DrawElite`: as the weapon classes' (stiff breath); elite `DrawTremor`: read nowhere.

**Recoil Control**: `RecoilControlImprove` 0.003 a level (`RecoilControl.RecoilBonusPerLevel`) is the recoil suppression's second part, but the skill is out of the list (above): it stays 0. `RecoilControlElite`: not the skill's.

**Troubleshooting**: `TroubleFixing` 0.005 a level (`MalfRepairSpeedBonusPerLevel`): clearing a malfunction faster, 1 + the buff. Elite: after a malfunction, that weapon's later malfunctions from the same cause (its wear, its magazine, its round) are 0.3 as likely (`EliteDurabilityChanceReduceMult`, `...MagChance...`, `...AmmoChance...`; turned on by `TroubleFixingDurElite` alone; `TroubleFixingMagElite` and `...AmmoElite` are read nowhere), and an examined malfunction's kind is known at once (`TroubleFixingExamineMalfElite`). Malfunctions are not built.

**Covert Movement** (`footsteps.md` has where each lands)

| Buff | A level | Elite | What it changes |
|---|---|---|---|
| `CovertMovementSoundVolume` | 0.012 (0.6) | | a step on a common surface, times 1 − the buff × how quietly one walks |
| `CovertMovementLoud` | 0.006 (0.3) | 0.6 | a step on a loud surface, the same way: at elite as quiet as on a common one |
| `CovertMovementEquipment` | 0.012 (0.6) | | the kit's noise, 1 + overweight − the buff |
| `CovertMovementSpeed` | 1 + 0.01 a level (1.5) | | how slowly one must walk to walk quietly: quiet in full below 0.4 × the buff − 0.2 of the top speed, not at all above 0.4 × the buff (0.2 to 0.4 untrained, 0.4 to 0.6 at 50); none with a damaged leg, less by the overweight |
| `CovertMovementElite` | | yes | read nowhere |

**Surgery**: `SurgerySpeed` 0.004 a level (0.2), elite 0.4: a surgery's time over 1 + the buff, its animation that much faster; `SurgeryReducePenalty` 0.01 a level (0.5), elite 1: the most health a restored part loses is cut by the buff (none at elite) (`health.md`).

**Search**: `SearchBuffSpeed` 0.01 a level (0.5): searching **a body** faster by the buff (a container: Attention alone); elite `SearchDouble`: two searches at once (`PlayerSearchController`).

**Mag Drills** (`inventory.md`, the magazine's processes): `MagDrillsLoadSpeed` and `MagDrillsUnloadSpeed` 0.6 a level, `MagDrillsInventoryCheckSpeed` 0.8: a round's time, or a check's, times (100 − the buff + the magazine's own modifier) / 100; elite `MagDrillsLoadProgression` (rounds after the first quicker) and `MagDrillsInstantCheck` (a magazine is known on being put into one's own things); `MagDrillsInventoryCheckAccuracy` (the level over 10, at most 2): read nowhere.

**Light Vests and Heavy Vests** (`body-hits.md`): a piece's speed and turning penalty, times 1 − 0.006 a level (light) or 0.005 (heavy); a blow's damage through light armour less by 0.006 a level, blunt damage through heavy armour times 1 − 0.004 a level; at elite, no bleeding under a light piece not worn out, and a heavy piece deflects a shot one time in twenty (`RicochetChanceHVestsEliteLevel` 0.05, above half its durability). Their repair figures (`WearAmountRepair...`, the elite's deterioration odds) are out of a raid.

**Weapon Maintenance** (`WeaponTreatment`): a shot's wear on the weapon less by 0.005 a level (`DurLossReducePerLevel`; `Weapon.OnShot`); the rest is repairs, out of a raid.

**Crafting and Hideout Management**: the hideout's (crafting times 0.75% less a level, fuel and the like 0.5% less a level, more slots at elite); out of scope.

**Not the character's, listed so that nothing is missed**: repairs and wear (`Intellect.WearAmountReducePerLevel` 0.004, `Intellect.WearChanceReduceEliteLevel` 0.5; `WeaponTreatment.WearAmountRepairGunsReducePerLevel` 0.01, `...WearChanceRepairGunsReduceEliteLevel` 0.5; the vests' `WearAmountRepair...ReducePerLevel` 0.01 heavy, 0.008 light, `WearChanceRepair...ReduceEliteLevel` 0.5; the `BuffSettings` of `HeavyVests`, `LightVests`, `WeaponTreatment` and `Melee`, the odds of a repair's buff), the hideout (`Crafting`, `HideoutManagement`), the traders (`Charisma.BonusSettings`).

## How a skill is earned (the code's rules, the install's figures)

**An action gives points.** The game raises an action where something happens (a step's end, a shot, a reload), with an amount (metres, one, a count, seconds), and each skill that listens takes the amount times its factor (`SkillAction.Factor`). The factor is the skill's figure in the install times **`SkillProgressRate` 0.4**, or for the weapon classes and mastering times **`WeaponSkillProgressRate` 1**. A skill whose figure is a formula of the movement (`SkillDependency`) takes the formula's figure times 0.4.

**Points pass through the session's fatigue** (`Skill.UseEffectiveness`, `SkillManager.GetEffectiveness`), in steps of at most one point, each step times the skill's **effectiveness** of the moment, which goes by the points this skill has taken in the session: below `SkillFreshPoints` (1), `SkillFreshEffectiveness` 1.3; below that plus `SkillPointsBeforeFatigue` (1), 1; then `SkillFatiguePerPoint` 0.6 to the power 1 + the points − 2 (0.6 at 2, 0.36 at 3, 0.01 at 11), never under `SkillMinEffectiveness` 0.0001. Once an award has left it at 1 or less, the fatigue lasts until **`SkillFatigueReset` 200 seconds** (the engine's clock) pass with no award to that skill: then the effectiveness is 1 again and the session's count at most 1. The count is kept in the profile between raids (`PointsEarnedDuringSession`; whether the server clears it after a raid: `PROVISIONAL`). Then the hideout's bonuses (`BonusController.Calculate`, none in the app).

**The first nine levels are cheaper** (`CalculateExpOnFirstLevels`, `CalculateLevelExp`): below level 9 a level takes (level + 1) × 10 points instead of 100 (10 to reach 1, 90 to go from 8 to 9; 450 to reach 9), and a point is worth 10 / (level + 1) of progress. **At elite the skill stops listening** (`UpdateRules`: above 50, `Unsubscribe`): nothing more is earned. **No skill falls**: the install's `SkillAtrophy` is false and the client reads no such rule.

**Some skills are fed by others** (`SkillManager.SkillProgress`, raised with each award as the action gave it, before the giver's fatigue; the fed share then passes the taker's own fatigue): Health takes 0.5 × 0.4 = 0.2 of Strength's, Endurance's and Vitality's points (`Health.SkillProgress`); Charisma 0.2 of Intellect's, Attention's and Perception's (`Charisma.SkillProgressInt`, `...Atn`, `...Per`); Surgery 0.2 of First Aid's and Field Medicine's (none: both locked); Intellect 0.2 of Lockpicking's (none) and a fifteenth of Crafting's (`CraftingPointsToInteligence` 15, times 0.4, no fatigue). The install's `DependentSkillRatios` (0.25 to Health, 0.2 to Charisma, 0.066 to Intellect) are not the client's: it reads none of them.

**The server's skills**: Crafting, Hideout Management and Weapon Maintenance cannot be earned in the client (`ClientAuthorizedSkill`: "can not be progressed locally"); the hideout and the repairs earn them.

**Each skill's actions** (the amount is what the game passes; "points" is the amount's worth before fatigue and below level 9; every action is the player's own in a raid):

| Skill | Action (when the game raises it) | Points |
|---|---|---|
| Endurance | metres walked, at the end of a walk, and metres sprinted, at the end of a sprint, on the ground; none while overweight | 0.005 × 0.4 a metre walked, 0.04 × 0.4 sprinted, each times 1 + 0.05 × the chronic fatigue's strength (`GainPerFatigueStack`) |
| Strength | the same metres, only while overweight; a throw; a melee blow; push-ups at the hideout's gym | walking `lerp(0.005, 0.015, overweight)` × 0.4 a metre, sprinting `lerp(0.04, 0.12, overweight)` × 0.4; a throw and a blow 0.2 × 0.4 each (the code's own 0.2, the install's the same); the gym is out of scope |
| Vitality | damage taken, by its amount; a bleeding begun | 0.005 × 0.4 a point of damage; 0.5 × 0.4 a bleeding |
| Health | fed (above) | |
| Stress Resistance | a pain begun, not on painkillers; the time at low health (`LowEdgeHealth`) | 0.33 × 0.4 a pain; 0.033 × 0.4 a second |
| Metabolism | energy or hydration gained, by the amount | 20 × 0.4 a point gained (`EnergyRecoveryRate`, `HydrationRecoveryRate`) |
| Immunity | an intoxication begun; the time under a stimulant's ill effect | 0.5 × 0.4 an intoxication; 0.011 × 0.4 a second |
| Perception | the raid's length, at its end; a thing the raid's statistics count as unique loot (`OnUniqueLoot`; which things: not read) | 2.5 × 0.4 an hour; 0.334 × 0.4 a thing (the code's own figures, the install's the same) |
| Intellect | examining a thing; a repair; fed | 20 × 0.4 and 0.005 × 0.4: neither is raised by the client in a raid |
| Attention | a thing found in a search: on a body, elsewhere; examining with an instruction | 0.167 × 0.4 on a body, 0.334 × 0.4 elsewhere; 3 × 0.4 (not raised in a raid) |
| Charisma | fed; the traders (`Counters`: the server's) | |
| a weapon class | a reload, a shot, a round chambered, with that class's weapon | the class's `WeaponReloadAction`, `WeaponShotAction`, `WeaponChamberAction`, × 1; a shot counts 2 at the class's elite |
| Throwing | a throw | 0.25 × 0.4 |
| Aim Drills | a shot within the first two seconds of bringing the sights up (`FastAimTimer`: down to 0 at 1 a second while aiming, back up to 2 at 1 a second while not, so sooner after sights lowered briefly) | 0.2 × 0.4 a shot (two at the shooting weapon's class's elite) |
| Troubleshooting | a malfunction cleared | 3.2 × 0.4 (`SkillPointsPerMalfFix`) |
| Covert Movement | metres walked quietly (the step's quiet share below 1) | 0.025 × 0.4 a metre |
| Surgery | a surgery done; fed | 3 × 0.4 a surgery |
| Search | a container or body searched to the end; a thing found on a body | 0.33 × 0.4; 0.166 × 0.4 |
| Mag Drills | rounds loaded and unloaded in a raid, by the count; a magazine checked | 0.02 × 0.4 a round loaded, 0.01 × 0.4 unloaded; 0.05 × 0.4 a check |
| Light Vests, Heavy Vests | a light or heavy piece's durability lost, by the amount | 0.05 × 0.4 (light), 0.025 × 0.4 (heavy) a point (the code's own; the install's `Counters` are the server's) |

**The weapon classes' figures, as the client takes them** (reload / shot / chamber, from the install; `WeaponsSettingExtension`, `CreateCombat`): Assault 0.025 / 0.1 / 0.1, Pistol 0.025 / 0.1 / 0.1, Revolver 0.025 / 0.1 / 0.1, Shotgun 0.01 / 0.1 / 0.1, Sniper 0.02 / 0.2 / 0.025, DMR 0.02 / 0.2 / 0.025. **The client gives SMGs and LMGs the Assault figures, Launchers and Melee the Pistol's**; the install's own SMG (0.1 / 0.4 / 0.4), LMG and HMG (0.15 / 0.6 / 0.6) and launchers' (0.275 / 1.1 / 1.1) figures are not read, nor any class's `WeaponFixAction` (a malfunction is Troubleshooting's). The HMG and Attached Launcher skills are tied to no weapon (`typeof(int)`, `typeof(float)`): nothing earns them, and their buffs reach no weapon. A knife neither reloads, fires nor chambers: Melee earns nothing.

**Mastering** (`Mastering`, `MasterNewWeapon`): a weapon's group (`config.Mastering`, 79 groups) is begun when one of its weapons is first drawn; each shot with a weapon of the group gives a point (× `WeaponSkillProgressRate` 1; two at the class's elite), with no fatigue and nothing cheaper; level 1 at the group's `Level2` points, level 2 at `Level2` + `Level3` (the M4's: 1,600, then 3,600).

**What the install holds that the client does not read**: `DependentSkillRatios`, `SkillEnduranceWeightThreshold` 0.65 (the client's own default is 0.75, unused), `SkillAtrophy`, `RecoilControl.RecoilAction` 0.00015 (no skill listens to recoil), `QTELevelMultipliers` (the gym's), `Metabolism`'s three duration rates (`DecreaseNegativeEffectDurationRate`, `DecreasePoisonDurationRate`, `IncreasePositiveEffectDurationRate`, all 1), `Immunity`'s `ImmunityMiscEffects`, `ImmunityPainKiller`, `ImmunityPoisonBuff` (1 each), `Memory`, and the weapon classes' figures named above. Actions the game raises that no skill listens to: recoil, crawling, jumping, holding the breath, drawing, aiming, a shot after aiming, picking a lock, trading, exhaustion, dehydration, a skill's level rising, and Sniping's kill (its skill out of the list). Nineteen skills have no settings at all in the file (the locked ones, the bots' and Sniping).

## The wiki beside the code

Read on 2026-10-23 through search summaries of the English wiki's pages (each about 85 days old in the search index, so from about late July 2026; no page's own date seen) and of TarkovForge, a site with a page a skill (undated); a guide with invented elites (kappaguide) was set aside. The summaries garble a sign now and then (a "+" for a reduction): the code settles the sign. **Since the snapshot** the live game's skills level "up to 60% faster" (Battlestate, 2026-10-02, as quoted by a news site): the user's SPT client is older, and the figures above are its own.

**The same in both**: Endurance's four figures, its elite's 70% and its breath free of energy; Strength's jump, carry, melee, throw and aiming figures and the weightless weapons; Vitality's bleeding odds; Health's fracture, energy and hydration; Stress Resistance's pain and the berserk; Metabolism's three; Immunity's poison, painkiller and the elite's two nine-in-tens; Perception's radius; Attention's three; Charisma's; the weapon classes' reload, swap, recoil and ergonomics, and the doubled mastering; Throwing's cost and its elite; Aim Drills' speed and sound; Troubleshooting's speed and the elite's known kind; Covert Movement's five, the elite as quiet everywhere at 60%; Surgery's four; the vests'; Weapon Maintenance's wear; Mag Drills' elites; every elite worth the per-level figure × 50, as the code holds the level at 50. The rules: 10 points for level 1 and 10 more a level up to 100 (550 to level 10), elite at 51, boosts to 60, the 200 s reset with the count back to 1, no atrophy (gone since 0.8, 2018). Recoil Control and Memory removed (the wiki: in 0.14.5, 2024-04-03), Heavy Machine Guns, Underbarrel Launchers and Melee doing nothing, the locked skills unbuilt: as the code. Earning that agrees: Vitality's 0.002 a point of damage, Immunity's about 0.0045 a second of an ill effect, the vests' 0.02 (light) and 0.01 (heavy) a point of durability, 20% of Attention's and Perception's to Charisma.

**Where they differ, and the code wins**:

- **Strength's movement and sprint speed** (the wiki: 0.4% a level, 20%): the buff is read nowhere; the speeds go by the level over 60 (above).
- **Endurance's breath recovery** (the wiki: its time 1% less a level, 50%): a rate, times 1 + 0.01 a level, 0.75 at elite.
- **Vitality**: the survival buff is 0.4% a level, not the wiki's 0.5% (whose own 20% at 50 is the code's), and it and the elite regeneration are read nowhere; the elite stops a light bleeding after 22 s and a heavy one after 33 s (the wiki: about 20 and 30); a bleeding gives 0.2 points (the wiki: 0.3).
- **Health takes 20%** of Strength's, Endurance's and Vitality's points (the wiki: 25%, the install's unread `DependentSkillRatios`); **Charisma 20% of Intellect's** (the wiki: 10%).
- **Stress Resistance**: the tremor buff is read nowhere; a second at low health gives 0.0132 points and a pain 0.132 (the wiki: 0.033 and 0.33, the install's figures before the 0.4 rate).
- **Immunity's first buff** cuts what food and drink take of the energy and hydration (the wiki: every ill effect of stimulants, food and drink, whose time is Metabolism's).
- **Intellect**: the examining and repairing buffs are read nowhere; a repair kit's cost is 0.4% less a level (the wiki: 0.5%).
- **Attention**: a thing found gives 0.134 points in a container and 0.067 on a body (the wiki: 0.08).
- **The weapon classes are earned by shots**, hitting or not, with reloads and rounds chambered (the wiki: hits).
- **Throwing's strength** (the wiki: 0.5% a level): read nowhere.
- **Aim Drills**: its elite is the weapon classes' own 3 s (the wiki: 2 s); a shot in aiming's first two seconds gives 0.08 points, hit or not (the wiki: 0.2 a hit while aiming).
- **Troubleshooting's elite** makes a malfunction from the same cause 0.3 as likely (the wiki: half).
- **Surgery** gives 1.2 points a surgery (the wiki: 1.1).
- **Search** does speed the search of a body (the wiki: nothing in practice); a container's goes by Attention alone.
- **Mag Drills** shortens a round's time by 0.6% a level and a check's by 0.8% (the wiki's "speed"; 30% and 40% less time at 50); the check's accuracy buff (the level over 10, to 2: the wiki's rough count at 10 and exact at 20) is read nowhere by that name (`inventory.md` has the check).
- **The fatigue** is 1.3, 1, then 0.6 to the power of the points less 1 (60%, 36%, 22%), as the wiki's Strength page and the guides have it; not the steps on its skills page (45%, 20% and 9% after about 5, 6 and 7 points).
- **No per-raid cap** on Strength's points from throws and blows (TarkovForge: 3 each): the fatigue alone.
- **A boost past level 50 does make a skill elite** while it lasts (the Japanese wiki says not).
- **Who earns while walking**: Endurance not while overweight, Strength only then (the wiki's "yellow number"); Covert Movement while the step is quiet at all (the wiki: under a quarter of the speed with no bar on the sound's icon, which is the same rule seen from the screen: `PROVISIONAL`).

**The wiki's alone, with no code behind them** (`PROVISIONAL`, none of them the character's in a raid): the hideout's boosts to points (the shooting range +10% to Combat, the library +30% to Practical, the air filter with a filter in +40% to Physical, the defective wall 3% less a level to Physical; `BonusController`, not read); the gym (2.4, 2.0 and 1.6 points a success by level); Crafting's and Hideout Management's points; Charisma's from traders; the repairs' enhancements from level 3; Perception's elite as a notice of loot nearby (the code's sphere above).

## Not built

**The earning** (the actions above): not built, and to stay so (the user, 2026-10-23: "we will disable that option and leave it for host games to alter levels on runtime"); a host game levels a skill by setting it, and the points it sets are kept. The buffs whose systems are not built (above). The SKILLS tab's MASTERING page, the stimulants' skill boosts on the views (`SkillBuffIcon`, the level's colour), the points earned in a raid (`ProgressValue`, the effectiveness arrows: nothing earns). A comparison of the sounds' levels with a recording (course 7) applies the recording character's skill buffs first (`backlog.md`, 2026-10-22). **Compared with the game: nothing**; every figure is from the code, and the regression's replay gives the new buffs level 0 (it sets only what it set before: Strength's speeds and Endurance's and Strength's stamina from a recording).
