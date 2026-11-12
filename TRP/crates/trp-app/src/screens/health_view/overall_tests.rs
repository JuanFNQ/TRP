//! Tests of the OVERALL tab's texts.

use super::{format_values, rich_runs, rich_spans};

/// A localized text's holes filled as `string.Format` fills them: the
/// online time's `{0}h{1:00}m` with none of either.
#[test]
fn a_format_s_holes_are_filled() {
    assert_eq!(format_values("{0}h{1:00}m", &["0", "0"]), "0h00m");
    assert_eq!(format_values("R: {0}", &["12"]), "R: 12");
    assert_eq!(format_values("{1:00}", &["0", "123"]), "123");
    assert_eq!(format_values("{3} left", &["0"]), "{3} left");
}

/// A short statistic with no figure: its hole and its unit give way to
/// the `-`, the colour tag kept.
#[test]
fn a_figure_not_had_takes_its_unit_with_it() {
    use super::without_figure;
    assert_eq!(without_figure("S/R: <color=#C3C5BBFF>{0}%</color>", "-"), "S/R: <color=#C3C5BBFF>-</color>");
    assert_eq!(without_figure("Online: <color=#C3C5BBFF>{0}h</color>", "-"), "Online: <color=#C3C5BBFF>-</color>");
    assert_eq!(without_figure("R: {0}", "-"), "R: -");
    assert_eq!(without_figure("Raids", "-"), "Raids");
}

/// TextMeshPro's colour tags as runs of their colours, the others left
/// out; words without one as they are.
#[test]
fn colour_tags_are_runs() {
    assert_eq!(rich_spans("Raids"), None);
    let spans = rich_spans("R: <color=#C5C3B2>0</color>").expect("tagged");
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0], ("R: ".to_owned(), None));
    assert_eq!(spans[1].0, "0");
    let colour = spans[1].1.expect("its colour");
    assert!((colour[0] - 0xC5 as f32 / 255.0).abs() < 1e-6 && colour[3] == 1.0);
    let spans = rich_spans("<b>a</b><color=#00000080>b</color>c").expect("tagged");
    assert_eq!(spans.iter().map(|(words, _)| words.as_str()).collect::<String>(), "abc");
    assert!((spans[1].1.expect("b's")[3] - 128.0 / 255.0).abs() < 1e-6);
    assert_eq!(spans[2].1, None);
    // A colour by its name (the repair window's `<color=white>`).
    let spans = rich_spans("60/100<color=white> (100)</color>").expect("tagged");
    assert_eq!(spans[1], (" (100)".to_owned(), Some([1.0, 1.0, 1.0, 1.0])));
    // The runs keep the bold apart (an armour plate's tooltip:
    // `ArmorPlatesFormatter`), and a tag of no colour or bold as words.
    let runs = rich_runs("<sprite name=\"armor_classes_4\"> <b>Front plate:</b> <color=red>x</color>");
    assert_eq!(runs.len(), 4, "{runs:?}");
    assert_eq!(runs[0], ("<sprite name=\"armor_classes_4\"> ".to_owned(), None, false));
    assert_eq!(runs[1], ("Front plate:".to_owned(), None, true));
    assert_eq!(runs[2], (" ".to_owned(), None, false));
    assert_eq!(runs[3], ("x".to_owned(), Some([1.0, 0.0, 0.0, 1.0]), false));
}
