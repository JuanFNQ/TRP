//! The interface at a frame: its elements, canvases and draw order.

use super::*;

/// An element drawn on a canvas, as it stands: each field as last written (absent fields at
/// their defaults: no turn, not culled, an alpha of 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: i64,
    pub path: String,
    /// The engine's class: `Image`, `RawImage`, `TextMeshProUGUI`, ...
    pub kind: String,
    pub canvas: i64,
    /// `box`: `[left, top, right, bottom]` in the window's pixels from its top left.
    pub bounds: [f64; 4],
    /// Degrees, counter-clockwise.
    pub turn: f64,
    /// Its own `[r, g, b, a]`.
    pub color: Option<[f64; 4]>,
    /// What its renderer and the canvas groups over it multiply its colour's alpha by.
    pub alpha: f64,
    pub culled: bool,
    /// Its draw order in its canvas.
    pub depth: i64,
    /// An image's kind: `Simple`, `Sliced`, `Tiled`, `Filled`.
    pub image: Option<String>,
    /// Its sprite's or texture's name.
    pub picture: Option<String>,
    /// A filled image's share.
    pub fill: Option<f64>,
    /// A raw image's `[x, y, w, h]`.
    pub uv: Option<[f64; 4]>,
    pub words: Option<String>,
    /// The font's name and size (`font`: `[name, size]`).
    pub font: Option<String>,
    pub size: Option<f64>,
    /// Where a text's words are drawn, `[left, top, right, bottom]`.
    pub ink: Option<[f64; 4]>,
    /// The box of the rectangular mask over it.
    pub clip: Option<[f64; 4]>,
}

impl Element {
    pub fn new(id: i64) -> Element {
        Element {
            id,
            path: String::new(),
            kind: String::new(),
            canvas: 0,
            bounds: [0.0; 4],
            turn: 0.0,
            color: None,
            alpha: 1.0,
            culled: false,
            depth: 0,
            image: None,
            picture: None,
            fill: None,
            uv: None,
            words: None,
            font: None,
            size: None,
            ink: None,
            clip: None,
        }
    }

    /// Sets one field as a line writes it; `null` clears it. A key not known is passed over.
    pub fn set(&mut self, key: &str, value: &Value) {
        match key {
            "path" => self.path = words_of(value).unwrap_or_default(),
            "kind" => self.kind = words_of(value).unwrap_or_default(),
            "canvas" => self.canvas = whole(value).unwrap_or(0),
            "box" => self.bounds = four(value).unwrap_or([0.0; 4]),
            "turn" => self.turn = value.as_f64().unwrap_or(0.0),
            "color" => self.color = four(value),
            "alpha" => self.alpha = value.as_f64().unwrap_or(1.0),
            "culled" => self.culled = value.as_bool().unwrap_or(false),
            "depth" => self.depth = whole(value).unwrap_or(0),
            "image" => self.image = words_of(value),
            "picture" => self.picture = words_of(value),
            "fill" => self.fill = value.as_f64(),
            "uv" => self.uv = four(value),
            "words" => self.words = words_of(value),
            "font" => {
                let list = value.as_array();
                self.font = list.and_then(|list| list.first()).and_then(words_of);
                self.size = list.and_then(|list| list.get(1)).and_then(Value::as_f64);
            }
            "ink" => self.ink = four(value),
            "clip" => self.clip = four(value),
            _ => {}
        }
    }

    /// Sets each field of an added or changed element.
    pub fn set_all(&mut self, fields: &[(String, Value)]) {
        for (key, value) in fields {
            self.set(key, value);
        }
    }

    /// The alpha it is drawn with: its alpha times its colour's.
    pub fn shown_alpha(&self) -> f64 {
        self.alpha * self.color.map_or(1.0, |color| color[3])
    }

    pub fn width(&self) -> f64 {
        self.bounds[2] - self.bounds[0]
    }

    pub fn height(&self) -> f64 {
        self.bounds[3] - self.bounds[1]
    }

    pub fn centre(&self) -> [f64; 2] {
        [(self.bounds[0] + self.bounds[2]) * 0.5, (self.bounds[1] + self.bounds[3]) * 0.5]
    }

    /// Its words without the space around them, where it has any: white space and the
    /// characters of no width too (TextMeshPro's input field ends its text with a zero-width
    /// space, "30\u{200B}", where the app writes "30").
    pub fn trimmed_words(&self) -> Option<&str> {
        let blank = |c: char| c.is_whitespace() || matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}');
        self.words.as_deref().map(|words| words.trim_matches(blank)).filter(|words| !words.is_empty())
    }

    /// Its boxes (the box, the ink, the clip) scaled from one window's pixels to another's.
    pub fn scaled(&self, scale: [f64; 2]) -> Element {
        let by = |b: [f64; 4]| [b[0] * scale[0], b[1] * scale[1], b[2] * scale[0], b[3] * scale[1]];
        Element { bounds: by(self.bounds), ink: self.ink.map(by), clip: self.clip.map(by), ..self.clone() }
    }
}

/// The last `names` names of a path (`a/b/c/d`, 3: `b/c/d`), or the whole of a shorter one.
pub fn path_tail(path: &str, names: usize) -> &str {
    match path.rmatch_indices('/').nth(names.saturating_sub(1)) {
        Some((at, _)) if names > 0 => &path[at + 1..],
        _ => path,
    }
}

/// A canvas the interface is drawn on.
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    pub id: i64,
    pub path: String,
    /// Its root canvas's id (its own, for a root).
    pub root: i64,
    /// Its root's way of drawing: `ScreenSpaceOverlay`, `ScreenSpaceCamera`, `WorldSpace`.
    pub mode: String,
    pub order: i64,
    pub layer: String,
    pub scale: f64,
    pub on: bool,
}

impl Canvas {
    pub fn new(id: i64) -> Canvas {
        Canvas { id, path: String::new(), root: id, mode: String::new(), order: 0, layer: String::new(), scale: 1.0, on: true }
    }

    pub fn set(&mut self, key: &str, value: &Value) {
        match key {
            "path" => self.path = words_of(value).unwrap_or_default(),
            "root" => self.root = whole(value).unwrap_or(self.id),
            "mode" => self.mode = words_of(value).unwrap_or_default(),
            "order" => self.order = whole(value).unwrap_or(0),
            "layer" => self.layer = words_of(value).unwrap_or_default(),
            "scale" => self.scale = value.as_f64().unwrap_or(1.0),
            "on" => self.on = value.as_bool().unwrap_or(true),
            _ => {}
        }
    }
}

/// The interface as it stands after the lines applied so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// The frame and time of the last line applied (none before the first).
    pub frame: Option<i64>,
    pub t: f64,
    pub interface: Option<String>,
    pub screen: Option<String>,
    pub size: Option<[f64; 2]>,
    pub cursor: bool,
    pub mouse: Option<[f64; 2]>,
    pub buttons: i64,
    pub over: Option<String>,
    pub press: Option<String>,
    pub drag: Option<String>,
    pub dragging: bool,
    pub canvases: BTreeMap<i64, Canvas>,
    pub elements: BTreeMap<i64, Element>,
}

impl State {
    /// Applies one line: the screen and pointer, then the canvases, the elements added, those
    /// changed and those gone, in the order the recorder writes them. A change to an element
    /// never added makes it, with the fields given.
    pub fn apply(&mut self, line: &Line) {
        self.frame = Some(line.frame);
        self.t = line.t;
        if let Some(version) = &line.interface {
            self.interface = Some(version.clone());
        }
        if let Some(screen) = &line.screen {
            self.screen = screen.clone();
        }
        if let Some(size) = line.size {
            self.size = Some(size);
        }
        if let Some(cursor) = line.cursor {
            self.cursor = cursor;
        }
        if let Some(mouse) = line.mouse {
            self.mouse = Some(mouse);
        }
        if let Some(buttons) = line.buttons {
            self.buttons = buttons;
        }
        if let Some(over) = &line.over {
            self.over = over.clone();
        }
        if let Some(press) = &line.press {
            self.press = press.clone();
        }
        if let Some(drag) = &line.drag {
            self.drag = drag.clone();
        }
        if let Some(dragging) = line.dragging {
            self.dragging = dragging;
        }
        for change in &line.canvases {
            if change.gone {
                self.canvases.remove(&change.id);
                continue;
            }
            let canvas = self.canvases.entry(change.id).or_insert_with(|| Canvas::new(change.id));
            for (key, value) in &change.fields {
                canvas.set(key, value);
            }
        }
        for change in &line.add {
            let mut element = Element::new(change.id);
            element.set_all(&change.fields);
            self.elements.insert(change.id, element);
        }
        for change in &line.set {
            self.elements.entry(change.id).or_insert_with(|| Element::new(change.id)).set_all(&change.fields);
        }
        for id in &line.gone {
            self.elements.remove(id);
        }
    }

    /// Whether an element is drawn: its alpha times its colour's above 0, not culled, and its
    /// canvas on (a canvas never written counts as on).
    pub fn is_drawn(&self, element: &Element) -> bool {
        element.shown_alpha() > 0.0 && !element.culled && self.canvases.get(&element.canvas).is_none_or(|canvas| canvas.on)
    }

    /// The scale an element is drawn at: its root canvas's (the game's root canvas's scale
    /// factor; the app's UI scale times the window's), else its own canvas's, else 1.
    pub fn canvas_scale(&self, element: &Element) -> f64 {
        let Some(canvas) = self.canvases.get(&element.canvas) else { return 1.0 };
        self.canvases.get(&canvas.root).map_or(canvas.scale, |root| root.scale)
    }

    /// A text's font size in the window's pixels: its size as written (the game's canvas
    /// units, the app's logical units) times the scale it is drawn at.
    pub fn font_px(&self, element: &Element) -> Option<f64> {
        element.size.map(|size| size * self.canvas_scale(element))
    }

    /// The elements drawn (`all`: every element) in draw order: by their canvas's order, then
    /// by root canvas (so that two roots of one order do not interleave), then by their depth.
    pub fn in_draw_order(&self, all: bool) -> Vec<&Element> {
        let mut list: Vec<&Element> = self.elements.values().filter(|element| all || self.is_drawn(element)).collect();
        let key = |element: &Element| {
            let canvas = self.canvases.get(&element.canvas);
            (canvas.map_or(0, |c| c.order), canvas.map_or(element.canvas, |c| c.root), element.depth, element.id)
        };
        list.sort_by_key(|element| key(element));
        list
    }
}
