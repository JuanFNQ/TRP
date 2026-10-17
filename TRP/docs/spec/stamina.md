# Stamina

What the character pays for moving and for holding a weapon up, and what running out does. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly: `Physical`, `PhysicalBase`, `Stamina`, and the callers in `MovementContext`, the movement states and the firearm controller). Class and field names are the game's. Figures marked "here" are this install's (`globals.json`, `config.Stamina` unless another section is named).

## Three pools

The character has three pools of the same kind (`Stamina`): **legs** (`Physical.Stamina`, the game's "base"), **arms** (`HandsStamina`) and **oxygen** (`Oxygen`). Each has:

- how much is in it (`Current`), starting full;
- a capacity and a rate of refilling, both worked out from settings, skills and the character's condition (below) and kept until something they depend on changes;
- a moment before which it does not refill (`DisableRestoration`, on the engine's clock);
- a list of the drains running on it;
- a multiplier, which is the health system's stamina coefficient: 1 for a player in an ordinary raid.

A pool under 15 is **exhausted** (`EXHAUSTED_THRESHOLD`). The character as a whole counts as exhausted when the legs or the oxygen are.

## A drain

Everything that costs stamina is one of sixteen `Consumption` objects. Each says:

- what it costs (`Delta`): per second while it runs, or all at once when it is charged;
- which pools it is charged to first (`PrimaryTarget`);
- how full a pool must be for it to start (`StartThreshold`, compared with *more than*);
- whether it may start at all (`Availability`);
- which pool running dry stops it (`Requires`);
- how long after it the pool does not refill (`Downtime`), unless it lets the pool refill regardless (`AllowsRestoration`).

There are two ways to apply one. **Run it** (`SetActive(true)`): it is put on the list of each pool it targets and costs `Delta` a second there until taken off. **Charge it** (`Consume`): `Delta` comes off at once.

A cost is either worked out every time it is asked for or kept until told it is stale (`Compute` and `Deferred`); the per-second ones are all kept. What marks each stale is under "Figures the game keeps".

| Drain | Cost | Charged to | Starts above | Stops when dry | No refill for | Refill regardless |
|---|---|---|---|---|---|---|
| Sprint (run) | `(Overweight + hydration) * SprintDrainRate` a second: 4 here, unburdened | legs, oxygen | 15 | legs | 1 s | no |
| Sprint spam (charged when a sprint starts) | 0 if no sprint started in the last 2 s, else 0.5 more than last time, up to 5 | legs | | | 0 | no |
| Jump (charged) | `(Overweight + hydration) * JumpConsumption * (1 - Endurance's jump buff)`: 14 here, untrained; doubled with a damaged leg | legs, oxygen | 15 | | 1 s | no |
| Walk (run, while the movement state is walking) | upright: `WalkConsumption` at the walk's overweight, and nothing when that is 0; fully crouched: `CrouchConsumption` at the overweight (0.17 a second unburdened); lying: 0 | legs | | | 0.1 s | no |
| Aim (run, while the sights are up) | `sqrt(weapon's ergonomic weight) * AimDrainRate * AimConsumptionByPose[stance class] * (1 - Strength's aim buff) * hydration`, times 1.5 or 2 with one or both arms damaged | arms | | | 0 | no |
| Stand up (charged when the prone flag drops) | `StandupConsumption` at the overweight: 10 unburdened | legs, arms, oxygen | | | 1.5 s | only when overweight is 1 |
| Stance raised (charged per third of a notch) | `PoseLevelConsumptionPerNotch` at the overweight (1.1 unburdened) times the notches | legs | | | 0.5 s | only when overweight is 1 |
| Overweight idle (run while overweight is 1) | the legs' own refill rate plus `OverweightConsumptionByPose[stance class]` | legs | | | | yes |
| Crawl (run in the crawl, only when overweight is 1) | `ProneConsumption` | legs, arms, oxygen | | | 0.5 s | no |
| Hold breath (run) | `BaseHoldBreathConsumption * HoldBreathStaminaMultiplier` at how full the legs are | oxygen | 15 | oxygen | 0 | no |
| Melee, a wound, a fast weapon switch, a vault's and a climb's legs and hands | not read further | | | | | |

`hydration` is a factor from how much the character has drunk (`StaminaDrain.GetAt`): 1 from 90% hydration up, rising to 1.2 just above none and 2 at none. `Overweight` is 0 up to the lower `BaseOverweightLimits` (26 kg here) and 1 at the upper (77).

**The stance class** (`MovementContext.PoseToInt`, handed to `Physical.UpdatePose` whenever the stance target or the prone flag is set): 0 lying, 1 with the stance *target* below 0.11, else 2. So "crouched" for stamina is only the lowest notch.

**Charging to the arms spills.** A charge whose targets include the arms is taken from the arms, and if the arms could not pay all of it the *whole* cost is taken from the legs again, on top of what the legs already paid if they were a target too. **Running on the arms spills too:** a drain run on the arms is put on the legs and the oxygen as well from the moment the arms run dry (so aiming with spent arms drains the legs).

## A pool, each frame

`Player.ComplexUpdate` calls `Physical.Update(dt)` once a frame with the player's own frame time (the whole-millisecond clock of `movement.md`). It comes after the keys have been taken and the movement context has had its update (the stance smoothed, the speed limits looked at) and after the weapon's logic, and **before the body's animator runs**: the animator is the engine's own and is worked after every script's update. So whatever follows from a movement state the animator enters in a frame reaches the pools a frame late or at once, by what it is: a drain *run* from then on (the walk's, as the walking state is entered; the aim's stopped, as a jump puts the sights down) first shows in the next frame's update, while a cost *charged* on entering (the jump's) comes off there and then. Recorded: on the frame a jump begins with the sights up, the legs drop by the jump's cost and the arms still pay that frame's aiming (eleventh recording, 31).

It works the legs, the arms and the oxygen in that order, each the same way:

1. Add up the cost a second of every drain on the pool's list. Any of them that costs something and does not let the pool refill sets the moment refilling may resume to *now + the longest such drain's downtime + (1 - how full the pool is)*. "Now" is the engine's clock (`Time.time`).
2. If the sum is above 0, take `sum * dt` off. A pool that reaches 0 **expires**: every drain that requires that pool is taken off all lists (a sprint ends when the legs are spent).
3. If the pool is below its capacity: add the buff's refill (medicine; 0 otherwise), and if the engine's clock is past the moment of step 1, add `refill rate * dt`, to the capacity at most.
4. Clamp to the capacity.

**A charge** (`Consume`) takes the cost off at once, to 0 at least, and, if the cost was above 0 and the drain does not let the pool refill, sets the moment refilling may resume to *now + that drain's downtime + (1 - how full the pool is after the charge)*. A pool brought to 0 expires as above.

So the pause before refilling is longer the emptier the pool: a full second longer at empty than at full. And a running drain renews the pause every frame, so the pause counts from the drain's last frame.

**Starting** (`CanAdd`): a drain may start on a pool only while the pool holds *more than* its start threshold, and its own condition allows. A sprint and a jump need more than 15 in the legs.

## Capacity and refill

- Legs: capacity `Capacity * (1 + Endurance's capacity buff) + medicine`, at least 1 (115 here, untrained). Refill `BaseRestorationRate * RestorationMultiplierByPose[stance class] * energy * (1 + Endurance's restoration buff) / hydration` (4.5 a second standing, 5.6 fully crouched, 6.5 lying, here, untrained, fed and watered).
- Arms: capacity `HandsCapacity * (1 + the elite Endurance buff)` (80). Refill the same formula on `HandsRestoration` (2.1 standing).
- Oxygen: capacity `OxygenCapacity * (0.5 + 0.5 * max(energy, elite) * (2 * Endurance's breath buff + health))` (350 at full health and energy). Refill `OxygenRestoration * (1 + Endurance's restoration buff)` (4).

`energy` is `StaminaRestoration.GetAt` of how well fed the character is: 1 between 10% and 90%, and **0.917 at 100%** (the curve falls from 1 at 90% toward 0.5 at 150%), so a character that has just eaten its fill refills 8% slower than one a little hungry.

The skills' buffs grow by the level, reaching their full value at level 50: Endurance's capacity buff 0.5, its jump buff 0.3, its restoration buff 0.5, its breath buff 1; Strength's aim buff 0.2.

## Figures the game keeps

A kept cost (`Deferred`) is worked out the next time it is read after being marked stale, and not again until marked again. What marks which:

| Marked stale by | Costs |
|---|---|
| a change of hydration (`OnHydrationChanged`) | the jump's and the sprint's, and both refills |
| a change of energy (`OnEnergyChanged`) | both refills, the overweight idle's, the oxygen's capacity |
| a change of stance class (`UpdatePose`) | the overweight idle's, aiming's, the walk's, both refills |
| a change of carried weight | the overweight idle's, the walk's, the sprint's, a vault's and a climb's |
| `Physical.Aim` being called (the sights up or down, or the weapon taken stock of again: `RecalculateErgonomic`) | aiming's |
| an arm's damage changing, Strength gaining a level | aiming's |
| a vault or a climb beginning | that move's own |

**Aiming's cost has the hydration factor in it and is not marked stale by a change of hydration.** So the factor in force is the one from when the sights last went up (or the stance class last changed, or the weapon was last taken stock of), however thirsty the character has grown since. The health system lowers hydration in steps about a minute apart, so it shows only when a step falls inside a spell of aiming: recorded once (twelfth recording, scenario 51: hydration stepped from 0.793 to 0.783 with the sights up, and the arms went on draining at the old 0.2405 a second for the remaining six seconds where the new factor would have made it 0.2414).

## Who calls it

- **The sprint.** `MovementContext.EnableSprint(on)` calls `Physical.Sprint(on)`. Turning it on runs the sprint drain if the legs hold more than 15 and its condition allows (not lying, overweight under 1, no leg damage or medicine in use), and charges the spam cost. `Physical.Sprinting` is whether the sprint drain is on the legs' list, which is the flag `movement.md` calls the sprint's ("The sprint is a flag before it is a state"): the movement states read it every frame, so a sprint whose drain was taken off because the legs ran dry ends on the next frame like one whose key was let go.
- **A jump.** `MovementContext.TryJump` asks `Physical.CanJump` (more than 15 in the legs, and not pressing uphill on a slope) before raising the animator's flag. Entering the jump state charges the jump (`MovementContext.OnJump`).
- **The stance.** `SmoothPoseLevel`, each frame the smoothed stance rises, adds the rise to a tally and, for each whole third of a notch in it (`DELTA_LEVEL / 3`), charges the stance drain for that many thirds. Lowering the stance costs nothing.
- **Getting up.** Setting the prone flag down charges the stand-up drain, at the key, before the body starts to rise.
- **Walking.** `Physical` listens for changes of movement state: entering the walking state runs the walk drain, leaving it stops it; the crawl likewise, when overweight.
- **Aiming.** The firearm controller's aim setter calls `Physical.Aim(ergonomic weight)` with the sights up and `Aim(0)` with them down, which runs or stops the aim drain. The weight is the one `weapon-handling.md` describes ("When the weapon's weight is taken"). The same call is made, with the sights as they are, whenever the weapon is taken stock of again (`RecalculateErgonomic`: drawn, modified, a reload's end), so sights held up through a reload's end go on at the new weight.
- **A vault and a climb.** Entering either's movement state charges it (`OnVault`, `OnClimb`), and the choice of the move first asks whether the legs, and the arms where the obstacle takes a hand, hold more than 15 (`CanVault`, `CanClimb`). The costs and what they go by are in `vaulting.md`, "Stamina".

## What running out does

- No sprint and no jump start with 15 or less in the legs; a sprint under way ends when the legs reach 0.
- With the legs exhausted the stance changes as slowly as at nine tenths overweight (`PoseLevelIncreaseSpeed`, `PoseLevelDecreaseSpeed` taken at `max(Overweight, 0.9)`) and going down and getting up play as slowly as fully overweight (`TransitionSpeed` at `max(Overweight, 1)`: 0.66 here).
- While the legs are exhausted a fatigue tally runs up, and at `FatigueAmountToCreateEffect` (15 s here) the health system is given a fatigue effect. Not read further.
- The weapon's breathing reads the pools every step: the arms' level (and the legs', here) makes the hands shakier, the oxygen's makes the breath deeper and quicker. And a breath can be held at the oxygen's cost. Both are `weapon-handling.md`'s ("Breathing"), read and built on 2026-10-10. The hand shake effector is not tiredness: it follows a fast weapon switch.
- Melee is slower and weaker with the arms exhausted.

## The game's own display

`BattleStancePanel`, the cluster at the bottom left of the game's screen, shows exactly these:

- two filled bars, the legs' and the arms', each `Current / capacity`, each switching to another look while its pool is under 15;
- a stance slider set to the smoothed stance (or -0.35 lying), snapped to the nearest of a set of figures drawn for it;
- a speed slider whose value is the speed setting in force (the sprint's pace while the sprint flag is up, otherwise the clamped speed) and whose top is the character's top speed setting, over a row of 21 notches (every 0.05 and one at 1) of which those up to *the state's speed limit over the top speed* are lit.

## Built

`crates/trp-sim/src/stamina.rs`: the three pools and the drains above for a sprint and its spam, a jump, walking fully crouched or over the walk's weight, aiming, getting up, raising the stance, the overweight idle and the crawl's; the refill with its pause; expiry. `Movement` owns it (`Movement::stamina`) and calls it where the game's movement code does: the sprint's flag is now the stamina's (`enable_sprint`), a jump is refused at 15 and under, and spent legs slow the stance and the two prone transitions. A host says whether the sights are up (`Input::aiming`, as before) and what the weapon's ergonomic weight is (`Stamina::aim_mass`), and sets the skills with `StaminaSettings::set_skills` (the app: `--endurance`, `--strength`).

The app shows it as the game's panel does (`crates/trp-app/src/hud.rs`): at the bottom left a column for the stance, a row of 21 notches for the speed setting with those the state allows lit, and two bars for the legs and the arms that turn red under 15. Plain bars; the game's own art for the panel is not read.

A vault's and a climb's gate and charges are in since 2026-10-10 (`Stamina::can_vault`, `vault`, `climb`; `vaulting.md`). The cost of aiming is kept as the game keeps it (`Stamina::aim` works it out afresh when the sights or the weight change, `set_pose` when the stance class does, `resettle` when the skills do; a change of hydration alone does not reach it).

Taken as constant: hydration, energy and health (a host may set the two factors), no medicine, no damaged limbs. **Since 0.10.0, with the options' `HEALTH SYSTEM` on, health gives them** (`health.md`, "What health does to the character"; `Stamina::set_health`, `set_aim_factor`, `health_lets_sprint`): energy and hydration as they fall and are eaten and drunk back, the common health's share in the oxygen's capacity (re-read only on a change of energy, as the game keeps it), damaged arms' 1.5 or 2 on aiming's cost, and no sprint paid for with medicine at work or a damaged leg without a painkiller; and 15 s on spent legs gives health a fatigue (`take_fatigues`). From the code; compared with nothing.

Holding breath is in since 2026-10-10 (`Stamina::hold_breath`: 4 a second off the oxygen on full legs, 8 on empty ones; `weapon-handling.md`, "Breathing", has its rules), and the weapon's breathing takes the pools' levels (`Stamina::breath_stamina`, `breath_oxygen`).

The carried weight can change while running since 2026-10-11 (`Stamina::reweigh`, called by `Movement::set_weight` when the inventory's sum changes): how overweight the character is, for everything and for an upright walk, is set afresh, the pools keep what they hold, and the drain for standing about fully overweight starts or stops with it. From the code; nothing overweight is on record.

Not built: melee, wounds, the fast weapon switch; fatigue.

## Compared with the running game

Every recording has the legs' and the arms' figures on every frame (`stamina`, `hands_stamina`), and the replay sets the simulation's beside them. Worked through on 2026-10-10 with the eleventh recording, then all the others replayed.

What the recordings show, figure by figure, for a character of Endurance 40 and Strength 40 carrying 7.5 kg (no overweight):

| | Game | By the rules above |
|---|---|---|
| The legs' capacity | 161 | 115 with Endurance's buff at level 40, 0.4 |
| The arms' capacity | 80 | 80 |
| A sprint, early in the recording | 3.9996 a second | 4 |
| A jump | 10.64 | 14 less Endurance's 24% |
| A walk fully crouched | 0.170 a second | 0.17 |
| A walk upright | nothing | nothing below the walk's overweight |
| The sights up, standing, weapon of 2.880 kg ergonomic weight | 1.568 a second off the arms | `sqrt(2.880) * 1.1`, less Strength's 16% |
| Getting up from lying | 10 off the legs and 10 off the arms, on the frame of the key | 10 and 10 |
| The stance raised from a crouch | 0.3667 at a time: the first with 0.498 of rise in the tally, then one for every 0.0553 more | a third of 1.1, at three notches of 0.166 and every third of a notch after |
| The pause after a jump that left the legs 6.6% down | 1.070 s | 1 s and 0.066 |
| Legs and arms refilling at once, standing | 6.239 and 2.912 a second: 2.1425 to 1 | 4.5 to 2.1: 2.1429 to 1 |
| The legs' refill, standing, over the eleven minutes of the recording | 5.895 a second at the start, 6.005 a minute in, 6.30 from the fifth minute | `4.5 * 1.4` is 6.30; times 0.936 for a character 97.7% fed, rising to 1 as it falls to 90% |
| A sprint, late in the same recording | 4.017, 4.034, 4.052, 4.087 a second in the sixth, seventh, eighth and eleventh minute | 4 times the thirst factor, which rises past 1 once the character is under 90% watered |

The last two rows are the character's food and water, which fall all through a raid and which no recorder up to 0.4.6 writes. The replay reads both factors off each scenario's own frames (the refill a second where a pool rises, the sprint's and aiming's cost a second), one figure of each a scenario, and keeps the last read where a scenario shows neither. It also reads off a recording where each pool stood as a scenario began, when each next began to refill, and how much rise the stance's tally already held (every charge in the scenario bounds that; the slow end of a rise bounds it to a ten-thousandth).

With that, frame by frame:

| Recording | Scenarios | Legs (of 161) | Arms (of 80) |
|---|---|---|---|
| Twelfth, with the character's food and water from the trace and nothing read off the frames but where each pool stood as a scenario began | 59 | the game's to four decimal places on every frame of every scenario | the same in 58; within 0.006 in one (below) |
| Eleventh | 59 | within 0.015 on every frame of every scenario; within 0.002 in 48 of them | the same |
| Fourth, fifth, sixth, seventh, ninth | 28, 28, 40, 49, 2 | within 0.020 | within 0.015 |
| Eighth, tenth | 56, 56 | within 0.046 (two and three scenarios past 0.02) | within 0.020 |
| Third | 19 | not comparable: up to 1.8 out | up to 0.28 out |

What is left in the eleventh is hunger and thirst moving inside a scenario where the replay has one figure of each. The third recording has no record of the player's own frame time, which the pools are worked by (8, 8 and 9 ms where the engine's frames are 8.33): standing, the replay's reconstruction of it runs 4% slow there.

**The twelfth recording** (2026-10-10, recorder 0.4.7, all 59 scenarios) is the first with the character's food and water on every frame (`energy`, `hydration`: 0.956 and 0.960 at its start, 0.713 and 0.743 at its end, stepping down about once a minute), the oxygen, the three capacities and the two skills (Endurance 40 and Strength 40, as had been worked out from the capacities and the speeds). With those taken from the trace the replay fits nothing, and the two pools came out exact in 57 scenarios at once. The other two showed two things:

- *Scenario 51* (the sights up through a roll lying down): the arms 0.005 out, growing steadily from the frame hydration stepped. Aiming's cost is a kept figure that a change of hydration does not mark stale ("Figures the game keeps"). Built so; exact since.
- *Scenario 48* (the aim key in a reload's tail): the arms 0.006 out, from the reload's end on. The game takes stock of the weapon again there and aims on at the new ergonomic weight (a magazine of 30 where the header's figure was taken with 29: 2.889 kg for 2.880). The replay holds the header's weight through a scenario. **Not corrected**: it is the replay's, and belongs with the weapon line's length, which the same moment measures again (`weapon-handling.md`).

What the comparison corrected in the first reading: stamina is worked before the animator, not at the frame's end ("A pool, each frame"); and getting up can fall on the same frame as a charge for the stance.

**The legs run dry: recorded on 2026-10-11** (the fourteenth recording, at the end of this file): a sprint ended by spent legs, the refusals at 15 and under, and what exhaustion does to the stance's speed and to going down and getting up all agree with the game frame for frame. What follows in this section is how it stood before, kept as written.

**Not recorded, from the code alone** (as of the thirteenth recording): the legs run dry (no recording takes them below 141 of 161), so a sprint ended by spent legs, the refusals at 15 and under, and what exhaustion does to the stance's speed and to going down and getting up; the cost of sprints begun in quick succession; spent arms passing the cost of aiming to the legs; anything overweight; the oxygen's own drains (the pool is on record since the twelfth recording, where nothing draws on it but a sprint); a vault's and a climb's charges. Measured in the simulation and seen in the app: a sprint held from rested legs of 115 ends by itself after 28.7 s (`a_sprint_runs_the_legs_dry_and_ends`; 29.9 s into a run of the app, where the sprint begins 1.3 s in).

**A scenario for it was written (and run on 2026-10-10: next paragraph):** `61-legs-dry` (recorder 0.4.8; 42 s; no magazine). It walks 7.5 m back from the spot, makes fifteen standing jumps a second and a half apart (a jump keeps its state for 1.39 s), which takes legs of 161 to about 10 to 13, waits for them to refill past 15, sprints forward until they are empty and the sprint ends by itself, then tries a jump on empty legs and goes down and up a stance on them. Played through the simulation for a rested character of Endurance 40 and Strength 40: the sprint begins with 20.4 in the legs, ends by itself 5.1 s on and 21.7 m out, the jump is refused, and the rise takes three times as long as rested. For a character as thirsty as the twelfth recording's was at its end the same sums give a sprint begun with 17.5 and ended after 4.3 s. If the legs are at 15 or under when the sprint key comes, the sprint is refused and the character walks instead: that is on record then, and the sprint to empty is not.

**The thirteenth recording** (2026-10-10, recorder 0.4.8, all 60 scenarios; food and water 0.91 and 0.92 at its start, 0.67 and 0.70 at its end) has `61-legs-dry`, and **the scenario did not do what it was written for.** The legs never ran dry: the lowest they went was 15.74, three quarters of a point over the threshold. The script asked for fifteen jumps a second and a half apart; in the game a jump kept its state for 1.53 s, so each press came a little earlier in the jump before it and was taken a little later, and two of the fifteen (at 14.0 and 24.5 s) were not taken at all. Thirteen jumps and the refilling between them left the legs at 54.4 when the sprint began and at 32.3 when the script let it go. The sprint did begin (the first thing to check).

What it does show: the legs are the game's within 0.02 on every frame from 2.2 s to the end, through thirteen jumps, the two presses that were lost, the sprint, a fourteenth jump with the crouch key pressed in mid-air and the rise after it (before 2.2 s the scenario's own start, a stance still settling, charges a frame apart from the game three times, 0.37 each). And it corrected the movement code: **the crouch key is taken in mid-jump** (`movement.md`, "Stance"), which the simulation had refused; with that the stance's rise on legs of 16 to 21 is charged as the game charges it.

So exhaustion is still from the code alone: the refusals at 15 and under, a sprint ended by spent legs, the slower stance and the slower going down and getting up. **A scenario that cannot miss needs the script to wait on the legs,** not on the clock: a press of the jump key every quarter of a second takes the legs down to a sawtooth between 4 and 16 whatever the jumps' lengths (a jump is taken whenever they are over 15, and refused otherwise, which is itself the refusal on record), but where in the sawtooth they stand when the presses end is not known to within three seconds, and a sprint that is to end on empty legs inside the 30 m of the recording ground has to begin with them between 15 and 22. That wants a gate in the recorder's scripts (hold the script's clock at a time until the legs are within two figures, and write the script's own time on every frame for the replay to follow); not written.

**The gate, and the scenario rewritten on it (2026-10-11; recorder 0.4.9; not yet recorded).** A script can now hold its own clock at a time until the legs are where it needs them (`gate` in a scenario file: the time, `legs_below` or `legs_from` a figure, a key to press every so many seconds while it waits, and a timeout), and every other time in the script is that clock's. The recorder writes the script's time on every frame beside the real one (`script_t`, `gate`, `gate_press`), and the replay follows those, so it needs no gate logic of its own; the same clock is in `crates/trp-sim/src/script.rs` (`Clock`) for trying a script before it is recorded.

`61-legs-dry` as it now stands (about 42 s in all, no magazine): walk 7.5 m back; **gate 1** holds the clock and presses the jump key every quarter second until the legs are under 15; 1.7 s on, with the character landed and the refill still paused, one more press of the jump key (to be refused); **gate 2** waits for the legs to refill to 16; forward, and half a second later the sprint key; **gate 3** holds the clock, keys held, until the legs are under 0.05, which is the sprint ended by spent legs; then the keys are let go, the jump key is pressed on spent legs, the crouch key twice a second apart, and the prone key twice.

Tried in the simulation (`cargo test -p trp-sim --test install the_legs_dry -- --nocapture`), for three characters: untrained and fed (legs of 115), Endurance and Strength 40 (161), and the same 67% fed and 70% watered as the thirteenth recording's was at its end. In all three the gates open on legs of 5.6 to 12.9, 16.1 and 0.00, the jump asked on legs under 15 is refused, the sprint begins on 18.5 to 19.7 and ends by itself 4.6 to 4.9 s later, 17.2 to 18.5 m ahead of the spot (20.2 to 21.5 m at the furthest, 7.8 m behind at the start), the jump on spent legs is refused, the stance comes down to 0.16 in a second and is back at 0.94 two seconds after the key, going down takes 1.36 s and getting up 1.25 s. Those last figures are the simulation's own reading of the code and are what the recording is for.

**The fourteenth recording** (2026-10-11 00:57, recorder 0.4.9, `61-legs-dry` alone: 5,459 frames, 45.8 s, on level ground; Endurance 40 and Strength 40, 98% fed and watered at its start and 96% at its end) **has it all, and the simulation agrees with it frame for frame.** What the game did:

| When | What | Legs |
|---|---|---|
| 4.5 to 27.6 s | sixteen jumps on the spot, the gate pressing the key every quarter second; each taken with more than 15 in the legs and costing 10.64 | 161 down to 4.78; the last was taken on 15.42 |
| 29.3 s | the jump key on 4.78: nothing | refilling from 28.97 |
| 31.6 s | gate 2 opens on 16.7; forward, and the sprint key half a second on | 19.6 when the sprint is taken |
| 32.2 to 37.0 s | the sprint, 4.79 s of it, until the legs are empty and it ends by itself; then the walk, and standing from 37.9 | 19.1 to 0 |
| 38 to 41 s | the jump key on spent legs: nothing; the crouch key twice: down and up, slowly | 0, then refilling |
| 41.6 s | the prone key: going down takes 1.364 s (0.9 rested) | 6.3 |
| 43.3 s | the prone key again: getting up takes 1.255 s and costs what is left of the legs and 10 of the arms | 5.9 to 0; the arms 80 to 70 |

The replay beside it, on all 5,459 frames: the movement state, the body animator's state and the state it is entering are the game's on every frame; the legs and the arms are the game's to four decimal places; the stance is the game's on every frame; the distance is within 3.5 mm and the capsule exact. So these are now recorded, each as it was read from the code: no jump and no sprint begin on 15 or less; a jump is taken on anything over 15 and may leave the legs well under it; a sprint under way ends on the frame the legs reach nothing; the stance moves as slowly as at nine tenths overweight on spent legs; going down and getting up play at 0.66 of their speed; and getting up takes what the legs have when they have less than its 10.

It took no change to the stamina code. It did show one thing wrong elsewhere, on one frame: the body's lag in height goes by the player's own clock (`body-animator.md`, "Other things the body's animation gives").

Still from the code alone: the cost of sprints begun in quick succession; spent arms passing the cost of aiming to the legs; anything overweight; the oxygen's own drains; fatigue.
