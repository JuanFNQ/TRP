//! Fitting an underbarrel launcher, reading its state and toggling to it.

use super::*;

impl Firearm {
    /// Fits the build's launcher, its chamber loaded or not. A weapon taken in hand
    /// afterwards tells its animator it has one.
    pub fn fit_launcher(&mut self, data: &LauncherData, loaded: bool) {
        self.launcher = Some(Box::new(Launcher {
            animator: WeaponAnimator::new(&data.animator),
            nodes: data.nodes.clone(),
            caseless: data.caseless,
            bones: data.bones,
            round: loaded,
            case: false,
            round_drawn: loaded,
            case_drawn: false,
            marked: false,
            speaks: false,
            step: 0,
            range: RANGES[0],
            shown_range: RANGES[0],
            recoil_due: false,
            then: None,
            marks_seen: Vec::new(),
        }));
    }

    /// The launcher's chamber as its item holds it, told at a draw: a live round
    /// or not, its model put in its chamber with it (`UnderbarrelContainer.
    /// InitShellsAndAmmo`, as the hands are made); the spent case goes with what
    /// the item keeps of it (none here: the inventory keeps no case,
    /// `PROVISIONAL`). The range's step, the mark and the rest are the hands' own.
    pub fn set_launcher_round(&mut self, loaded: bool) {
        if let Some(launcher) = self.launcher.as_mut() {
            launcher.round = loaded;
            launcher.round_drawn = loaded;
            launcher.animator.set("AmmoInChamber", loaded as u8 as f32);
        }
    }

    /// Whether the weapon has a launcher.
    pub fn has_launcher(&self) -> bool {
        self.launcher.is_some()
    }

    /// Whether the launcher is in use (`FirearmController.IsInLauncherMode`: its rest,
    /// its looks, its shot or its reload; not the switch, nor a round moved from the
    /// inventory).
    pub fn in_launcher_mode(&self) -> bool {
        matches!(self.operation, Operation::LauncherIdle | Operation::LauncherUtility { .. } | Operation::LauncherFire { .. } | Operation::LauncherReload { .. })
    }

    /// Whether the launcher's animator is the hands' now.
    pub fn launcher_speaks(&self) -> bool {
        self.launcher.as_ref().is_some_and(|launcher| launcher.speaks)
    }

    /// The launcher's chamber: a live round, a spent case.
    pub fn launcher_chamber(&self) -> Option<(bool, bool)> {
        self.launcher.as_ref().map(|launcher| (launcher.round, launcher.case))
    }

    /// What its model shows (underbarrel-launcher spec, "Reloading and unloading"):
    /// the round's model in its chamber (`patron_in_weapon`), and the spent case's
    /// at its shell port (`shellport`). The round's goes in at the reload's or the
    /// load's `MagShow` (though the reload's round is in the chamber from its start),
    /// or as the hands are made with one; it leaves at the shot (to the port as the case, for a round
    /// that keeps one) and at the discharge's `DelAmmoChamber`. The case leaves at a
    /// `ShellEject` of the reload or a look (thrown: `LauncherEvent::CaseThrown`) and
    /// at the load's `RemoveShell` (gone).
    pub fn launcher_models(&self) -> Option<(bool, bool)> {
        self.launcher.as_ref().map(|launcher| (launcher.round_drawn, launcher.case_drawn))
    }

    /// The range set on the launcher's sight, in metres.
    pub fn launcher_range(&self) -> Option<i32> {
        self.launcher.as_ref().map(|launcher| launcher.range)
    }

    /// The launcher's animator.
    pub fn launcher_animator(&self) -> Option<&WeaponAnimator> {
        self.launcher.as_ref().map(|launcher| &launcher.animator)
    }

    /// The weapon's bones the launcher's animator poses, in its pose's order.
    pub fn launcher_nodes(&self) -> Option<&[usize]> {
        self.launcher.as_ref().map(|launcher| launcher.nodes.as_slice())
    }

    /// The marks the launcher's animator delivered last frame.
    pub fn launcher_marks(&self) -> &[Mark] {
        self.launcher.as_ref().map_or(&[], |launcher| &launcher.marks_seen)
    }

    /// The launcher's bones in the build.
    pub fn launcher_bones(&self) -> Option<LauncherBones> {
        self.launcher.as_ref().map(|launcher| launcher.bones)
    }

    /// Whether the weapon's motion and the arms go by the launcher's bones now
    /// (`IsGrenadeLauncher`, `UpdateLauncherBones`): from a switch's start going in
    /// to its end coming back, the time its animator is the hands'. Its sight is
    /// then the only one, the rifle is carried by its prop bone, and the left elbow
    /// bends toward its goal.
    pub fn launcher_bones_in_use(&self) -> bool {
        self.launcher_speaks()
    }

    /// Writes the launcher's pose over its bones in a pose of the whole rig (its
    /// animator's, or the one it draws), where the weapon has one. Its bones keep
    /// the last pose it gave them while the rifle is in use: its animator is then
    /// off, and the engine leaves an animator's bones as it last put them.
    pub fn put_launcher_pose(&self, pose: &mut [([f32; 3], [f32; 4])], drawn: bool) {
        let Some(launcher) = &self.launcher else { return };
        let put = |own: &[([f32; 3], [f32; 4])], pose: &mut [([f32; 3], [f32; 4])]| {
            for (&node, placed) in launcher.nodes.iter().zip(own) {
                if let Some(bone) = pose.get_mut(node) {
                    *bone = *placed;
                }
            }
        };
        if drawn {
            put(&launcher.animator.drawn_pose(), pose);
        } else {
            put(launcher.animator.pose(), pose);
        }
    }

    /// The weapon's slot key pressed again with the weapon in hand
    /// (`FirearmHandsInputTranslator.ChangeWeaponMode`, `ToggleLauncher`): to the
    /// launcher from the rifle's rest or its looks, back from the launcher's rest or
    /// its looks; dropped by everything else. Whether a switch began.
    pub fn toggle_launcher(&mut self) -> bool {
        if self.launcher.is_none() {
            return false;
        }
        match self.operation {
            Operation::Idle | Operation::Utility { .. } => self.begin_switch(true, None),
            Operation::LauncherIdle | Operation::LauncherUtility { .. } => self.begin_switch(false, None),
            _ => return false,
        }
        true
    }
}
