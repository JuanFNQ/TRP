//! Tests of the interface trace: reading and replaying its lines and pairing two traces' elements.

use super::*;

/// Three frames: a text and an image added on a canvas, the image's picture cleared and its
/// alpha set to 0 with the pointer over it, the text gone with two sounds; a bad line and
/// an unknown key among them.
const SMALL: &str = r#"{"frame":1,"t":0.0167,"interface":"0.16.0","screen":"BattleUIScreen","size":[1920,1080],"cursor":false,"mouse":[960,540],"buttons":0,"canvases":[{"id":10,"path":"Root/Canvas","root":10,"mode":"ScreenSpaceOverlay","order":5,"layer":"Default","scale":1,"on":true}],"add":[{"id":100,"path":"Root/Canvas/Panel/Label","kind":"TextMeshProUGUI","canvas":10,"box":[10,20,110,40],"color":[1,1,1,1],"alpha":1,"depth":3,"words":"HELLO","font":["Bender",14],"ink":[12,22,60,38]},{"id":101,"path":"Root/Canvas/Panel/Icon","kind":"Image","canvas":10,"box":[0,0,64,64],"color":[1,1,1,0.5],"alpha":1,"depth":2,"image":"Simple","picture":"icon_a"}]}
not json at all
{"frame":3,"t":0.05,"set":[{"id":101,"picture":null,"alpha":0,"novel":7}],"over":"Root/Canvas/Panel/Icon","screen":"Inventory","whatever":{"x":1}}

{"frame":5,"t":0.0833,"gone":[100],"over":null,"sounds":[["ui","Click"],["clip","click_01",false,true,1]]}
"#;

#[test]
fn ui_trace_reads_lines_and_reports_a_bad_one() {
    let trace = Trace::parse(SMALL);
    assert_eq!(trace.lines.len(), 3);
    assert_eq!(trace.bad.len(), 1);
    assert_eq!(trace.bad[0].0, 2);
    assert_eq!(trace.lines[1].number, 3);
    assert_eq!(trace.lines[0].interface.as_deref(), Some("0.16.0"));
    assert_eq!(trace.lines[0].add.len(), 2);
    // The fields kept in the writer's order, whatever serde_json's map order is.
    let keys: Vec<&str> = trace.lines[0].add[0].fields.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["path", "kind", "canvas", "box", "color", "alpha", "depth", "words", "font", "ink"]);
    assert_eq!(trace.lines[2].over, Some(None));
    assert_eq!(trace.lines[2].sounds, vec![vec!["ui".to_string(), "Click".into()], vec!["clip".into(), "click_01".into(), "false".into(), "true".into(), "1".into()]]);
    assert_eq!(trace.first_frame(), Some(1));
    assert_eq!(trace.last_frame(), Some(5));
}

#[test]
fn ui_trace_replays_adds_sets_and_gone() {
    let trace = Trace::parse(SMALL);
    let before = trace.state_at(0);
    assert_eq!(before.frame, None);
    assert!(before.elements.is_empty());

    let first = trace.state_at(2);
    assert_eq!(first.frame, Some(1));
    assert_eq!(first.screen.as_deref(), Some("BattleUIScreen"));
    assert_eq!(first.size, Some([1920.0, 1080.0]));
    assert_eq!(first.mouse, Some([960.0, 540.0]));
    let label = &first.elements[&100];
    assert_eq!(label.words.as_deref(), Some("HELLO"));
    assert_eq!(label.font.as_deref(), Some("Bender"));
    assert_eq!(label.size, Some(14.0));
    assert_eq!(label.bounds, [10.0, 20.0, 110.0, 40.0]);
    assert_eq!(label.turn, 0.0);
    assert!(!label.culled);
    let icon = &first.elements[&101];
    assert_eq!(icon.picture.as_deref(), Some("icon_a"));
    assert_eq!(icon.shown_alpha(), 0.5);
    // Drawn in draw order: the icon (depth 2) before the label (depth 3).
    let drawn: Vec<i64> = first.in_draw_order(false).iter().map(|e| e.id).collect();
    assert_eq!(drawn, [101, 100]);

    let second = trace.state_at(3);
    let icon = &second.elements[&101];
    assert_eq!(icon.picture, None, "a field set to null is gone");
    assert_eq!(icon.alpha, 0.0);
    assert_eq!(icon.image.as_deref(), Some("Simple"), "a field not set stays");
    assert_eq!(second.over.as_deref(), Some("Root/Canvas/Panel/Icon"));
    assert_eq!(second.screen.as_deref(), Some("Inventory"));
    let drawn: Vec<i64> = second.in_draw_order(false).iter().map(|e| e.id).collect();
    assert_eq!(drawn, [100]);
    assert_eq!(second.in_draw_order(true).len(), 2);

    let last = trace.state_at(99);
    assert_eq!(last.frame, Some(5));
    assert!(!last.elements.contains_key(&100));
    assert_eq!(last.elements.len(), 1);
    assert_eq!(last.over, None);
}

#[test]
fn ui_trace_finds_frames_by_time_screens_sounds_and_follows() {
    let trace = Trace::parse(SMALL);
    assert_eq!(trace.frame_at_t(0.06), Some(3));
    assert_eq!(trace.frame_at_t(0.05), Some(3));
    assert_eq!(trace.frame_at_t(0.0), None);
    let screens = trace.screens();
    assert_eq!(screens.len(), 2);
    assert_eq!(screens[0], Shown { screen: Some("BattleUIScreen".into()), spans: vec![(1, 2)] });
    assert_eq!(screens[1], Shown { screen: Some("Inventory".into()), spans: vec![(3, 5)] });
    let sounds = trace.sounds();
    assert_eq!(sounds.len(), 2);
    assert_eq!(sounds[0].0, 5);
    let most = trace.most();
    assert_eq!((most.elements, most.elements_frame, most.drawn, most.drawn_frame), (2, 1, 2, 1));

    let followed = trace.follow("icon");
    assert_eq!(followed.len(), 1);
    assert_eq!(followed[0].id, 101);
    assert_eq!(followed[0].kind, "Image");
    assert_eq!(followed[0].events.len(), 2);
    assert!(matches!(followed[0].events[0].happened, Happened::Added(_)));
    let Happened::Changed(fields) = &followed[0].events[1].happened else { panic!("a change") };
    assert_eq!(fields.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), ["alpha", "picture", "novel"]);
    let followed = trace.follow("hello");
    assert_eq!(followed.len(), 1);
    assert_eq!(followed[0].events.last().map(|e| &e.happened), Some(&Happened::Gone));
}

#[test]
fn ui_trace_leaves_out_culled_elements_and_those_on_a_canvas_off() {
    let text = r#"{"frame":0,"t":0,"canvases":[{"id":1,"order":0,"on":true},{"id":2,"order":1,"on":false}],"add":[{"id":5,"kind":"Image","canvas":1,"box":[0,0,1,1],"alpha":1,"culled":true},{"id":6,"kind":"Image","canvas":2,"box":[0,0,1,1],"alpha":1},{"id":7,"kind":"Image","canvas":3,"box":[0,0,1,1],"alpha":1}]}
{"frame":1,"t":0.1,"canvases":[{"id":2,"order":1,"on":true}],"set":[{"id":5,"culled":false}]}
{"frame":2,"t":0.2,"canvases":[{"id":2,"gone":true}]}"#;
    let trace = Trace::parse(text);
    let ids = |state: &State| state.in_draw_order(false).iter().map(|e| e.id).collect::<Vec<_>>();
    assert_eq!(ids(&trace.state_at(0)), [7]);
    assert_eq!(ids(&trace.state_at(1)), [5, 7, 6]);
    let last = trace.state_at(2);
    assert!(!last.canvases.contains_key(&2));
    assert_eq!(ids(&last), [5, 6, 7], "with its canvas gone, the element counts as on a canvas of order 0, on");
}

fn text_at(id: i64, words: &str, bounds: [f64; 4]) -> Element {
    Element { kind: "TextMeshProUGUI".into(), words: Some(words.into()), bounds, ..Element::new(id) }
}

fn image_at(id: i64, picture: &str, bounds: [f64; 4]) -> Element {
    Element { kind: "Image".into(), picture: Some(picture.into()), bounds, ..Element::new(id) }
}

#[test]
fn ui_trace_pairs_repeated_words_by_the_nearest_centre() {
    let this = [text_at(1, "OK", [0.0, 0.0, 20.0, 10.0]), text_at(2, "OK", [100.0, 0.0, 120.0, 10.0]), text_at(3, " CLOSE ", [0.0, 50.0, 40.0, 60.0])];
    let other = [text_at(11, "OK", [103.0, 1.0, 123.0, 11.0]), text_at(12, "CLOSE", [300.0, 300.0, 340.0, 310.0]), text_at(13, "OK", [2.0, 0.0, 22.0, 10.0])];
    let pairs = pair(&this, &other);
    assert_eq!(pairs, [(0, 2, Paired::Words), (1, 0, Paired::Words), (2, 1, Paired::Words)], "words pair however far apart, trimmed");
}

#[test]
fn ui_trace_pairs_a_field_s_words_without_the_zero_width_space() {
    // `courses10`'s `how-many`: the game's field reads "30" and a zero-width space.
    let this = [text_at(1, "30\u{200B}", [1290.0, 607.0, 1365.0, 640.0])];
    let other = [text_at(11, "30", [1290.0, 607.0, 1365.0, 640.0])];
    assert_eq!(this[0].trimmed_words(), Some("30"));
    assert_eq!(pair(&this, &other), [(0, 0, Paired::Words)]);
    assert_eq!(text_at(2, "\u{200B}", [0.0; 4]).trimmed_words(), None, "nothing but a zero-width space is no words");
}

#[test]
fn ui_trace_pairs_the_rest_by_overlap() {
    let this = [
        image_at(1, "a", [0.0, 0.0, 100.0, 100.0]),
        image_at(2, "b", [200.0, 0.0, 300.0, 100.0]),
        text_at(3, "HERE", [400.0, 0.0, 500.0, 20.0]),
    ];
    let other = [
        // Over the first, 8,100 of 10,000 square pixels shared (0.81): paired.
        image_at(11, "a2", [10.0, 10.0, 100.0, 100.0]),
        // Over the second, 5,000 of 15,000 (a third): not paired.
        image_at(12, "b", [250.0, 0.0, 350.0, 100.0]),
        // An image over the third, a text: paired by overlap, kinds ignored.
        image_at(13, "c", [400.0, 0.0, 500.0, 22.0]),
    ];
    let pairs = pair(&this, &other);
    assert_eq!(pairs, [(0, 0, Paired::Overlap), (2, 2, Paired::Overlap)]);
    assert!((overlap(&this[0].bounds, &other[0].bounds) - 0.81).abs() < 1e-9);
    assert_eq!(overlap(&[5.0, 5.0, 5.0, 5.0], &[5.0, 5.0, 5.0, 5.0]), 1.0);
    assert_eq!(overlap(&[0.0, 0.0, 1.0, 1.0], &[2.0, 2.0, 3.0, 3.0]), 0.0);
}

#[test]
fn ui_trace_beside_scales_the_other_window() {
    let game = Trace::parse(
        r#"{"frame":10,"t":1,"size":[1920,1080],"add":[{"id":1,"kind":"TextMeshProUGUI","canvas":0,"box":[100,100,300,140],"alpha":1,"words":"INVENTORY","ink":[110,105,290,135]},{"id":2,"kind":"Image","canvas":0,"box":[0,0,960,540],"alpha":1,"picture":"back"},{"id":3,"kind":"Image","canvas":0,"box":[1800,1000,1900,1060],"alpha":1,"picture":"lone"}]}"#,
    );
    let app = Trace::parse(
        r#"{"frame":2,"t":0.1,"size":[960,540],"add":[{"id":7,"kind":"Text","canvas":0,"box":[50,50,150,70],"alpha":1,"words":"INVENTORY","ink":[55,52,145,68]},{"id":8,"kind":"Image","canvas":0,"box":[0,0,481,270],"alpha":1,"picture":"background"},{"id":9,"kind":"Image","canvas":0,"box":[0,500,10,510],"alpha":0}]}"#,
    );
    let sides = beside(&game.state_at(10), &app.state_at(2));
    assert_eq!(sides.scale, [2.0, 2.0]);
    assert_eq!(sides.other.len(), 2, "the hidden one is not drawn");
    let text = sides.other.iter().find(|e| e.id == 7).unwrap();
    assert_eq!(text.bounds, [100.0, 100.0, 300.0, 140.0]);
    assert_eq!(text.ink, Some([110.0, 104.0, 290.0, 136.0]));
    let mut pairs: Vec<(i64, i64, Paired)> = sides.pairs.iter().map(|&(i, j, by)| (sides.this[i].id, sides.other[j].id, by)).collect();
    pairs.sort();
    assert_eq!(pairs, [(1, 7, Paired::Words), (2, 8, Paired::Overlap)]);
    assert_eq!(sides.alone_this.iter().map(|&i| sides.this[i].id).collect::<Vec<_>>(), [3]);
    assert!(sides.alone_other.is_empty());

    let same = beside(&game.state_at(10), &game.state_at(10));
    assert_eq!(same.scale, [1.0, 1.0]);
    assert_eq!(same.pairs.len(), 3);
}

#[test]
fn ui_trace_font_sizes_in_window_pixels() {
    // The game: a root canvas of scale 1.5 and a canvas under it whose own scale is not
    // the one it is drawn at; a canvas with no root written; a text on no canvas known.
    let game = Trace::parse(
        r#"{"frame":1,"t":0,"size":[1920,1080],"canvases":[{"id":10,"root":10,"scale":1.5},{"id":11,"root":10,"scale":1},{"id":12,"root":99,"scale":2}],"add":[{"id":1,"kind":"TextMeshProUGUI","canvas":11,"box":[0,0,100,30],"alpha":1,"words":"EXAMINE","font":["Bender",14]},{"id":2,"kind":"TextMeshProUGUI","canvas":12,"box":[0,100,100,130],"alpha":1,"words":"OWN","font":["Bender",10]},{"id":3,"kind":"TextMeshProUGUI","canvas":50,"box":[0,200,100,230],"alpha":1,"words":"NONE","font":["Bender",12]},{"id":4,"kind":"Image","canvas":10,"box":[0,300,10,310],"alpha":1}]}"#,
    );
    let state = game.state_at(1);
    let px = |id: i64| state.font_px(&state.elements[&id]);
    assert_eq!(px(1), Some(21.0), "the root canvas's scale");
    assert_eq!(px(2), Some(20.0), "its own canvas's, the root not written");
    assert_eq!(px(3), Some(12.0), "1, no canvas written");
    assert_eq!(px(4), None, "not a text");

    // The app at half the window, its one canvas at a UI scale of 0.75: 14 x 0.75 x 2 = 21.
    let app = Trace::parse(
        r#"{"frame":1,"t":0,"size":[960,540],"canvases":[{"id":1,"root":1,"scale":0.75}],"add":[{"id":5,"kind":"Text","canvas":1,"box":[0,0,50,15],"alpha":1,"words":"EXAMINE","font":["Bender",14]}]}"#,
    );
    let sides = beside(&state, &app.state_at(1));
    let (i, j, _) = sides.pairs.iter().copied().find(|&(i, _, by)| by == Paired::Words && sides.this[i].id == 1).unwrap();
    assert_eq!(sides.other[j].id, 5);
    assert_eq!(sides.this_font_px[i], Some(21.0));
    assert_eq!(sides.other_font_px[j], Some(21.0));
}

#[test]
fn ui_trace_path_tail_keeps_the_last_names() {
    assert_eq!(path_tail("a/b/c/d", 3), "b/c/d");
    assert_eq!(path_tail("c/d", 3), "c/d");
    assert_eq!(path_tail("d", 3), "d");
    assert_eq!(path_tail("a/b", 1), "b");
}
