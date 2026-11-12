//! The controls generated and chosen, and read from the install.

use super::*;

impl Controls {
    /// The generated bindings made afresh of those they are made of, after
    /// all the rest, as the game's list of bindings has them (`GENERATED`);
    /// none for one whose binding is missing.
    pub fn generate(&mut self) {
        self.keys.retain(|group| !group.own_keys() && !group.reserved());
        // The game's reserved bindings come after the player's
        // (`ControlSettingsGroup.ReservedKeys`): each function key let go
        // (`F1`) and pressed twice (`DoubleF1`), what the gestures' binds
        // are said by. (Enter and Escape, also reserved, are heard here
        // apart, by their keys.)
        for n in 1..=12u16 {
            let key = Source::of(&[FUNCTION_KEYS + n - 1]);
            self.keys.push(KeyGroup { name: format!("F{n}"), variants: [key.clone(), Source::default()], press: PressType::Release });
            self.keys.push(KeyGroup { name: format!("DoubleF{n}"), variants: [key, Source::default()], press: PressType::DoubleClick });
        }
        for (name, of) in GENERATED {
            if let Some(source) = self.keys.iter().find(|group| group.name == of) {
                let made = KeyGroup { name: name.to_owned(), variants: source.variants.clone(), press: PressType::Continuous };
                self.keys.push(made);
            }
        }
    }

    pub fn group(&self, name: &str) -> Option<usize> {
        self.keys.iter().position(|group| group.name == name)
    }

    /// The keys chosen for a variant of a binding are in
    /// (`CommandKeyPair.ListenForKey`, after the choice). Says whether the
    /// game now asks how the binding is to be pressed: it does where
    /// another binding has the same keys and is pressed the same way, or
    /// where the binding had no keys before, if its way may be chosen and
    /// its other variant is empty. Until that is answered (`press_chosen`)
    /// the other binding keeps its keys.
    pub fn key_chosen(&mut self, group: usize, variant: usize, source: Source) -> bool {
        let was_empty = self.keys[group].is_empty();
        self.keys[group].variants[variant] = source.clone();
        let free = self.settle(group, &source, false);
        let asks = (!free || was_empty) && self.keys[group].press_shown() && self.keys[group].variants[1 - variant].is_empty();
        if asks {
            if was_empty {
                self.keys[group].press = PressType::ALL[0];
            }
        } else {
            if self.keys[group].variants.iter().any(|variant| variant.wheel.is_some()) {
                // A binding with a notch of the wheel is a press.
                self.keys[group].press = PressType::Press;
            }
            self.settle(group, &source, true);
        }
        // The same keys in its other variant are dropped there.
        if !source.is_empty() && self.keys[group].variants[1 - variant] == source {
            self.keys[group].variants[1 - variant] = Source::default();
        }
        self.fix_press(group);
        asks
    }

    /// How a binding is pressed has been chosen (`DropdownClosed`): any
    /// other binding with the same keys and now the same way loses them.
    pub fn press_chosen(&mut self, group: usize, press: PressType) {
        self.keys[group].press = press;
        for variant in self.keys[group].variants.clone() {
            self.settle(group, &variant, true);
        }
        self.fix_press(group);
    }

    /// The keys chosen for one way of an axis are in
    /// (`ControlSettingsTab.AxisChosen`): the same keys are dropped
    /// wherever else they stand, on an axis or on a binding.
    pub fn axis_chosen(&mut self, axis: usize, pair: usize, way: usize, source: Source) {
        self.axes[axis].pairs[pair][way] = source.clone();
        if source.is_empty() {
            return;
        }
        for (a, group) in self.axes.iter_mut().enumerate() {
            for (p, sources) in group.pairs.iter_mut().enumerate() {
                for (w, other) in sources.iter_mut().enumerate() {
                    if (a, p, w) != (axis, pair, way) && *other == source {
                        *other = Source::default();
                    }
                }
            }
        }
        for group in self.keys.iter_mut().filter(|group| !group.hidden()) {
            for other in group.variants.iter_mut().filter(|other| **other == source) {
                *other = Source::default();
            }
        }
    }

    /// A binding that has one way of being pressed is pressed that way.
    fn fix_press(&mut self, group: usize) {
        if let Some((_, press)) = FIXED_PRESS.iter().find(|(name, _)| *name == self.keys[group].name) {
            self.keys[group].press = *press;
        }
    }

    /// Looks for the same keys on the other bindings shown
    /// (`ControlSettingsTab.KeyChosen`). Two bindings whose way of being
    /// pressed may be chosen share keys as long as they are pressed
    /// differently. Pressed alike, the other loses its keys when `full`;
    /// before that the answer is that the keys are not free, and the game
    /// asks for another way. Where either's way may not be chosen, the
    /// other loses its keys at once. When `full`, the axes lose them too.
    fn settle(&mut self, group: usize, source: &Source, full: bool) -> bool {
        if source.is_empty() {
            return true;
        }
        let (choice, press) = (self.keys[group].press_shown(), self.keys[group].press);
        let mut free = true;
        for other in 0..self.keys.len() {
            if other == group || self.keys[other].hidden() {
                continue;
            }
            let Some(at) = self.keys[other].variants.iter().position(|variant| variant == source) else { continue };
            let both = choice && self.keys[other].press_shown();
            if !both || press == self.keys[other].press {
                free = !both;
                if full || !both {
                    self.keys[other].variants[at] = Source::default();
                }
            }
        }
        if full {
            for axis in &mut self.axes {
                for other in axis.pairs.iter_mut().flatten().filter(|other| **other == *source) {
                    *other = Source::default();
                }
            }
        }
        free
    }
}

/// One variant of a binding as the install keeps it, for the manager: its
/// keys, and a notch of the wheel if it ends in one. Empty if it names a
/// key there is no number for here, or an axis other than the wheel.
pub fn of_install(source: &trp_data::setup::install::KeySource) -> Source {
    let wheel = match &source.axis {
        Some((axis, up)) if axis == "Mouse Scroll" => Some(*up),
        Some(_) => return Source::default(),
        None => None,
    };
    match source.keys.iter().map(|name| code(name)).collect::<Option<Vec<Code>>>() {
        Some(keys) => Source { keys, wheel },
        None => Source::default(),
    }
}

/// The install's own bindings (`Control.ini`): every key binding it has,
/// in its order, and those of its axes that keys push (the mouse's four
/// are the view's). Nothing where the install has no such file.
pub fn install_controls(install: &trp_data::Install) -> Controls {
    let keys = install
        .key_bindings()
        .into_iter()
        .map(|binding| {
            let variant = |at: usize| binding.variants.get(at).map(of_install).unwrap_or_default();
            KeyGroup { variants: [variant(0), variant(1)], press: PressType::of(&binding.press).unwrap_or(PressType::Press), name: binding.name }
        })
        .collect();
    let axes = install
        .axis_bindings()
        .into_iter()
        .filter(|axis| axis.pairs.iter().flatten().all(|way| way.axis.is_none()))
        .map(|axis| {
            let pair = |at: usize| axis.pairs.get(at).map(|[up, down]| [of_install(up), of_install(down)]).unwrap_or_default();
            AxisGroup { pairs: [pair(0), pair(1)], name: axis.name }
        })
        .collect();
    let mut controls = Controls { keys, axes, double_click: install.control_setting("DoubleClickTimeout").unwrap_or(0.3).clamp(0.3, 1.0) };
    controls.generate();
    controls
}
