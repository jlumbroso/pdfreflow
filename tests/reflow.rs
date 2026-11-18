//! End-to-end tests: build a small two-column "paper", reflow it, write it out,
//! read it back, and check the pages a reader would actually get.
//!
//! The paper is synthetic on purpose. Real inputs are confidential papers under
//! review and the development stand-in is copyrighted, so neither can live in
//! this repository (see CLAUDE.md, "Confidential inputs").

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, ObjectId, Stream, dictionary};

use pdfreflow::geom::Rect;
use pdfreflow::layout::LayoutError;
use pdfreflow::{Error, Options, cut, ink, reflow};

// The synthetic paper: US Letter, Courier 10pt (every glyph 6pt wide), two
// columns of 40 characters with a 24pt gap, lines 12pt apart.
const LEFT: f64 = 54.0;
const RIGHT: f64 = 318.0;
const COLUMN_WIDTH: f64 = 240.0;
const TOP: i32 = 700;
const BOTTOM: i32 = 100;
// Courier's box around a 10pt line, as the tool estimates it: 7.5 above the
// baseline and 2.2 below.
const ASCENT: f64 = 7.5;
const DESCENT: f64 = 2.2;
// White space the tool leaves around a column, and around a full-width band.
const PAD: f64 = 4.0;
const BAND_PAD: f64 = 2.0;
// The full width of the text, from the left column's edge to the right one's,
// with its white space: what a full-width piece is cut to before shrinking.
const FULL_WIDTH: f64 = RIGHT + COLUMN_WIDTH - LEFT + 2.0 * PAD;

struct Line {
    x: f64,
    y: i32,
    chars: usize,
}

/// One page of the synthetic paper: lines of text, and filled rectangles that
/// stand in for figures.
struct Page {
    lines: Vec<Line>,
    figures: Vec<Rect>,
}

/// A column of full-width lines from baseline `from` down to baseline `to`.
fn column(x: f64, from: i32, to: i32) -> Vec<Line> {
    (to..=from)
        .rev()
        .step_by(12)
        .map(|y| Line { x, y, chars: 40 })
        .collect()
}

/// A page with what every page of a paper carries outside its text block: a
/// running header above the left column and a page number centred in the gap.
fn page() -> Page {
    Page {
        lines: vec![
            Line {
                x: LEFT,
                y: 750,
                chars: 16,
            },
            Line {
                x: 303.0,
                y: 60,
                chars: 1,
            },
        ],
        figures: Vec::new(),
    }
}

/// An ordinary page: two full columns.
fn plain_page() -> Page {
    let mut plain = page();
    plain.lines.extend(column(LEFT, TOP, BOTTOM));
    plain.lines.extend(column(RIGHT, TOP, BOTTOM));
    plain
}

fn paper(pages: Vec<Page>) -> Document {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
        "FirstChar" => 32,
        "LastChar" => 126,
        "Widths" => (32..=126).map(|_| Object::Integer(600)).collect::<Vec<_>>(),
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let mut kids: Vec<Object> = Vec::new();
    for page in &pages {
        let mut operations = Vec::new();
        for figure in &page.figures {
            let numbers = [figure.x0, figure.y0, figure.width(), figure.height()];
            operations.push(Operation::new(
                "re",
                numbers.iter().map(|&n| Object::Real(n as f32)).collect(),
            ));
            operations.push(Operation::new("f", vec![]));
        }
        for line in &page.lines {
            operations.push(Operation::new("BT", vec![]));
            operations.push(Operation::new("Tf", vec!["F1".into(), 10.into()]));
            operations.push(Operation::new(
                "Td",
                vec![Object::Real(line.x as f32), Object::Integer(line.y.into())],
            ));
            operations.push(Operation::new(
                "Tj",
                vec![Object::string_literal("x".repeat(line.chars))],
            ));
            operations.push(Operation::new("ET", vec![]));
        }
        let content = Content { operations }.encode().unwrap();
        let content_id = doc.add_object(Stream::new(dictionary! {}, content));
        // Like many real PDFs, the pages inherit their size and fonts from the
        // page tree instead of stating them.
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        kids.push(page_id.into());
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => pages.len() as i64,
            "Kids" => kids,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc
}

/// A three-page paper:
/// - page 1 has a two-line title that crosses the gap, above two full columns;
/// - page 2 has a full left column and a right column that stops halfway;
/// - page 3 has six lines in the left column and nothing on the right.
fn three_page_paper() -> Document {
    let mut first = page();
    first.lines.extend([700, 688].map(|y| Line {
        x: 150.0,
        y,
        chars: 52,
    }));
    first.lines.extend(column(LEFT, 664, BOTTOM));
    first.lines.extend(column(RIGHT, 664, BOTTOM));

    let mut second = page();
    second.lines.extend(column(LEFT, TOP, BOTTOM));
    second.lines.extend(column(RIGHT, TOP, 400));

    let mut third = page();
    third.lines.extend(column(LEFT, TOP, 640));

    paper(vec![first, second, third])
}

/// What a reader would open: the document reflowed, written to a PDF in
/// memory, and read back. Gives the pages in order, each with its visible
/// rectangle.
struct Reflowed {
    doc: Document,
    ids: Vec<ObjectId>,
    pages: Vec<Rect>,
}

impl Reflowed {
    /// How wide a page is on screen, its quarter turn (if any) applied.
    fn shown_width(&self, index: usize) -> f64 {
        if cut::page_rotation(&self.doc, self.ids[index]) % 180 == 0 {
            self.pages[index].width()
        } else {
            self.pages[index].height()
        }
    }

    /// The boxes of the text that is visible on a page.
    fn visible_text(&self, index: usize) -> Vec<Rect> {
        let page = self.pages[index];
        ink::page_ink(&self.doc, self.ids[index])
            .into_iter()
            .filter(|ink| ink.kind == ink::InkKind::Text)
            .map(|ink| ink.rect)
            .filter(|rect| rect.y0 >= page.y0 && rect.y1 <= page.y1)
            .collect()
    }
}

fn reflowed_with(mut doc: Document, options: Options) -> Reflowed {
    reflow(&mut doc, options).expect("the paper should reflow");
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("the result should save");
    let doc = Document::load_mem(&bytes).expect("the result should be a readable PDF");
    let ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
    let pages = ids.iter().map(|&id| cut::page_box(&doc, id)).collect();
    Reflowed { doc, ids, pages }
}

fn reflowed(doc: Document) -> Reflowed {
    reflowed_with(doc, Options::default())
}

fn assert_near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 2.0,
        "{what}: expected about {expected}, got {actual}"
    );
}

fn assert_left_column(page: &Rect) {
    assert_near(page.x0, LEFT - PAD, "left column, left edge");
    assert_near(
        page.x1,
        LEFT + COLUMN_WIDTH + PAD,
        "left column, right edge",
    );
}

fn assert_right_column(page: &Rect) {
    assert_near(page.x0, RIGHT - PAD, "right column, left edge");
    assert_near(
        page.x1,
        RIGHT + COLUMN_WIDTH + PAD,
        "right column, right edge",
    );
}

#[test]
fn each_column_becomes_its_own_page_in_reading_order() {
    let result = reflowed(three_page_paper());
    let pages = &result.pages;

    // The title, then two columns, two columns, and a page whose right column
    // is empty.
    assert_eq!(pages.len(), 6);
    assert_left_column(&pages[1]);
    assert_right_column(&pages[2]);
    assert_left_column(&pages[3]);
    assert_right_column(&pages[4]);
    assert_left_column(&pages[5]);
}

#[test]
fn a_title_across_both_columns_comes_first_and_stays_whole() {
    let result = reflowed(three_page_paper());

    // The first page of the result is the title alone: both of its lines,
    // complete, and nothing else.
    let title = result.pages[0];
    let lines = result.visible_text(0);
    assert_eq!(lines.len(), 2);
    for line in &lines {
        assert!(
            line.x0 >= title.x0 && line.x1 <= title.x1,
            "a title line is cut off: {line:?} in {title:?}"
        );
    }
    // The columns under it start below the title, not through it.
    assert_near(
        result.pages[1].y1,
        664.0 + ASCENT + PAD,
        "first column, top edge",
    );
}

#[test]
fn crops_are_tight_and_leave_out_the_header_and_page_number() {
    let result = reflowed(three_page_paper());
    let pages = &result.pages;
    let top = f64::from(TOP) + ASCENT + PAD;
    let bottom = f64::from(BOTTOM) - DESCENT - PAD;

    // Full columns run from the top of the text block to its bottom. The
    // running header (baseline 750) and the page number (baseline 60) are
    // outside every crop, although the page number sits right in the gap.
    assert_near(pages[3].y1, top, "full column, top edge");
    for page in &pages[1..4] {
        assert_near(page.y0, bottom, "full column, bottom edge");
    }

    // A column that stops halfway down gives a short page, not a tall one
    // that is half empty.
    assert_near(pages[4].y1, top, "half column, top edge");
    assert_near(
        pages[4].y0,
        400.0 - DESCENT - PAD,
        "half column, bottom edge",
    );
    assert_near(
        pages[5].y0,
        640.0 - DESCENT - PAD,
        "six-line column, bottom edge",
    );
}

#[test]
fn the_cut_shares_the_page_content_instead_of_copying_it() {
    let mut doc = three_page_paper();
    let contents = |doc: &Document| -> Vec<Vec<u8>> {
        doc.get_pages()
            .into_values()
            .map(|id| doc.get_page_content(id))
            .collect()
    };
    let before = contents(&doc);
    reflow(&mut doc, Options::default()).unwrap();
    let after = contents(&doc);

    // Every piece of a source page still draws the whole source page; only the
    // page box differs. This is what keeps text sharp and selectable.
    assert_eq!(after[1], before[0]);
    assert_eq!(after[2], before[0]);
    assert_eq!(after[3], before[1]);
    assert_eq!(after[4], before[1]);
    assert_eq!(after[5], before[2]);
    // The shrunk title is the same drawing again, wrapped in a scaling
    // instruction, not a redrawn or rendered copy.
    assert!(
        after[0]
            .windows(before[0].len())
            .any(|part| part == before[0]),
        "the title page should still contain the source page's drawing"
    );
}

/// A figure (a filled rectangle with a caption under it) across both columns
/// at the top of a page, with the two columns below it; then an ordinary page.
fn paper_with_a_spanning_figure() -> Document {
    let mut first = page();
    first.figures.push(Rect::new(60.0, 560.0, 552.0, 700.0));
    first.lines.push(Line {
        x: 100.0,
        y: 540,
        chars: 68,
    });
    first.lines.extend(column(LEFT, 508, BOTTOM));
    first.lines.extend(column(RIGHT, 508, BOTTOM));
    paper(vec![first, plain_page()])
}

#[test]
fn a_figure_across_both_columns_is_kept_whole_on_its_own_page() {
    let result = reflowed(paper_with_a_spanning_figure());

    // The figure with its caption, then the columns below it, then page 2.
    assert_eq!(result.pages.len(), 5);
    let figure = result.pages[0];
    // Shrunk to the width of a column, and as tall as figure plus caption.
    let scale = figure.width() / FULL_WIDTH;
    let height = (700.0 + BAND_PAD) - (540.0 - DESCENT - BAND_PAD);
    assert_near(
        figure.height(),
        height * scale,
        "figure page, height once shrunk",
    );
    assert_eq!(
        result.visible_text(0).len(),
        1,
        "the caption stays with the figure"
    );
    assert_left_column(&result.pages[1]);
    assert_near(
        result.pages[1].y1,
        508.0 + ASCENT + PAD,
        "column under the figure, top edge",
    );
}

#[test]
fn with_the_sideways_option_a_wide_figure_is_turned_and_shown_larger() {
    let upright = reflowed(paper_with_a_spanning_figure());
    let sideways = reflowed_with(paper_with_a_spanning_figure(), Options { sideways: true });

    assert_eq!(
        cut::page_rotation(&sideways.doc, sideways.ids[0]),
        270,
        "the figure page should be turned a quarter turn"
    );
    // Turned, the figure is drawn larger than life; shrunk, at half size.
    // (Before the turn is applied, the page box's width is the figure's.)
    assert!(sideways.pages[0].width() > 1.2 * FULL_WIDTH);
    assert!(upright.pages[0].width() < 0.6 * FULL_WIDTH);
    // It is still no wider on screen than a column, so the zoom is untouched.
    let column_width = sideways.shown_width(1);
    assert!(sideways.shown_width(0) <= column_width + 0.1);
    // The columns are not turned.
    assert_eq!(cut::page_rotation(&sideways.doc, sideways.ids[1]), 0);
}

#[test]
fn with_the_sideways_option_stacked_figures_are_turned_one_by_one() {
    // The page that started ADR-0003: nothing on it but full-width figures,
    // one above the other, each with its caption.
    let mut figures = page();
    for (top, caption) in [(680.0, 480), (380.0, 180)] {
        figures
            .figures
            .push(Rect::new(60.0, top - 180.0, 552.0, top));
        figures.lines.push(Line {
            x: 100.0,
            y: caption,
            chars: 68,
        });
    }
    let doc = paper(vec![plain_page(), plain_page(), figures]);

    let result = reflowed_with(doc, Options { sideways: true });

    // Four columns, then one turned page per figure, each with its caption.
    assert_eq!(result.pages.len(), 6);
    for index in [4, 5] {
        assert_eq!(cut::page_rotation(&result.doc, result.ids[index]), 270);
        assert_eq!(result.visible_text(index).len(), 1);
    }
}

#[test]
fn a_sideways_page_comes_out_as_wide_as_a_column() {
    // Papers sometimes store a page turned on its side (a landscape table).
    // Such a page is not searched for columns, but it must still not be wider
    // than the others on screen.
    let mut doc = three_page_paper();
    let last = doc.get_pages().into_values().last().unwrap();
    doc.get_dictionary_mut(last).unwrap().set("Rotate", 90);

    let result = reflowed(doc);
    let last = result.pages.len() - 1;

    assert_eq!(cut::page_rotation(&result.doc, result.ids[last]), 90);
    assert_near(
        result.shown_width(last),
        result.shown_width(1),
        "sideways page, width on screen",
    );
}

mod regression_tests {
    use super::*;

    #[test]
    fn a_single_column_page_inside_a_two_column_paper_is_not_cut_in_two() {
        // Bug (found 2026-11-18, before the first release): the columns were
        // found by adding up the ink of all pages. With one page in three set
        // in a single wide column, that page's ink filled the gap in the sum
        // and the whole paper was refused as "not two-column".
        // Fix: find the columns from the typical page instead (a median over
        // pages, `layout::detect`), so a minority of unusual pages cannot hide
        // the gap. ADR-0002, Iteration 2.
        let result = reflowed(paper_with_a_wide_page());
        let pages = &result.pages;

        // The two ordinary pages are cut; the wide page stays in one piece,
        // which therefore has the proportions of the whole text block and not
        // of a column.
        assert_eq!(pages.len(), 5);
        let block_height = f64::from(TOP - BOTTOM) + ASCENT + DESCENT + 2.0 * PAD;
        let proportions = pages[4].height() / pages[4].width();
        assert!(
            (proportions - block_height / FULL_WIDTH).abs() < 0.02,
            "the wide page should keep the text block's proportions, got {proportions}"
        );
    }

    #[test]
    fn one_wide_page_does_not_make_every_other_page_look_small() {
        // Bug (found 2026-11-18 by the human, reading a real manuscript on a
        // phone): a page kept in one piece came out twice as wide as the
        // column pages. A phone reader sizes its view to the widest page, so
        // that one page zoomed every other page out and the reflow was undone.
        // Fix: every output page has the same width; content wider than a
        // column is shrunk to fit. ADR-0003, QST-WIDE-PAGES.
        let result = reflowed(paper_with_a_wide_page());

        let width = result.shown_width(0);
        for index in 0..result.pages.len() {
            assert!(
                (result.shown_width(index) - width).abs() < 0.1,
                "page {} is {} wide, the first is {width}",
                index + 1,
                result.shown_width(index)
            );
        }

        // Shrinking must move the text along with the page: all 51 lines of
        // the wide page are drawn inside the shrunk page, edge to edge.
        let wide = result.pages[4];
        let lines = result.visible_text(4);
        assert_eq!(lines.len(), 51);
        let scale = width / FULL_WIDTH;
        for line in &lines {
            assert_near(line.x0, wide.x0 + PAD * scale, "shrunk line, left end");
            assert_near(line.x1, wide.x1 - PAD * scale, "shrunk line, right end");
        }
    }

    /// Two ordinary pages, then a page set in one wide column (an appendix, or
    /// a full-width abstract page): 84 characters from the left edge of the
    /// left column to the right edge of the right one.
    fn paper_with_a_wide_page() -> Document {
        let mut wide = page();
        wide.lines
            .extend((BOTTOM..=TOP).rev().step_by(12).map(|y| Line {
                x: LEFT,
                y,
                chars: 84,
            }));
        paper(vec![plain_page(), plain_page(), wide])
    }
}

#[test]
fn a_one_column_document_is_refused_rather_than_cut_down_the_middle() {
    let lines = (BOTTOM..=TOP)
        .rev()
        .step_by(12)
        .map(|y| Line {
            x: LEFT,
            y,
            chars: 84,
        })
        .collect();
    let mut doc = paper(vec![Page {
        lines,
        figures: Vec::new(),
    }]);

    let result = reflow(&mut doc, Options::default());

    assert!(matches!(
        result,
        Err(Error::Layout(LayoutError::NotTwoColumn))
    ));
}

/// Runs on the development stand-in paper when `PDFREFLOW_SAMPLE` points at
/// it; the paper itself is never in the repository.
#[test]
fn the_stand_in_paper_splits_into_columns() {
    let Ok(path) = std::env::var("PDFREFLOW_SAMPLE") else {
        eprintln!("PDFREFLOW_SAMPLE is not set: skipping the stand-in paper");
        return;
    };
    let mut doc = Document::load(&path).expect("the stand-in paper should load");
    let source: Vec<Rect> = doc
        .get_pages()
        .into_values()
        .map(|id| cut::page_box(&doc, id))
        .collect();

    let report = reflow(&mut doc, Options::default()).expect("the stand-in paper should reflow");

    // Every page of a two-column article yields at least its columns, and
    // every page of the result is one column wide.
    assert!(report.output_pages > source.len());
    let widths: Vec<f64> = doc
        .get_pages()
        .into_values()
        .map(|id| cut::page_box(&doc, id).width())
        .collect();
    for width in &widths {
        assert!(
            (width - widths[0]).abs() < 0.1,
            "pages differ in width: {width} and {}",
            widths[0]
        );
        assert!(
            *width < 0.5 * source[0].width(),
            "a page is wider than half the source page"
        );
        assert!(
            *width > 0.3 * source[0].width(),
            "a page is implausibly narrow"
        );
    }
}
