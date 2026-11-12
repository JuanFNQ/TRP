//! A trace file's lines, their fields and the changes they carry.

use super::*;

/// An element's fields in the order the recorder writes them, which is the order they are kept
/// and printed in; a field not among them comes after, by its name.
const FIELD_ORDER: &[&str] = &[
    "path", "kind", "canvas", "box", "turn", "color", "alpha", "culled", "depth", "image", "picture", "fill", "uv", "words",
    "font", "ink", "clip",
];

/// A canvas's fields in the order the recorder writes them.
const CANVAS_ORDER: &[&str] = &["path", "root", "mode", "order", "layer", "scale", "on"];

fn field_rank(order: &[&str], key: &str) -> usize {
    order.iter().position(|known| *known == key).unwrap_or(order.len())
}

/// A number, or not a number where the writer put `null` (its NaN) or anything else.
fn figure(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

/// A whole number, from an integer or a float the writer rounded.
pub(super) fn whole(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_f64().filter(|v| v.is_finite()).map(|v| v.round() as i64))
}

pub(super) fn words_of(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// Four numbers (a box, a colour, a uv), or none where the value is not a list of four.
pub(super) fn four(value: &Value) -> Option<[f64; 4]> {
    let list = value.as_array().filter(|list| list.len() == 4)?;
    Some([figure(&list[0]), figure(&list[1]), figure(&list[2]), figure(&list[3])])
}

fn two(value: &Value) -> Option<[f64; 2]> {
    let list = value.as_array().filter(|list| list.len() == 2)?;
    Some([figure(&list[0]), figure(&list[1])])
}

/// The fields of an added or changed element, or of a canvas, without its id (and a canvas's
/// `gone`), in the writer's `order`.
fn fields_of(object: &Map<String, Value>, order: &[&str]) -> Vec<(String, Value)> {
    let mut fields: Vec<(String, Value)> = object
        .iter()
        .filter(|(key, _)| key.as_str() != "id" && key.as_str() != "gone")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    fields.sort_by(|a, b| field_rank(order, &a.0).cmp(&field_rank(order, &b.0)).then_with(|| a.0.cmp(&b.0)));
    fields
}

/// An element added (all of its fields) or changed (the changed fields alone; a field set to
/// `null` is gone).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub id: i64,
    pub fields: Vec<(String, Value)>,
}

impl Change {
    /// The value of one of its fields, if it holds it.
    pub fn field(&self, key: &str) -> Option<&Value> {
        self.fields.iter().find(|(name, _)| name == key).map(|(_, value)| value)
    }
}

/// A canvas seen or changed (its fields, which the recorder writes whole), or gone.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasChange {
    pub id: i64,
    pub gone: bool,
    pub fields: Vec<(String, Value)>,
}

/// One line of the file: what changed on one frame. Each `Option` is `None` where the line
/// does not hold the key; `Some(None)` is a key written `null` (no screen, nothing under the
/// pointer).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    /// Its number in the file, from 1.
    pub number: usize,
    pub frame: i64,
    /// The trace's time, in seconds (NaN where the line has none).
    pub t: f64,
    /// The writer's version, on the first line.
    pub interface: Option<String>,
    pub screen: Option<Option<String>>,
    /// The window's `[width, height]` in pixels.
    pub size: Option<[f64; 2]>,
    pub cursor: Option<bool>,
    /// `[x, y]`, pixels from the screen's top left.
    pub mouse: Option<[f64; 2]>,
    /// Left 1, right 2, middle 4.
    pub buttons: Option<i64>,
    pub over: Option<Option<String>>,
    pub press: Option<Option<String>>,
    pub drag: Option<Option<String>>,
    pub dragging: Option<bool>,
    pub canvases: Vec<CanvasChange>,
    pub add: Vec<Change>,
    pub set: Vec<Change>,
    pub gone: Vec<i64>,
    /// Each sound as written (`["ui", type]`, `["item", group, type]`, `["clip", name, single,
    /// common, volume]`), its parts as text: a string as it is, anything else as JSON.
    pub sounds: Vec<Vec<String>>,
}

/// The entries of a list of elements (`add`, `set`); an entry without an id is passed over.
fn changes(value: &Value) -> Vec<Change> {
    let Some(list) = value.as_array() else { return Vec::new() };
    list.iter()
        .filter_map(|entry| {
            let object = entry.as_object()?;
            let id = object.get("id").and_then(whole)?;
            Some(Change { id, fields: fields_of(object, FIELD_ORDER) })
        })
        .collect()
}

fn canvas_changes(value: &Value) -> Vec<CanvasChange> {
    let Some(list) = value.as_array() else { return Vec::new() };
    list.iter()
        .filter_map(|entry| {
            let object = entry.as_object()?;
            let id = object.get("id").and_then(whole)?;
            let gone = object.get("gone").and_then(Value::as_bool).unwrap_or(false);
            Some(CanvasChange { id, gone, fields: fields_of(object, CANVAS_ORDER) })
        })
        .collect()
}

pub(super) fn sounds(value: &Value) -> Vec<Vec<String>> {
    let part = |value: &Value| value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string());
    let Some(list) = value.as_array() else { return Vec::new() };
    list.iter()
        .map(|sound| match sound.as_array() {
            Some(parts) => parts.iter().map(part).collect(),
            None => vec![part(sound)],
        })
        .collect()
}

impl Line {
    /// One line of the file (`number` from 1). An error says why it could not be read: not
    /// JSON, not an object, or no frame.
    pub fn parse(number: usize, text: &str) -> std::result::Result<Line, String> {
        // serde_json's message ends "at line 1 column N", the line being the text given: the
        // column alone is kept, the file's line being `number`.
        let value: Value = serde_json::from_str(text).map_err(|error| {
            let message = error.to_string();
            match message.rfind(" at line ") {
                Some(at) => format!("{} at column {}", &message[..at], error.column()),
                None => message,
            }
        })?;
        let Value::Object(object) = value else { return Err("not a JSON object".into()) };
        let frame = object.get("frame").and_then(whole).ok_or("no frame")?;
        let t = object.get("t").map_or(f64::NAN, figure);
        let mut line = Line { number, frame, t, ..Line::default() };
        for (key, value) in &object {
            match key.as_str() {
                "interface" => line.interface = words_of(value).or_else(|| Some(value.to_string())),
                "screen" => line.screen = Some(words_of(value)),
                "size" => line.size = two(value),
                "cursor" => line.cursor = value.as_bool(),
                "mouse" => line.mouse = two(value),
                "buttons" => line.buttons = whole(value),
                "over" => line.over = Some(words_of(value)),
                "press" => line.press = Some(words_of(value)),
                "drag" => line.drag = Some(words_of(value)),
                "dragging" => line.dragging = value.as_bool(),
                "canvases" => line.canvases = canvas_changes(value),
                "add" => line.add = changes(value),
                "set" => line.set = changes(value),
                "gone" => line.gone = value.as_array().map(|list| list.iter().filter_map(whole).collect()).unwrap_or_default(),
                "sounds" => line.sounds = sounds(value),
                _ => {}
            }
        }
        Ok(line)
    }
}
