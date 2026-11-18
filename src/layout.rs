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
/// Rows that cross the gap and lie closer together than this (in points) are
/// one full-width band: the lines of a caption, a figure and its caption.
const BAND_JOIN: usize = 24;
/// The widest space between two words of one line, as a fraction of the
/// text's height. Text further apart than this is not one line.
const WORD_SPACE: f64 = 0.55;
/// A full-width band is at least this tall, so that a lone rule is not one.
const MIN_BAND: usize = 6;
/// Within a full-width region, ink separated by at least this much white (in
/// points) is a separate part: a figure, or its caption.
const FIGURE_GAP: usize = 8;
/// White space kept above and below a full-width band, in points.
const BAND_PAD: f64 = 2.0;
/// A band turned sideways is magnified at most this much.
const MAX_TURNED_SCALE: f64 = 1.5;
/// Turning a band sideways must show it at least this much larger than
/// shrinking it would, or it is not worth holding the phone sideways for.
const TURN_GAIN: f64 = 1.25;

/// The ink of one page, with the page's visible rectangle.
pub struct PageInk {
    pub media: Rect,
    /// The page's stored rotation in degrees: 0, 90, 180 or 270. Rotated pages
    /// are not searched for columns; they are kept whole.
    pub rotation: i64,
    pub boxes: Vec<InkBox>,
}

impl PageInk {
    fn upright(&self) -> bool {
        self.rotation == 0
    }

    /// The page's width as a reader shows it, rotation applied.
    fn shown_width(&self) -> f64 {
        if self.rotation % 180 == 0 {
            self.media.width()
        } else {
            self.media.height()
        }
    }
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
    /// Height of a typical run of text, in points.
    pub line_height: f64,
}

/// Choices the person running the tool can make.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// Turn full-width figures and tables a quarter turn when that shows them
    /// larger than shrinking them would. Off unless asked for: it makes the
    /// figure bigger, but it has to be read with the phone held sideways.
    pub sideways: bool,
}

/// One output page: a rectangle cut out of a source page and drawn at `scale`.
///
/// Every piece of a document comes out the same width (`rect.width() * scale`
/// is the same for all of them). A phone reader sizes its view to the widest
/// page, so a single wider page would zoom every other page out (ADR-0003).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub rect: Rect,
    /// 1.0 for a column; less than 1.0 for content wider than a column, which
    /// is shrunk to fit.
    pub scale: f64,
    /// True when the piece is shown turned a quarter turn (its top to the left).
    pub turned: bool,
}

impl Piece {
    /// A column, cut out as it is.
    fn column(rect: Rect) -> Piece {
        Piece {
            rect,
            scale: 1.0,
            turned: false,
        }
    }

    /// A piece `width` wide once drawn, however wide `rect` is.
    fn fitted(rect: Rect, shown_width: f64, width: f64) -> Piece {
        let scale = width / shown_width;
        Piece {
            rect,
            // Leave true columns exactly alone: no rounding-sized rescaling.
            scale: if (scale - 1.0).abs() < 0.005 {
                1.0
            } else {
                scale
            },
            turned: false,
        }
    }

    /// A wide, short piece shown on its side, so that its height (not its
    /// width) has to fit the column width. Never wider than a column on
    /// screen; narrower when the magnification limit is reached first.
    fn sideways(rect: Rect, width: f64) -> Piece {
        Piece {
            rect,
            scale: (width / rect.height()).min(MAX_TURNED_SCALE),
            turned: true,
        }
    }
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
        .filter(|page| page.upright() && !page.boxes.is_empty())
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
    let (type_area, line_height) =
        type_area(&usable, bounds, columns).ok_or(LayoutError::NoText)?;
    Ok(Layout {
        left,
        gutter,
        right,
        type_area,
        line_height,
    })
}

/// The vertical extent of the text block: all rows that hold ink on some page,
/// minus running headers and footers.
///
/// A running header or footer is recognised by its shape across the whole
/// document: a strip no taller than a line or two, at the very top or bottom,
/// with white space between it and everything else on *every* page.
///
/// Also returns the height of a typical run of text, which is what "a line or
/// two" is measured against.
fn type_area(pages: &[&PageInk], bounds: Rect, columns: Span) -> Option<(Span, f64)> {
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
    let area = Span {
        lo: down.position(blocks.first()?.0),
        hi: down.position(blocks.last()?.1),
    };
    Some((area, line_height))
}

/// Decides, for every page, which pieces replace it, in reading order.
///
/// A two-column page is read from the top down. Each stretch of two columns
/// gives one piece per column that has ink; each full-width band between them
/// (a title, a figure or table that spans both columns) gives one piece of
/// its own. Anything that is not a two-column page — a page set in one wide
/// column, a rotated page, a blank page — gives a single piece. Every piece
/// that is not a column is shrunk to the width of one.
pub fn plan(pages: &[PageInk], layout: &Layout, options: Options) -> Vec<Vec<Piece>> {
    let inner_pad = PAD.min(layout.gutter.width() / 2.0);
    // Both columns are cut to one width: the wider of the two, measured from
    // the gap outwards, so the narrower one gains a little outer margin.
    let width = layout.left.width().max(layout.right.width()) + PAD + inner_pad;
    let left = Span {
        lo: layout.left.hi + inner_pad - width,
        hi: layout.left.hi + inner_pad,
    };
    let right = Span {
        lo: layout.right.lo - inner_pad,
        hi: layout.right.lo - inner_pad + width,
    };
    let both = Span {
        lo: layout.left.lo - PAD,
        hi: layout.right.hi + PAD,
    };
    let padded_type_area = Span {
        lo: layout.type_area.lo - PAD,
        hi: layout.type_area.hi + PAD,
    };

    pages
        .iter()
        .map(|page| {
            let whole = Piece::fitted(page.media, page.shown_width(), width);
            if !page.upright() {
                return vec![whole];
            }
            let pieces: Vec<Piece> = if is_single_column(page, layout) {
                crop(page, both, layout.type_area, padded_type_area)
                    .map(|rect| full_width(page, rect, width, options))
                    .unwrap_or_default()
            } else {
                let mut pieces = Vec::new();
                // The columns of one stretch between bands. A crop gets its
                // white space only where the text block ends; where it meets a
                // band it stops at the band, so neither shows a sliver of the other.
                let columns_between = |pieces: &mut Vec<Piece>, down: Span| {
                    let limit = Span {
                        lo: if down.lo <= layout.type_area.lo {
                            down.lo - PAD
                        } else {
                            down.lo
                        },
                        hi: if down.hi >= layout.type_area.hi {
                            down.hi + PAD
                        } else {
                            down.hi
                        },
                    };
                    if down.width() > 0.0 {
                        pieces.extend(
                            [left, right]
                                .into_iter()
                                .filter_map(|column| crop(page, column, down, limit))
                                .map(Piece::column),
                        );
                    }
                };
                let mut top = layout.type_area.hi;
                for band in bands(page, layout) {
                    columns_between(
                        &mut pieces,
                        Span {
                            lo: band.hi,
                            hi: top,
                        },
                    );
                    let limit = Span {
                        lo: band.lo - BAND_PAD,
                        hi: band.hi + BAND_PAD,
                    };
                    if let Some(rect) = crop(page, both, band, limit) {
                        pieces.extend(full_width(page, rect, width, options));
                    }
                    top = band.lo;
                }
                columns_between(
                    &mut pieces,
                    Span {
                        lo: layout.type_area.lo,
                        hi: top,
                    },
                );
                pieces
            };
            if pieces.is_empty() {
                vec![whole]
            } else {
                pieces
            }
        })
        .collect()
}

/// The full-width bands of a two-column page, from the top down: the vertical
/// stretches where ink runs across the gap between the columns.
///
/// A row "crosses" when ink runs unbroken from one side of the gap to the
/// other. That is true of a title, a caption line or a wide figure; it is not
/// true of a page number sitting in the gap, of a column's figure that sticks
/// out a little, or of a vertical rule between the columns.
///
/// Words on a line are separate runs of text with spaces between them, so text
/// counts as unbroken across a space no wider than about half its own size.
/// The gap between two columns is wider than that, which is what keeps two
/// ordinary lines side by side from looking like one line that crosses.
fn bands(page: &PageInk, layout: &Layout) -> Vec<Span> {
    let origin = layout.type_area.lo.floor();
    let rows = (layout.type_area.hi.ceil() - origin).max(0.0) as usize;
    let columns = Span {
        lo: layout.left.lo,
        hi: layout.right.hi,
    };
    // Per row: which of 64 slices of the gap hold ink, and whether the row
    // holds any ink at all.
    let mut gap_ink = vec![0u64; rows];
    let mut inked = vec![false; rows];
    for ink in &page.boxes {
        let rect = ink.rect;
        if is_background(ink, page.media)
            || columns.overlap(rect.x0, rect.x1) <= 0.0
            || rect.y1 < origin
            || rect.y0 > origin + rows as f64
        {
            continue;
        }
        let first = (rect.y0 - origin).floor().max(0.0) as usize;
        let last = ((rect.y1 - origin).ceil().max(0.0) as usize).max(first + 1);
        // Half a word space on each side, so that neighbouring words meet.
        let reach = if ink.kind == InkKind::Text {
            WORD_SPACE / 2.0 * rect.height()
        } else {
            0.0
        };
        let mask = gap_mask(layout.gutter, rect.x0 - reach, rect.x1 + reach);
        for row in first..last.min(rows) {
            inked[row] = true;
            gap_ink[row] |= mask;
        }
    }
    let crosses = |mask: u64| mask == u64::MAX;
    let in_core = |mask: u64| mask & 0x0000_FFFF_FFFF_0000 != 0;

    // Runs of crossing rows become one band when they are close together,
    // when the gap keeps holding ink in between (the inside of a table or a
    // drawing), or when only a few lines sit between them (the authors' names
    // between a title and the affiliations).
    let few_lines = (3.5 * layout.line_height) as usize;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for row in (0..rows).filter(|&row| crosses(gap_ink[row])) {
        let joins = runs.last().is_some_and(|&(_, end)| {
            let between = end..row;
            let busy = between.clone().filter(|&r| in_core(gap_ink[r])).count();
            let lines = between.clone().filter(|&r| inked[r]).count();
            between.len() < BAND_JOIN
                || 2 * busy >= between.len()
                || (1..=few_lines).contains(&lines)
        });
        match runs.last_mut() {
            Some((_, end)) if joins => *end = row + 1,
            _ => runs.push((row, row + 1)),
        }
    }
    runs.retain(|(start, end)| end - start >= MIN_BAND);

    // A band also takes the one short line that hangs just above or below it
    // without crossing the gap: the last line of a caption, typically.
    let line = (1.4 * layout.line_height + 1.0) as usize;
    for (start, end) in runs.iter_mut() {
        *start -= stray_line(inked[..*start].iter().rev(), line);
        *end += stray_line(inked[*end..].iter(), line);
    }

    let mut bands: Vec<Span> = Vec::new();
    for (start, end) in runs.into_iter().rev() {
        let band = Span {
            lo: origin + start as f64,
            hi: origin + end as f64,
        };
        match bands.last_mut() {
            Some(above) if band.hi >= above.lo => above.lo = above.lo.min(band.lo),
            _ => bands.push(band),
        }
    }
    bands
}

/// Which of 64 equal slices of the gap the range `x0..x1` touches, as bits.
fn gap_mask(gutter: Span, x0: f64, x1: f64) -> u64 {
    if x1 <= gutter.lo || x0 >= gutter.hi {
        return 0;
    }
    let slice = |x: f64| (x.clamp(gutter.lo, gutter.hi) - gutter.lo) / gutter.width() * 64.0;
    let first = (slice(x0).floor() as u32).min(63);
    let last = (slice(x1).ceil() as u32).clamp(first + 1, 64);
    (u64::MAX >> (64 - (last - first))) << first
}

/// Walking away from a band over `rows` (true where a row holds ink): how many
/// rows to add so as to take in one short line that sits right next to the band
/// and is clearly set apart from whatever comes after it. Zero if there is none.
fn stray_line<'a>(rows: impl Iterator<Item = &'a bool>, line: usize) -> usize {
    let mut rows = rows.copied().peekable();
    let mut count = |want: bool, limit: usize| {
        let mut n = 0;
        while n < limit && rows.next_if_eq(&want).is_some() {
            n += 1;
        }
        n
    };
    let before = count(false, 5);
    let run = count(true, line + 1);
    let after = count(false, before + 3);
    let at_end = rows.peek().is_none();
    if before <= 4 && (1..=line).contains(&run) && (after >= before + 3 || at_end) {
        before + run
    } else {
        0
    }
}

/// The pieces for one full-width region (a band, or a whole single-column
/// page). Normally one piece, shrunk to the width of a column. With the
/// sideways option, each figure in the region that would show larger on its
/// side becomes a turned piece of its own, and what lies between the turned
/// figures stays upright and shrunk.
fn full_width(page: &PageInk, region: Rect, width: f64, options: Options) -> Vec<Piece> {
    let shrunk = |rect: Rect| Piece::fitted(rect, rect.width(), width);
    if !options.sideways {
        return vec![shrunk(region)];
    }
    let mut pieces = Vec::new();
    let mut upright: Option<Rect> = None;
    for block in figure_blocks(page, region) {
        let turned = Piece::sideways(block, width);
        if turned.scale >= TURN_GAIN * shrunk(block).scale && is_figure(page, block) {
            pieces.extend(upright.take().map(shrunk));
            pieces.push(turned);
        } else {
            upright = Some(upright.map_or(block, |above| above.union(block)));
        }
    }
    pieces.extend(upright.map(shrunk));
    pieces
}

/// Cuts a full-width region into the separate figures stacked in it, each with
/// its caption, from the top down.
///
/// Stacked figures repeat a pattern: drawing then caption, drawing then
/// caption (or, for tables, caption then drawing). A new block starts each
/// time the pattern comes back to the kind of thing the region began with.
fn figure_blocks(page: &PageInk, region: Rect) -> Vec<Rect> {
    let origin = region.y0.floor();
    let rows = (region.y1.ceil() - origin).max(1.0) as usize;
    let mut text = vec![false; rows];
    let mut drawing = vec![false; rows];
    for ink in &page.boxes {
        if is_background(ink, page.media) {
            continue;
        }
        let Some(rect) = ink.rect.intersect(region) else {
            continue;
        };
        let first = ((rect.y0 - origin).floor().max(0.0) as usize).min(rows - 1);
        let last = ((rect.y1 - origin).ceil() as usize).clamp(first + 1, rows);
        let marks = if ink.kind == InkKind::Text {
            &mut text
        } else {
            &mut drawing
        };
        marks[first..last].fill(true);
    }

    // Runs of rows with ink, from the top down, each either mostly drawing or
    // mostly text.
    let mut runs: Vec<(usize, usize, bool)> = Vec::new();
    let mut row = rows;
    while row > 0 {
        row -= 1;
        if !(text[row] || drawing[row]) {
            continue;
        }
        let end = row + 1;
        let mut start = row;
        let mut blank = 0;
        while start > 0 && blank < FIGURE_GAP {
            start -= 1;
            blank = if text[start] || drawing[start] {
                0
            } else {
                blank + 1
            };
        }
        start += blank;
        let drawn = drawing[start..end].iter().filter(|&&d| d).count();
        runs.push((start, end, 10 * drawn >= 3 * (end - start)));
        row = start;
    }

    let mut blocks: Vec<(usize, usize)> = Vec::new();
    let mut seen_other_kind = false;
    for &(start, end, is_drawing) in &runs {
        let opens = is_drawing == runs[0].2;
        match blocks.last_mut() {
            Some(block) if !(opens && seen_other_kind) => {
                block.0 = start;
                seen_other_kind |= !opens;
            }
            _ => {
                blocks.push((start, end));
                seen_other_kind = false;
            }
        }
    }
    blocks
        .into_iter()
        .map(|(start, end)| {
            Rect::new(
                region.x0,
                (origin + start as f64 - BAND_PAD).max(region.y0),
                region.x1,
                (origin + end as f64 + BAND_PAD).min(region.y1),
            )
        })
        .collect()
}

/// True when a band holds a picture or a ruled table, not just text: ink that
/// is not text and takes up a fair share of the band's height.
fn is_figure(page: &PageInk, band: Rect) -> bool {
    page.boxes
        .iter()
        .filter(|ink| ink.kind != InkKind::Text && !is_background(ink, page.media))
        .filter_map(|ink| ink.rect.intersect(band))
        .reduce(Rect::union)
        .is_some_and(|drawn| drawn.height() >= 0.3 * band.height())
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

/// The tight crop of one column on one page: exactly `across` wide, and only as
/// tall as the ink that is actually there. `None` when the column is empty.
///
/// Ink is looked for within `down`; the white space added around it never
/// reaches past `limit`.
fn crop(page: &PageInk, across: Span, down: Span, limit: Span) -> Option<Rect> {
    let window = Rect::new(across.lo, down.lo, across.hi, down.hi);
    let extent = page
        .boxes
        .iter()
        .filter(|ink| !is_background(ink, page.media))
        .filter_map(|ink| {
            // Ink that only touches the window's edge from outside (a figure
            // ending exactly where this stretch begins) is not in it. A rule,
            // which has no height of its own, still counts.
            ink.rect
                .intersect(window)
                .filter(|part| part.height() > 0.0 || ink.rect.height() == 0.0)
        })
        .filter(|rect| rect.width() > 0.5)
        .reduce(Rect::union)?;
    // The height is kept inside the page; the width is not clipped, so that
    // every crop is exactly as wide as the others.
    Some(Rect::new(
        across.lo,
        (extent.y0 - PAD).max(limit.lo).max(page.media.y0),
        across.hi,
        (extent.y1 + PAD).min(limit.hi).min(page.media.y1),
    ))
}
