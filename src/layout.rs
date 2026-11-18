//! From ink to crops: find the two columns and the text block for the whole
//! document, then decide what to cut out of each page.
//!
//! The columns are found once, for the whole document, from what the typical
//! page looks like (ADR-0002). A title that crosses the gap on page 1 is ink on
//! one page only, and a page number sitting in the gap is one short line next
//! to columns that are sixty lines tall: neither changes the typical page, so
//! the gap is still seen and the cut goes straight through that noise.

use std::fmt;

use crate::geom::Rect;
use crate::ink::{InkBox, InkKind};

/// White space kept around each crop, in points.
const PAD: f64 = 4.0;
/// A place counts as "empty" when it has less than this fraction of the ink
/// that a typical place inside a column has.
const LOW: f64 = 0.25;
/// The narrowest gap between columns that is believed, in points.
const MIN_GUTTER: f64 = 4.0;
/// The gap must be centred within this middle fraction of the text width.
const CENTRAL: (f64, f64) = (0.3, 0.7);
/// Rows of ink separated by less than this much white (on every page) are one block.
const BLOCK_GAP: usize = 5;
/// A page whose gap holds at least this fraction of the ink density of its
/// columns is a single-column page and is not cut in two.
const SPANNING: f64 = 0.5;

/// The ink of one page, with the page's visible rectangle.
pub struct PageInk {
    pub media: Rect,
    /// False for pages stored rotated; those are passed through untouched.
    pub upright: bool,
    pub boxes: Vec<InkBox>,
}

/// A range along one axis, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub lo: f64,
    pub hi: f64,
}

impl Span {
    pub fn width(&self) -> f64 {
        self.hi - self.lo
    }

    fn overlap(&self, lo: f64, hi: f64) -> f64 {
        (hi.min(self.hi) - lo.max(self.lo)).max(0.0)
    }
}

/// The document's two-column geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Horizontal extent of the left column.
    pub left: Span,
    /// The empty strip between the columns.
    pub gutter: Span,
    /// Horizontal extent of the right column.
    pub right: Span,
    /// Vertical extent of the text block, running headers and footers excluded.
    pub type_area: Span,
}

/// What to do with one source page.
#[derive(Clone, Debug, PartialEq)]
pub enum PagePlan {
    /// Leave the page exactly as it is.
    Keep,
    /// Replace the page by these crops, in reading order.
    Crops(Vec<Rect>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// No text was found on any page (a scanned document, for instance).
    NoText,
    /// Text was found, but no empty strip down the middle of it.
    NotTwoColumn,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::NoText => write!(f, "no text found (scanned pages are not supported yet)"),
            LayoutError::NotTwoColumn => write!(f, "no gap between two columns found"),
        }
    }
}

/// Ink summed along one axis, in bins one point wide.
struct Profile {
    origin: f64,
    bins: Vec<f64>,
}

impl Profile {
    fn new(lo: f64, hi: f64) -> Profile {
        Profile {
            origin: lo.floor(),
            bins: vec![0.0; (hi.ceil() - lo.floor()).max(1.0) as usize],
        }
    }

    /// Adds `weight` per point over `lo..hi`.
    fn add(&mut self, lo: f64, hi: f64, weight: f64) {
        let first = ((lo - self.origin).floor().max(0.0)) as usize;
        for index in first..self.bins.len() {
            let bin_lo = self.origin + index as f64;
            if bin_lo >= hi {
                break;
            }
            let covered = hi.min(bin_lo + 1.0) - lo.max(bin_lo);
            if covered > 0.0 {
                self.bins[index] += weight * covered;
            }
        }
    }

    fn position(&self, index: usize) -> f64 {
        self.origin + index as f64
    }
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(values[values.len() / 2])
}

/// A box that covers nearly the whole page is a background, not content.
fn is_background(ink: &InkBox, media: Rect) -> bool {
    ink.kind != InkKind::Text
        && ink.rect.width() >= 0.9 * media.width()
        && ink.rect.height() >= 0.9 * media.height()
}

/// Finds the two columns and the text block from all pages together.
pub fn detect(pages: &[PageInk]) -> Result<Layout, LayoutError> {
    let usable: Vec<&PageInk> = pages
        .iter()
        .filter(|page| page.upright && !page.boxes.is_empty())
        .collect();
    let bounds = usable
        .iter()
        .map(|page| page.media)
        .reduce(Rect::union)
        .ok_or(LayoutError::NoText)?;

    // Horizontal profile of the text on the typical page: for each x, how much
    // text height sits above it, taking the median over all pages. A median,
    // not a sum, so that a minority of unusual pages (a title crossing the gap,
    // single-column appendix pages) cannot fill the gap in.
    let per_page: Vec<Profile> = usable
        .iter()
        .map(|page| {
            let mut profile = Profile::new(bounds.x0, bounds.x1);
            for ink in &page.boxes {
                if ink.kind == InkKind::Text {
                    profile.add(ink.rect.x0, ink.rect.x1, ink.rect.height());
                }
            }
            profile
        })
        .collect();
    let mut across = Profile::new(bounds.x0, bounds.x1);
    for (index, bin) in across.bins.iter_mut().enumerate() {
        let mut values: Vec<f64> = per_page.iter().map(|page| page.bins[index]).collect();
        *bin = median(&mut values).unwrap_or(0.0);
    }
    let peak = across.bins.iter().copied().fold(0.0, f64::max);
    let mut inside: Vec<f64> = across
        .bins
        .iter()
        .copied()
        .filter(|&v| v > 0.2 * peak)
        .collect();
    let level = median(&mut inside)
        .filter(|&level| level > 0.0)
        .ok_or(LayoutError::NoText)?;
    let threshold = LOW * level;
    let is_ink = |v: &f64| *v >= threshold;
    let first = across
        .bins
        .iter()
        .position(is_ink)
        .ok_or(LayoutError::NoText)?;
    let last = across
        .bins
        .iter()
        .rposition(is_ink)
        .ok_or(LayoutError::NoText)?;

    // The gap: the widest empty strip that lies near the middle of the text.
    let text_width = (last + 1 - first) as f64;
    let mut gutter: Option<(usize, usize)> = None;
    let mut index = first;
    while index <= last {
        if across.bins[index] >= threshold {
            index += 1;
            continue;
        }
        let start = index;
        while index <= last && across.bins[index] < threshold {
            index += 1;
        }
        let width = index - start;
        let centre = ((start + index) as f64 / 2.0 - first as f64) / text_width;
        if width as f64 >= MIN_GUTTER
            && (CENTRAL.0..=CENTRAL.1).contains(&centre)
            && gutter.is_none_or(|(s, e)| width > e - s)
        {
            gutter = Some((start, index));
        }
    }
    let (gutter_start, gutter_end) = gutter.ok_or(LayoutError::NotTwoColumn)?;
    let left = Span {
        lo: across.position(first),
        hi: across.position(gutter_start),
    };
    let gutter = Span {
        lo: across.position(gutter_start),
        hi: across.position(gutter_end),
    };
    let right = Span {
        lo: across.position(gutter_end),
        hi: across.position(last + 1),
    };

    let columns = Span {
        lo: left.lo,
        hi: right.hi,
    };
    let type_area = type_area(&usable, bounds, columns).ok_or(LayoutError::NoText)?;
    Ok(Layout {
        left,
        gutter,
        right,
        type_area,
    })
}

/// The vertical extent of the text block: all rows that hold ink on some page,
/// minus running headers and footers.
///
/// A running header or footer is recognised by its shape across the whole
/// document: a strip no taller than a line or two, at the very top or bottom,
/// with white space between it and everything else on *every* page.
fn type_area(pages: &[&PageInk], bounds: Rect, columns: Span) -> Option<Span> {
    let mut down = Profile::new(bounds.y0, bounds.y1);
    let mut text_heights = Vec::new();
    for page in pages {
        for ink in &page.boxes {
            if is_background(ink, page.media) {
                continue;
            }
            let width = columns.overlap(ink.rect.x0, ink.rect.x1);
            if width > 0.0 {
                down.add(ink.rect.y0, ink.rect.y1, width);
            }
            if ink.kind == InkKind::Text {
                text_heights.push(ink.rect.height());
            }
        }
    }
    let line_height = median(&mut text_heights)?;

    // Blocks of rows with ink, joined across gaps too small to mean anything.
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    for (index, &value) in down.bins.iter().enumerate() {
        if value <= 1.0 {
            continue;
        }
        match blocks.last_mut() {
            Some((_, end)) if index - *end < BLOCK_GAP => *end = index + 1,
            _ => blocks.push((index, index + 1)),
        }
    }

    let is_thin = |&(start, end): &(usize, usize)| (end - start) as f64 <= 1.8 * line_height + 2.0;
    for _ in 0..2 {
        if blocks.len() > 1 && blocks.first().is_some_and(is_thin) {
            blocks.remove(0);
        }
        if blocks.len() > 1 && blocks.last().is_some_and(is_thin) {
            blocks.pop();
        }
    }
    Some(Span {
        lo: down.position(blocks.first()?.0),
        hi: down.position(blocks.last()?.1),
    })
}

/// Decides, for every page, which crops replace it.
pub fn plan(pages: &[PageInk], layout: &Layout) -> Vec<PagePlan> {
    let inner_pad = PAD.min(layout.gutter.width() / 2.0);
    pages
        .iter()
        .map(|page| {
            if !page.upright || page.boxes.is_empty() {
                return PagePlan::Keep;
            }
            let crops: Vec<Rect> = if is_single_column(page, layout) {
                let both = Span {
                    lo: layout.left.lo,
                    hi: layout.right.hi,
                };
                crop(page, both, layout.type_area, PAD, PAD)
                    .into_iter()
                    .collect()
            } else {
                [
                    crop(page, layout.left, layout.type_area, PAD, inner_pad),
                    crop(page, layout.right, layout.type_area, inner_pad, PAD),
                ]
                .into_iter()
                .flatten()
                .collect()
            };
            if crops.is_empty() {
                PagePlan::Keep
            } else {
                PagePlan::Crops(crops)
            }
        })
        .collect()
}

/// True when this page's text fills the gap about as densely as the columns:
/// a page set in one wide column, which must not be cut in two.
fn is_single_column(page: &PageInk, layout: &Layout) -> bool {
    let mut gutter_ink = 0.0;
    let mut column_ink = 0.0;
    for ink in &page.boxes {
        if ink.kind != InkKind::Text {
            continue;
        }
        let height = layout.type_area.overlap(ink.rect.y0, ink.rect.y1);
        let (x0, x1) = (ink.rect.x0, ink.rect.x1);
        gutter_ink += layout.gutter.overlap(x0, x1) * height;
        column_ink += (layout.left.overlap(x0, x1) + layout.right.overlap(x0, x1)) * height;
    }
    let gutter_density = gutter_ink / layout.gutter.width();
    let column_density = column_ink / (layout.left.width() + layout.right.width());
    column_density > 0.0 && gutter_density >= SPANNING * column_density
}

/// The tight crop of one column on one page: the column's full width, and only
/// as tall as the ink that is actually there. `None` when the column is empty.
fn crop(page: &PageInk, across: Span, down: Span, pad_lo: f64, pad_hi: f64) -> Option<Rect> {
    let window = Rect::new(across.lo, down.lo, across.hi, down.hi);
    let extent = page
        .boxes
        .iter()
        .filter(|ink| !is_background(ink, page.media))
        .filter_map(|ink| ink.rect.intersect(window))
        .filter(|rect| rect.width() > 0.5)
        .reduce(Rect::union)?;
    Rect::new(
        across.lo - pad_lo,
        extent.y0 - PAD,
        across.hi + pad_hi,
        extent.y1 + PAD,
    )
    .intersect(page.media)
}
