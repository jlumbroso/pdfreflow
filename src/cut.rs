//! The cut: rewrite the page list so that each crop is its own page.
//!
//! A crop is the source page seen through a narrower page box. The drawing
//! instructions, fonts and images are shared, not copied or changed, so text
//! stays text and the file barely grows (ADR-0002).

use lopdf::{Dictionary, Document, Object, ObjectId, Result, Stream};

use crate::geom::Rect;
use crate::ink::numbers;
use crate::layout::Piece;

/// US Letter, the PDF default when a page declares no size at all.
const DEFAULT_PAGE: Rect = Rect {
    x0: 0.0,
    y0: 0.0,
    x1: 612.0,
    y1: 792.0,
};

/// A page property, looked up on the page and then on its ancestors in the
/// page tree, which is where PDF lets pages inherit it from.
fn inherited(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page_id;
    for _ in 0..64 {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(value) = dict.get(key) {
            return Some(value.clone());
        }
        id = dict.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}

fn rect_of(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<Rect> {
    let values = numbers(doc, &inherited(doc, page_id, key)?)?;
    match *values {
        [xa, ya, xb, yb] => Some(Rect::new(xa, ya, xb, yb)),
        _ => None,
    }
}

/// The visible rectangle of a page.
pub fn page_box(doc: &Document, page_id: ObjectId) -> Rect {
    let media = rect_of(doc, page_id, b"MediaBox").unwrap_or(DEFAULT_PAGE);
    rect_of(doc, page_id, b"CropBox")
        .and_then(|crop| crop.intersect(media))
        .unwrap_or(media)
}

/// The page's stored rotation in degrees (0 for an upright page).
pub fn page_rotation(doc: &Document, page_id: ObjectId) -> i64 {
    inherited(doc, page_id, b"Rotate")
        .and_then(|rotate| {
            doc.dereference(&rotate)
                .ok()
                .and_then(|(_, o)| o.as_i64().ok())
        })
        .unwrap_or(0)
}

fn rect_array(rect: &Rect) -> Object {
    let round = |v: f64| Object::Real(((v * 100.0).round() / 100.0) as f32);
    Object::Array(vec![
        round(rect.x0),
        round(rect.y0),
        round(rect.x1),
        round(rect.y1),
    ])
}

/// Replaces the document's page list according to `plans` (one per source
/// page, in order) and returns the new page count.
pub fn apply(doc: &mut Document, page_ids: &[ObjectId], plans: &[Vec<Piece>]) -> Result<usize> {
    let root = doc.catalog()?.get(b"Pages")?.as_reference()?;
    let mut kids: Vec<ObjectId> = Vec::new();
    // The instruction that closes a shrunk page's wrapper, shared by all of them.
    let mut restore: Option<ObjectId> = None;

    for (&page_id, pieces) in page_ids.iter().zip(plans) {
        // The new page list is flat, so anything the page used to inherit from
        // an intermediate node must be written onto the page itself.
        let mut page: Dictionary = doc.get_dictionary(page_id)?.clone();
        let inheritable: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];
        for key in inheritable {
            if !page.has(key)
                && let Some(value) = inherited(doc, page_id, key)
            {
                page.set(key, value);
            }
        }
        page.set("Parent", Object::Reference(root));

        // These describe the whole page and no longer hold for a piece of it.
        // Links (`Annots`) are dropped for now: see ADR-0002.
        let stale: [&[u8]; 5] = [b"TrimBox", b"BleedBox", b"ArtBox", b"Annots", b"B"];
        for key in stale {
            page.remove(key);
        }

        for (index, piece) in pieces.iter().enumerate() {
            let mut dict = page.clone();
            let shown = if piece.scale == 1.0 {
                // A column: the same drawing, seen through a narrower window.
                piece.rect
            } else {
                // Wider than a column: the same drawing again, but wrapped in
                // one instruction that shrinks it and moves the piece's corner
                // to the origin. Still nothing is rendered.
                let scale = piece.scale;
                let shrink = format!(
                    "q {scale:.5} 0 0 {scale:.5} {:.3} {:.3} cm\n",
                    -piece.rect.x0 * scale,
                    -piece.rect.y0 * scale
                );
                let open = doc.add_object(Stream::new(Dictionary::new(), shrink.into_bytes()));
                let close = match restore {
                    Some(id) => id,
                    None => {
                        let id = doc.add_object(Stream::new(Dictionary::new(), b"\nQ".to_vec()));
                        restore = Some(id);
                        id
                    }
                };
                let mut contents = vec![Object::Reference(open)];
                match dict.get(b"Contents") {
                    Ok(Object::Array(streams)) => contents.extend(streams.iter().cloned()),
                    Ok(Object::Reference(id)) => match doc.get_object(*id) {
                        Ok(Object::Array(streams)) => contents.extend(streams.iter().cloned()),
                        _ => contents.push(Object::Reference(*id)),
                    },
                    _ => {}
                }
                contents.push(Object::Reference(close));
                dict.set("Contents", Object::Array(contents));
                Rect::new(
                    0.0,
                    0.0,
                    piece.rect.width() * scale,
                    piece.rect.height() * scale,
                )
            };
            dict.set("MediaBox", rect_array(&shown));
            dict.set("CropBox", rect_array(&shown));
            if piece.turned {
                // A quarter turn that puts the piece's top on the left.
                dict.set("Rotate", Object::Integer(270));
            }
            if index == 0 {
                // The first piece keeps the source page's identity, so
                // bookmarks into the page still land on it.
                doc.set_object(page_id, dict);
                kids.push(page_id);
            } else {
                kids.push(doc.add_object(dict));
            }
        }
    }

    let count = kids.len();
    let pages = doc.get_dictionary_mut(root)?;
    pages.set(
        "Kids",
        Object::Array(kids.into_iter().map(Object::Reference).collect()),
    );
    pages.set("Count", Object::Integer(count as i64));
    Ok(count)
}
