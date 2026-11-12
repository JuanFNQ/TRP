//! Tests of the MASTERING page.

use super::*;

fn row(template: &str, kind: &str, level: u32, share: f32) -> Row {
    Row { template: template.to_owned(), short: template.to_owned(), name: format!("{template} rifle"), kind: kind.to_owned(), shown: Shown { level, into: 0.0, takes: 100, share, begun: true } }
}

/// The filter's lines count each type in the order first met; the
/// search finds a name whatever its case; the sort by level takes the
/// share among equals and keeps the rows' order among the rest.
#[test]
fn the_rows_are_filtered_searched_and_sorted_as_the_game_s() {
    let rows = [row("m4a1", "Assault rifle", 0, 0.5), row("adar", "Assault rifle", 0, 0.5), row("mp5", "SMG", 2, 1.0), row("akm", "Assault rifle", 1, 0.2), row("ak74", "Assault rifle", 1, 0.7)];
    let said = Words::default();
    assert_eq!(lines_of(&said, &rows, 0), ["All types (5)", "Assault rifle (4)", "SMG (1)"]);
    assert_eq!(listed(&rows, [0, 0], ""), [0, 1, 2, 3, 4]);
    assert_eq!(listed(&rows, [2, 0], ""), [2]);
    assert_eq!(listed(&rows, [0, 0], "AK"), [3, 4]);
    assert_eq!(listed(&rows, [0, 1], ""), [2, 4, 3, 0, 1]);
    assert_eq!(listed(&rows, [0, 2], ""), [0, 1, 2, 3, 4]);
}

/// A row's words: the level and one, the points cut to a whole over
/// what the level takes.
#[test]
fn a_weapon_with_no_stock_build_is_pictured_bare() {
    let Ok(install) = trp_data::Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = trp_data::Database::load(&install).expect("database");
    // The X-17 and the AK-50: the two mastering weapons the install has no build of.
    for template in ["676176d362e0497044079f4c", "67d0576f29f580ebc10efd08"] {
        assert!(db.default_preset(template).is_err(), "{template} has a stock build now: pick another");
        let (item, _) = trp_sim::items::inventory::Item::of_list(&db, &stock_or_bare(&db, template)).expect("the weapon alone");
        let cells = item.cells();
        assert!(cells[0] > 0 && cells[1] > 0, "{template}: {cells:?}");
        assert!(!crate::screens::icons::picture_key(&item).is_empty());
    }
}

#[test]
fn a_row_says_the_level_and_one() {
    let said = Words::default();
    let shown = Shown { level: 1, into: 612.9, takes: 2000, share: 0.306, begun: true };
    assert_eq!(row_words(&said, &shown), ("Level: 2".to_owned(), "<color=#c5c3b2><b>612</b></color>/2000".to_owned(), "2".to_owned()));
}
