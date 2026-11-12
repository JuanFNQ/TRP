//! Every skill's buffs by level.

use super::*;

/// Each skill's buffs in the order its constructor gives them
/// (`SkillManager.method_2` to `method_4`, `CreateCombat`, `GetWeaponBuffs`),
/// each field's kind and visibility as declared; the figures the code has
/// from the install's settings are the install's (`skills.md`).
pub fn buffs_of(skill: Skill) -> &'static [BuffDef] {
    use BuffKind::*;
    use Rule::*;
    const WEAPON: [BuffDef; 9] = [
        BuffDef::of("WeaponReloadBuff", Simple, Level(0.004, None)),
        BuffDef::of("WeaponSwapBuff", Simple, Level(0.008, None)),
        BuffDef::of("WeaponRecoilBuff", Simple, Level(0.003, None)),
        BuffDef::of("WeaponDoubleMastering", Elite, Mastering),
        BuffDef::hidden("WeaponDoubleActionRecoilReduceBuff", Level(0.01, None)),
        BuffDef::of("WeaponErgonomicsBuff", Simple, Level(0.002, None)),
        BuffDef::of("DrawElite", Elite, Flag),
        BuffDef::hidden("MountingErgonomicsGainPerLevel", Level(0.2, None)),
        BuffDef::hidden("BipodErgonomicsGainPerLevel", Level(0.5, None)),
    ];
    match skill {
        Skill::Endurance => const { &[
            BuffDef::of("EnduranceBuffEnduranceInc", Switching, Top(0.5, Some(0.7))),
            BuffDef::of("EnduranceHands", Elite, Level(0.0, Some(0.5))),
            BuffDef::of("EnduranceBuffJumpCostRed", Simple, Top(0.3, None)),
            BuffDef::of("EnduranceBuffBreathTimeInc", Simple, Top(1.0, None)),
            BuffDef::of("EnduranceBuffRestorationTimeRed", Plebian, Top(0.5, Some(0.75))),
            BuffDef::of("EnduranceBreathElite", Elite, Level(0.0, Some(1.0))),
        ] },
        Skill::Strength => const { &[
            BuffDef::of("StrengthBuffJumpHeightInc", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffLiftWeightInc", Simple, Top(0.3, None)),
            BuffDef::of("StrengthBuffMeleePowerInc", Simple, Top(0.3, None)),
            BuffDef::of("StrengthBuffSprintSpeedInc", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffThrowDistanceInc", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffAim", Simple, Top(0.2, None)),
            BuffDef::of("StrengthBuffElite", Elite, Flag),
            BuffDef::of("StrengthBuffMeleeCrits", Elite, Level(0.0, Some(0.5))),
        ] },
        Skill::Vitality => const { &[
            BuffDef::of("VitalityBuffBleedChanceRed", Simple, Level(0.012, None)),
            BuffDef::of("VitalityBuffSurviobilityInc", Simple, Level(0.004, None)),
            BuffDef::of("VitalityBuffRegeneration", Elite, Flag),
            BuffDef::of("VitalityBuffBleedStop", Elite, Flag),
        ] },
        Skill::Health => const { &[
            BuffDef::of("HealthBreakChanceRed", Simple, Level(0.012, None)),
            BuffDef::of("HealthEnergy", Simple, Level(0.006, None)),
            BuffDef::of("HealthHydration", Simple, Level(0.006, None)),
            BuffDef::of("HealthEliteAbsorbDamage", Elite, Flag),
        ] },
        Skill::StressResistance => const { &[
            BuffDef::of("StressPainChance", Simple, Level(0.01, None)),
            BuffDef::of("StressTremor", Simple, Level(0.012, None)),
            BuffDef::of("StressBerserk", Elite, Flag),
        ] },
        Skill::Metabolism => const { &[
            BuffDef::of("MetabolismEliteBuffNoDyhydration", Elite, Flag),
            BuffDef::of("MetabolismRatioPlus", Simple, Level(0.01, None)),
            BuffDef::of("MetabolismMiscDebuffTime", Simple, Level(0.01, None)),
        ] },
        Skill::Immunity => const { &[
            BuffDef::of("ImmunityMiscEffects", Simple, Level(0.01, None)),
            BuffDef::of("ImmunityPoisonBuff", Simple, Level(0.01, None)),
            BuffDef::of("ImmunityPainKiller", Simple, Level(0.006, None)),
            BuffDef::of("ImmunityPoisonChance", Elite, Level(0.0, Some(0.9))),
            BuffDef::of("ImmunityMiscEffectsChance", Elite, Level(0.0, Some(0.9))),
        ] },
        Skill::Perception => const { &[BuffDef::of("PerceptionLootDot", Simple, Level(0.02, None)), BuffDef::of("PerceptionmEliteNoIdea", Elite, Flag)] },
        Skill::Intellect => const { &[
            BuffDef::of("IntellectEliteAmmoCounter", Elite, Flag),
            BuffDef::of("IntellectEliteContainerScope", Elite, Flag),
            BuffDef::of("IntellectEliteNaturalLearner", Elite, Flag),
            BuffDef::of("IntellectLearningSpeed", Simple, Level(0.02, None)),
            BuffDef::of("IntellectWeaponMaintance", Simple, Level(0.02, None)),
            BuffDef::of("IntellectRepairPointsCostReduction", Simple, Level(0.004, None)),
        ] },
        Skill::Attention => const { &[
            BuffDef::of("AttentionEliteExtraLootExp", Elite, Flag),
            BuffDef::of("AttentionEliteLuckySearch", Elite, Level(0.0, Some(0.5))),
            BuffDef::of("AttentionLootSpeed", Simple, Level(0.02, None)),
            BuffDef::of("AttentionRareLoot", Simple, Level(0.02, None)),
        ] },
        Skill::Charisma => const { &[
            BuffDef::of("CharismaDailyQuestsRerollDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaHealingDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaInsuranceDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaExfiltrationDiscount", Simple, Level(0.001, None)),
            BuffDef::of("CharismaScavCaseDiscount", Elite, Level(0.0, Some(0.1))),
            BuffDef::of("CharismaFenceRepPenaltyReduction", Elite, Level(0.0, Some(0.5))),
            BuffDef::of("CharismaAdditionalDailyQuests", Elite, Level(0.0, Some(1.0))),
        ] },
        Skill::Pistol | Skill::Revolver | Skill::Smg | Skill::Assault | Skill::Shotgun | Skill::Sniper | Skill::Lmg | Skill::Hmg | Skill::Launcher | Skill::AttachedLauncher | Skill::Melee | Skill::Dmr => &WEAPON,
        Skill::Throwing => const { &[
            BuffDef::of("ThrowingStrengthBuff", Simple, Level(0.005, None)),
            BuffDef::of("ThrowingEnergyExpenses", Simple, Level(0.01, None)),
            BuffDef::of("ThrowingWeaponsBuffElite", Elite, Flag),
        ] },
        Skill::AimDrills => const { &[
            BuffDef::of("DrawSpeed", Simple, Top(0.5, None)),
            BuffDef::of("DrawElite", Elite, Flag),
            BuffDef::of("DrawTremor", Elite, Flag),
            BuffDef::of("DrawSound", Simple, Top(0.5, None)),
        ] },
        Skill::TroubleShooting => const { &[
            BuffDef::of("TroubleFixing", Simple, Level(0.005, None)),
            BuffDef::of("TroubleFixingExamineMalfElite", Elite, Flag),
            BuffDef::of("TroubleFixingDurElite", Elite, Flag),
            BuffDef::of("TroubleFixingMagElite", Elite, Flag),
            BuffDef::of("TroubleFixingAmmoElite", Elite, Flag),
        ] },
        Skill::Surgery => const { &[BuffDef::of("SurgerySpeed", Switching, Top(0.2, Some(0.4))), BuffDef::of("SurgeryReducePenalty", Switching, Level(0.01, Some(1.0)))] },
        Skill::CovertMovement => const { &[
            BuffDef::of("CovertMovementSoundVolume", Simple, Level(0.012, None)),
            BuffDef::of("CovertMovementLoud", Plebian, Level(0.006, Some(0.6))),
            BuffDef::of("CovertMovementElite", Elite, Flag),
            BuffDef::of("CovertMovementSpeed", Simple, CovertSpeed),
            BuffDef::of("CovertMovementEquipment", Simple, Level(0.012, None)),
        ] },
        Skill::Search => const { &[BuffDef::of("SearchDouble", Elite, Flag), BuffDef::of("SearchSpeed", Plebian, Level(0.01, None))] },
        Skill::MagDrills => const { &[
            BuffDef::of("MagDrillsLoadSpeed", Simple, Level(0.6, None)),
            BuffDef::of("MagDrillsUnloadSpeed", Simple, Level(0.6, None)),
            BuffDef::of("MagDrillsInventoryCheckSpeed", Simple, Level(0.8, None)),
            BuffDef::of("MagDrillsInventoryCheckAccuracy", Simple, CheckAccuracy),
            BuffDef::of("MagDrillsInstantCheck", Elite, Flag),
            BuffDef::of("MagDrillsLoadProgression", Elite, Flag),
        ] },
        Skill::LightVests => const { &[
            BuffDef::of("LightVestMoveSpeedPenaltyReduction", Simple, Level(0.006, None)),
            BuffDef::of("LightVestMeleeWeaponDamageReduction", Simple, Level(0.006, None)),
            BuffDef::of("LightVestRepairDegradationReduction", Simple, Level(0.008, None)),
            BuffDef::of("LightVestBleedingProtection", Elite, Flag),
            BuffDef::of("LightVestDeteriorationChanceOnRepairReduce", Elite, Flag),
        ] },
        Skill::HeavyVests => const { &[
            BuffDef::of("HeavyVestMoveSpeedPenaltyReduction", Simple, Level(0.005, None)),
            BuffDef::of("HeavyVestBluntThroughputDamageReduction", Simple, Level(0.004, None)),
            BuffDef::of("HeavyVestRepairDegradationReduction", Simple, Level(0.01, None)),
            BuffDef::of("HeavyVestNoBodyDamageDeflectChance", Elite, Flag),
            BuffDef::of("HeavyVestDeteriorationChanceOnRepairReduce", Elite, Flag),
        ] },
        Skill::WeaponTreatment => const { &[
            BuffDef::of("WeaponDurabilityLossOnShotReduce", Simple, Level(0.005, None)),
            BuffDef::of("WearAmountRepairGunsReducePerLevel", Simple, Level(0.01, None)),
            BuffDef::of("WearChanceRepairGunsReduceEliteLevel", Elite, Flag),
        ] },
        Skill::Crafting => const { &[
            BuffDef::of("CraftingSingleTimeReduce", Simple, Level(0.0075, None)),
            BuffDef::of("CraftingContinueTimeReduce", Simple, Level(0.0075, None)),
            BuffDef::of("CraftingElite", Elite, Flag),
        ] },
        Skill::HideoutManagement => const { &[
            BuffDef::of("HideoutExtraSlots", Elite, Flag),
            BuffDef::of("HideoutZoneBonusBoost", Simple, Level(1.0, None)),
            BuffDef::of("HideoutResourceConsumption", Simple, Level(0.005, None)),
        ] },
        _ => &[],
    }
}
