//! Tests of the inventory, with their shared helpers.
use super::*;

mod grids;
mod weapons;
mod moves;
mod reloads;
mod rules;

use moves::{RAID, dressed, pack, stack};

fn holder(name: &str, cells: [u32; 2], takes: &[&str], refuses: &[&str]) -> Holder {
    Holder { name: name.into(), cells, takes: takes.iter().map(|s| s.to_string()).collect(), refuses: refuses.iter().map(|s| s.to_string()).collect(), ..Holder::default() }
}

/// A kind of thing of a class: the class's name as the database would
/// have it is the word with its first letter large (`weapon` is a
/// `Weapon`).
fn kind(name: &str, class: &str, weight: f32, cells: [u32; 2]) -> Kind {
    let named = format!("{}{}", class[..1].to_uppercase(), &class[1..]);
    Kind { template: name.into(), name: name.into(), classes: vec![name.into(), class.into(), "item".into()], class_names: vec![name.into(), named], weight, cells, ..Kind::default() }
}

/// Another of the same.
fn again(item: &Item) -> Item {
    Item { id: next_id(), ..item.clone() }
}

fn thing(name: &str, class: &str, weight: f32, cells: [u32; 2]) -> Item {
    Item::new(Arc::new(kind(name, class, weight, cells)))
}
