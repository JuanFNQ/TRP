//! Numbers and strings written as the recorder writes them.

use super::*;

pub(super) fn r2(v: f32) -> f32 {
    ((f64::from(v) * 100.0).round() / 100.0) as f32
}

pub(super) fn r4(v: f32) -> f32 {
    ((f64::from(v) * 10000.0).round() / 10000.0) as f32
}

/// A number to `places` decimals, without the noughts at its end (`0.##`).
fn number(v: f32, places: usize) -> String {
    if !v.is_finite() {
        return "null".to_owned();
    }
    let mut s = format!("{:.*}", places, f64::from(v));
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_owned();
    }
    s
}

/// Pixels.
pub(super) fn p(v: f32) -> String {
    number(v, 2)
}

/// Colours, alphas, seconds and shares.
pub(super) fn f(v: f32) -> String {
    number(v, 4)
}

pub(super) fn rect(r: [f32; 4]) -> String {
    format!("[{},{},{},{}]", p(r[0]), p(r[1]), p(r[2]), p(r[3]))
}

pub(super) fn color(c: [f32; 4]) -> String {
    format!("[{},{},{},{}]", f(c[0]), f(c[1]), f(c[2]), f(c[3]))
}

/// A colour as authored (the screen's numbers), rounded as written.
pub(super) fn rgba(color: Color) -> [f32; 4] {
    let c = color.to_srgba();
    [r4(c.red), r4(c.green), r4(c.blue), r4(c.alpha)]
}

pub(super) fn font_of(font: &(Option<String>, f32)) -> String {
    format!("[{},{}]", font.0.as_deref().map_or("null".to_owned(), text), p(font.1))
}

/// A JSON string.
pub(super) fn text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A sound as the recorder writes it: `["ui", type]`, `["item", group,
/// way]`, `["clip", name, single, common, volume]`.
pub(super) fn told(sound: &Told) -> String {
    match sound {
        Told::Ui(kind) => format!("[\"ui\",{}]", text(kind)),
        Told::Item(group, way) => format!("[\"item\",{},{}]", text(group), text(way)),
        Told::Clip(name, common) => format!("[\"clip\",{},false,{common},1]", text(name)),
    }
}
