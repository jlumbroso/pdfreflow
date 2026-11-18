//! Where is the ink? Walks a page's drawing instructions and reports the
//! bounding box of every run of text, every image and every painted path,
//! in the page's own coordinate system. Nothing is rendered.
//!
//! This is a deliberately small interpreter: it tracks only what is needed to
//! know *where* things land (the transformation matrix and the text state),
//! and nothing about what they look like.

use std::collections::HashMap;
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::geom::{Matrix, Rect};

/// How deep to follow drawings nested inside drawings (Form XObjects).
const MAX_FORM_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkKind {
    Text,
    Image,
    Path,
}

#[derive(Clone, Copy, Debug)]
pub struct InkBox {
    pub rect: Rect,
    pub kind: InkKind,
}

/// Every piece of ink on one page.
pub fn page_ink(doc: &Document, page_id: ObjectId) -> Vec<InkBox> {
    let mut resources: Vec<&Dictionary> = Vec::new();
    if let Ok((direct, ids)) = doc.get_page_resources(page_id) {
        resources.extend(direct);
        resources.extend(ids.into_iter().filter_map(|id| doc.get_dictionary(id).ok()));
    }
    let content = doc.get_page_content(page_id);
    let mut interp = Interp {
        doc,
        out: Vec::new(),
        fonts: HashMap::new(),
    };
    interp.run(&content, &resources, Matrix::IDENTITY, 0);
    interp.out
}

pub(crate) fn num(object: &Object) -> Option<f64> {
    match object {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

fn deref<'a>(doc: &'a Document, object: &'a Object) -> &'a Object {
    doc.dereference(object).map(|(_, o)| o).unwrap_or(object)
}

/// An array of numbers, following references to the array and to its elements.
pub(crate) fn numbers(doc: &Document, object: &Object) -> Option<Vec<f64>> {
    deref(doc, object)
        .as_array()
        .ok()?
        .iter()
        .map(|o| num(deref(doc, o)))
        .collect()
}

fn matrix(values: &[f64]) -> Option<Matrix> {
    match *values {
        [a, b, c, d, e, f] => Some(Matrix { a, b, c, d, e, f }),
        _ => None,
    }
}

fn operand_numbers(operands: &[Object]) -> Option<Vec<f64>> {
    operands.iter().map(num).collect()
}

/// Glyph widths of a font, in glyph units (normally 1/1000 of the font size).
enum Widths {
    /// One byte per character.
    Simple {
        first: i64,
        widths: Vec<f64>,
        missing: f64,
    },
    /// Two bytes per character (composite fonts).
    Cid {
        map: HashMap<u32, f64>,
        default: f64,
    },
}

struct FontMetrics {
    widths: Widths,
    /// Glyph units to text units: 0.001 except for Type 3 fonts.
    scale: f64,
    /// Height above and depth below the baseline, as fractions of the font size.
    ascent: f64,
    descent: f64,
}

impl FontMetrics {
    /// Used when a font cannot be found: an average glyph is half an em wide.
    fn fallback() -> FontMetrics {
        FontMetrics {
            widths: Widths::Simple {
                first: 0,
                widths: Vec::new(),
                missing: 500.0,
            },
            scale: 0.001,
            ascent: 0.75,
            descent: -0.22,
        }
    }

    fn from_dict(doc: &Document, font: &Dictionary) -> FontMetrics {
        let get = |dict: &Dictionary, key: &[u8]| dict.get_deref(key, doc).ok().and_then(num);
        let mut metrics = FontMetrics::fallback();
        let subtype = font
            .get(b"Subtype")
            .and_then(Object::as_name)
            .unwrap_or(b"");

        let descriptor_owner = if subtype == b"Type0" {
            let descendant = font
                .get_deref(b"DescendantFonts", doc)
                .and_then(Object::as_array)
                .ok()
                .and_then(|fonts| fonts.first())
                .and_then(|o| deref(doc, o).as_dict().ok());
            let mut map = HashMap::new();
            let mut default = 1000.0;
            if let Some(descendant) = descendant {
                default = get(descendant, b"DW").unwrap_or(1000.0);
                if let Ok(w) = descendant.get_deref(b"W", doc).and_then(Object::as_array) {
                    read_cid_widths(doc, w, &mut map);
                }
            }
            metrics.widths = Widths::Cid { map, default };
            descendant
        } else {
            let widths = font
                .get(b"Widths")
                .ok()
                .and_then(|w| numbers(doc, w))
                .unwrap_or_default();
            let missing = if widths.is_empty() {
                // One of the 14 standard fonts, which may omit their widths.
                let base = font
                    .get(b"BaseFont")
                    .and_then(Object::as_name)
                    .unwrap_or(b"");
                if base.starts_with(b"Courier") {
                    600.0
                } else {
                    500.0
                }
            } else {
                font.get_deref(b"FontDescriptor", doc)
                    .and_then(Object::as_dict)
                    .ok()
                    .and_then(|d| get(d, b"MissingWidth"))
                    .unwrap_or(0.0)
            };
            metrics.widths = Widths::Simple {
                first: get(font, b"FirstChar").unwrap_or(0.0) as i64,
                widths,
                missing,
            };
            if subtype == b"Type3" {
                let font_matrix = font.get(b"FontMatrix").ok().and_then(|m| numbers(doc, m));
                if let Some(m) = font_matrix.as_deref().and_then(matrix) {
                    metrics.scale = m.a;
                    // A Type 3 font states its height through its bounding box.
                    if let Some(bbox) = font.get(b"FontBBox").ok().and_then(|b| numbers(doc, b))
                        && bbox.len() == 4
                        && bbox[3] > bbox[1]
                    {
                        metrics.ascent = bbox[3] * m.d;
                        metrics.descent = bbox[1] * m.d;
                    }
                }
            }
            Some(font)
        };

        let descriptor = descriptor_owner
            .and_then(|owner| owner.get_deref(b"FontDescriptor", doc).ok())
            .and_then(|d| d.as_dict().ok());
        if let Some(descriptor) = descriptor {
            // Font files sometimes carry nonsense here; only believe plausible values.
            if let Some(ascent) =
                get(descriptor, b"Ascent").filter(|a| (500.0..=1000.0).contains(a))
            {
                metrics.ascent = ascent / 1000.0;
            }
            if let Some(descent) =
                get(descriptor, b"Descent").filter(|d| (-400.0..=0.0).contains(d))
            {
                metrics.descent = descent / 1000.0;
            }
        }
        metrics
    }

    fn two_byte(&self) -> bool {
        matches!(self.widths, Widths::Cid { .. })
    }

    fn width(&self, code: u32) -> f64 {
        match &self.widths {
            Widths::Simple {
                first,
                widths,
                missing,
            } => usize::try_from(i64::from(code) - first)
                .ok()
                .and_then(|index| widths.get(index))
                .copied()
                .unwrap_or(*missing),
            Widths::Cid { map, default } => map.get(&code).copied().unwrap_or(*default),
        }
    }
}

/// Reads a composite font's `/W` array: `c [w1 w2 …]` or `c_first c_last w`.
fn read_cid_widths(doc: &Document, w: &[Object], map: &mut HashMap<u32, f64>) {
    let mut i = 0;
    while i + 1 < w.len() {
        let Some(first) = num(deref(doc, &w[i])) else {
            return;
        };
        let first = first as u32;
        match deref(doc, &w[i + 1]) {
            Object::Array(list) => {
                for (offset, width) in list.iter().enumerate() {
                    if let Some(width) = num(deref(doc, width)) {
                        map.insert(first.saturating_add(offset as u32), width);
                    }
                }
                i += 2;
            }
            last => {
                let (Some(last), Some(width)) =
                    (num(last), w.get(i + 2).and_then(|o| num(deref(doc, o))))
                else {
                    return;
                };
                for code in first..=(last as u32).min(first.saturating_add(0xFFFF)) {
                    map.insert(code, width);
                }
                i += 3;
            }
        }
    }
}

/// The part of the graphics state that `q` saves and `Q` restores.
#[derive(Clone)]
struct GState {
    ctm: Matrix,
    font: Option<Rc<FontMetrics>>,
    size: f64,
    char_spacing: f64,
    word_spacing: f64,
    horizontal_scale: f64,
    leading: f64,
    rise: f64,
}

struct Interp<'a> {
    doc: &'a Document,
    out: Vec<InkBox>,
    fonts: HashMap<ObjectId, Rc<FontMetrics>>,
}

impl<'a> Interp<'a> {
    fn run(&mut self, data: &[u8], resources: &[&'a Dictionary], ctm: Matrix, depth: usize) {
        let Ok(content) = Content::decode(data) else {
            return;
        };
        let mut gs = GState {
            ctm,
            font: None,
            size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal_scale: 1.0,
            leading: 0.0,
            rise: 0.0,
        };
        let mut saved: Vec<GState> = Vec::new();
        let mut text_matrix = Matrix::IDENTITY;
        let mut line_matrix = Matrix::IDENTITY;
        let mut path: Option<Rect> = None;

        for operation in &content.operations {
            let operands = operation.operands.as_slice();
            let nums = operand_numbers(operands);
            let nums = nums.as_deref();
            match operation.operator.as_str() {
                "q" => saved.push(gs.clone()),
                "Q" => {
                    if let Some(state) = saved.pop() {
                        gs = state;
                    }
                }
                "cm" => {
                    if let Some(m) = nums.and_then(matrix) {
                        gs.ctm = m.then(gs.ctm);
                    }
                }

                // Text state.
                "BT" => {
                    text_matrix = Matrix::IDENTITY;
                    line_matrix = Matrix::IDENTITY;
                }
                "Tf" => {
                    if let [name, size] = operands {
                        gs.font = name
                            .as_name()
                            .ok()
                            .and_then(|name| self.font(resources, name));
                        gs.size = num(size).unwrap_or(0.0);
                    }
                }
                "Tc" => gs.char_spacing = first(nums).unwrap_or(0.0),
                "Tw" => gs.word_spacing = first(nums).unwrap_or(0.0),
                "Tz" => gs.horizontal_scale = first(nums).unwrap_or(100.0) / 100.0,
                "TL" => gs.leading = first(nums).unwrap_or(0.0),
                "Ts" => gs.rise = first(nums).unwrap_or(0.0),

                // Text positioning.
                "Td" | "TD" => {
                    if let Some(&[tx, ty]) = nums {
                        if operation.operator == "TD" {
                            gs.leading = -ty;
                        }
                        line_matrix = Matrix::translate(tx, ty).then(line_matrix);
                        text_matrix = line_matrix;
                    }
                }
                "Tm" => {
                    if let Some(m) = nums.and_then(matrix) {
                        text_matrix = m;
                        line_matrix = m;
                    }
                }
                "T*" => {
                    line_matrix = Matrix::translate(0.0, -gs.leading).then(line_matrix);
                    text_matrix = line_matrix;
                }

                // Text showing.
                "Tj" => {
                    if let Some(Object::String(bytes, _)) = operands.first() {
                        self.show(&gs, &mut text_matrix, bytes);
                    }
                }
                "'" | "\"" => {
                    if operation.operator == "\""
                        && let [word, char, _] = operands
                    {
                        gs.word_spacing = num(word).unwrap_or(0.0);
                        gs.char_spacing = num(char).unwrap_or(0.0);
                    }
                    line_matrix = Matrix::translate(0.0, -gs.leading).then(line_matrix);
                    text_matrix = line_matrix;
                    if let Some(Object::String(bytes, _)) = operands.last() {
                        self.show(&gs, &mut text_matrix, bytes);
                    }
                }
                "TJ" => {
                    if let Some(Object::Array(items)) = operands.first() {
                        for item in items {
                            match item {
                                Object::String(bytes, _) => self.show(&gs, &mut text_matrix, bytes),
                                other => {
                                    if let Some(adjust) = num(other) {
                                        let shift =
                                            -adjust / 1000.0 * gs.size * gs.horizontal_scale;
                                        text_matrix =
                                            Matrix::translate(shift, 0.0).then(text_matrix);
                                    }
                                }
                            }
                        }
                    }
                }

                // Path construction: remember the bounding box, in page space.
                "m" | "l" | "c" | "v" | "y" => {
                    if let Some(nums) = nums {
                        for pair in nums.chunks_exact(2) {
                            let (x, y) = gs.ctm.apply(pair[0], pair[1]);
                            extend(&mut path, Rect::new(x, y, x, y));
                        }
                    }
                }
                "re" => {
                    if let Some(&[x, y, w, h]) = nums
                        && let Some(rect) = gs.ctm.apply_rect(x, y, x + w, y + h)
                    {
                        extend(&mut path, rect);
                    }
                }
                // Painting a path leaves ink; ending it without painting (`n`,
                // used for clipping) does not.
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    if let Some(rect) = path.take() {
                        self.push(rect, InkKind::Path);
                    }
                }
                "n" => path = None,

                // Images and nested drawings.
                "BI" => {
                    if let Some(rect) = gs.ctm.apply_rect(0.0, 0.0, 1.0, 1.0) {
                        self.push(rect, InkKind::Image);
                    }
                }
                "Do" => {
                    if let Some(name) = operands.first().and_then(|n| n.as_name().ok()) {
                        self.draw_xobject(resources, name, &gs, depth);
                    }
                }
                _ => {}
            }
        }
    }

    fn push(&mut self, rect: Rect, kind: InkKind) {
        if rect.is_finite() {
            self.out.push(InkBox { rect, kind });
        }
    }

    fn font(&mut self, resources: &[&'a Dictionary], name: &[u8]) -> Option<Rc<FontMetrics>> {
        let doc = self.doc;
        for dict in resources {
            let Ok(fonts) = dict.get_deref(b"Font", doc).and_then(Object::as_dict) else {
                continue;
            };
            let Ok(entry) = fonts.get(name) else { continue };
            return match entry {
                Object::Reference(id) => {
                    if let Some(metrics) = self.fonts.get(id) {
                        return Some(metrics.clone());
                    }
                    let metrics =
                        Rc::new(FontMetrics::from_dict(doc, doc.get_dictionary(*id).ok()?));
                    self.fonts.insert(*id, metrics.clone());
                    Some(metrics)
                }
                Object::Dictionary(font) => Some(Rc::new(FontMetrics::from_dict(doc, font))),
                _ => None,
            };
        }
        None
    }

    /// Records the box of one string of text and advances the text position past it.
    fn show(&mut self, gs: &GState, text_matrix: &mut Matrix, bytes: &[u8]) {
        let fallback;
        let font = match &gs.font {
            Some(font) => font.as_ref(),
            None => {
                fallback = FontMetrics::fallback();
                &fallback
            }
        };
        let two_byte = font.two_byte();
        let mut x = 0.0;
        // Spaces move the pen but leave no ink, so they do not widen the box.
        let mut ink: Option<(f64, f64)> = None;
        let mut glyph = |code: u32| {
            let width = font.width(code) * font.scale * gs.size;
            let is_space = !two_byte && code == 32;
            if !is_space {
                ink = Some((ink.map_or(x, |(start, _)| start), x + width));
            }
            x += width + gs.char_spacing + if is_space { gs.word_spacing } else { 0.0 };
        };
        if two_byte {
            for pair in bytes.chunks(2) {
                glyph(
                    pair.iter()
                        .fold(0, |code, &byte| (code << 8) | u32::from(byte)),
                );
            }
        } else {
            for &byte in bytes {
                glyph(u32::from(byte));
            }
        }

        if let Some((start, end)) = ink {
            let text_to_page = Matrix {
                a: gs.horizontal_scale,
                f: gs.rise,
                ..Matrix::IDENTITY
            }
            .then(*text_matrix)
            .then(gs.ctm);
            let rect =
                text_to_page.apply_rect(start, font.descent * gs.size, end, font.ascent * gs.size);
            if let Some(rect) = rect {
                self.push(rect, InkKind::Text);
            }
        }
        *text_matrix = Matrix::translate(x * gs.horizontal_scale, 0.0).then(*text_matrix);
    }

    fn draw_xobject(
        &mut self,
        resources: &[&'a Dictionary],
        name: &[u8],
        gs: &GState,
        depth: usize,
    ) {
        let doc = self.doc;
        let stream = resources.iter().find_map(|dict| {
            let xobjects = dict
                .get_deref(b"XObject", doc)
                .and_then(Object::as_dict)
                .ok()?;
            xobjects
                .get_deref(name, doc)
                .and_then(Object::as_stream)
                .ok()
        });
        let Some(stream) = stream else { return };
        match stream.dict.get(b"Subtype").and_then(Object::as_name) {
            Ok(b"Image") => {
                if let Some(rect) = gs.ctm.apply_rect(0.0, 0.0, 1.0, 1.0) {
                    self.push(rect, InkKind::Image);
                }
            }
            Ok(b"Form") if depth < MAX_FORM_DEPTH => {
                let form_matrix = stream
                    .dict
                    .get(b"Matrix")
                    .ok()
                    .and_then(|m| numbers(doc, m))
                    .as_deref()
                    .and_then(matrix)
                    .unwrap_or(Matrix::IDENTITY);
                let ctm = form_matrix.then(gs.ctm);
                // A form is clipped to its bounding box: ink outside it is not seen.
                let clip = stream
                    .dict
                    .get(b"BBox")
                    .ok()
                    .and_then(|b| numbers(doc, b))
                    .filter(|b| b.len() == 4)
                    .and_then(|b| ctm.apply_rect(b[0], b[1], b[2], b[3]));
                let own_resources = stream
                    .dict
                    .get_deref(b"Resources", doc)
                    .and_then(Object::as_dict);
                let form_resources: Vec<&'a Dictionary> = match own_resources {
                    Ok(dict) => vec![dict],
                    Err(_) => resources.to_vec(),
                };
                let Ok(data) = stream.get_plain_content() else {
                    return;
                };
                let start = self.out.len();
                self.run(&data, &form_resources, ctm, depth + 1);
                if let Some(clip) = clip {
                    let inside: Vec<InkBox> = self
                        .out
                        .drain(start..)
                        .filter_map(|ink| {
                            ink.rect.intersect(clip).map(|rect| InkBox {
                                rect,
                                kind: ink.kind,
                            })
                        })
                        .collect();
                    self.out.extend(inside);
                }
            }
            _ => {}
        }
    }
}

fn first(nums: Option<&[f64]>) -> Option<f64> {
    nums.and_then(|n| n.first().copied())
}

fn extend(path: &mut Option<Rect>, rect: Rect) {
    *path = Some(path.map_or(rect, |p| p.union(rect)));
}
