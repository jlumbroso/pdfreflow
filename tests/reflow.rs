//! End-to-end tests: build a small two-column "paper", reflow it, write it out,
//! read it back, and check the pages a reader would actually get.
//!
//! The paper is synthetic on purpose. Real inputs are confidential papers under
//! review and the development stand-in is copyrighted, so neither can live in
//! this repository (see CLAUDE.md, "Confidential inputs").

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};

use pdfreflow::geom::Rect;
use pdfreflow::layout::LayoutError;
use pdfreflow::{Error, cut, ink, reflow};

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
// White space the tool leaves around a crop.
const PAD: f64 = 4.0;

struct Line {
    x: f64,
    y: i32,
    chars: usize,
}

/// A column of full-width lines from baseline `from` down to baseline `to`.
fn column(x: f64, from: i32, to: i32) -> Vec<Line> {
    (to..=from)
        .rev()
        .step_by(12)
        .map(|y| Line { x, y, chars: 40 })
        .collect()
}

/// What every page of a paper carries outside its text block: a running header
/// above the left column and a page number centred in the gap.
fn furniture() -> Vec<Line> {
    vec![
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
    ]
}

fn paper(pages: Vec<Vec<Line>>) -> Document {
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
    for lines in &pages {
        let mut operations = Vec::new();
        for line in lines {
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
/// - page 1 has a title that crosses the gap, above two full columns;
/// - page 2 has a full left column and a right column that stops halfway;
/// - page 3 has six lines in the left column and nothing on the right.
fn three_page_paper() -> Document {
    let mut first = furniture();
    first.push(Line {
        x: 150.0,
        y: 700,
        chars: 52,
    });
    first.push(Line {
        x: 150.0,
        y: 688,
        chars: 52,
    });
    first.extend(column(LEFT, 664, BOTTOM));
    first.extend(column(RIGHT, 664, BOTTOM));

    let mut second = furniture();
    second.extend(column(LEFT, TOP, BOTTOM));
    second.extend(column(RIGHT, TOP, 400));

    let mut third = furniture();
    third.extend(column(LEFT, TOP, 640));

    paper(vec![first, second, third])
}

/// Reflows a document, writes it to a PDF in memory, and reads that PDF back:
/// what a reader would open.
fn reflowed(mut doc: Document) -> Document {
    reflow(&mut doc).expect("the paper should reflow");
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("the result should save");
    Document::load_mem(&bytes).expect("the result should be a readable PDF")
}

/// The visible rectangle of each page of a reflowed document, in order.
fn reflowed_pages(doc: Document) -> Vec<Rect> {
    let written = reflowed(doc);
    written
        .get_pages()
        .into_values()
        .map(|id| cut::page_box(&written, id))
        .collect()
}

fn assert_near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 2.0,
        "{what}: expected about {expected}, got {actual}"
    );
}

#[test]
fn each_column_becomes_its_own_page_in_reading_order() {
    let pages = reflowed_pages(three_page_paper());

    // Two columns, two columns, then a page whose right column is empty.
    assert_eq!(pages.len(), 5);

    let is_left = |page: &Rect| {
        assert_near(page.x0, LEFT - PAD, "left crop, left edge");
        assert_near(page.x1, LEFT + COLUMN_WIDTH + PAD, "left crop, right edge");
    };
    let is_right = |page: &Rect| {
        assert_near(page.x0, RIGHT - PAD, "right crop, left edge");
        assert_near(
            page.x1,
            RIGHT + COLUMN_WIDTH + PAD,
            "right crop, right edge",
        );
    };
    is_left(&pages[0]);
    is_right(&pages[1]);
    is_left(&pages[2]);
    is_right(&pages[3]);
    is_left(&pages[4]);
}

#[test]
fn crops_are_tight_and_leave_out_the_header_and_page_number() {
    let pages = reflowed_pages(three_page_paper());
    let top = f64::from(TOP) + ASCENT + PAD;
    let bottom = f64::from(BOTTOM) - DESCENT - PAD;

    // Full columns run from the top of the text block to its bottom. The
    // running header (baseline 750) and the page number (baseline 60) are
    // outside every crop, although the page number sits right in the gap.
    for page in &pages[0..3] {
        assert_near(page.y1, top, "full column, top edge");
        assert_near(page.y0, bottom, "full column, bottom edge");
    }

    // A column that stops halfway down gives a short page, not a tall one
    // that is half empty.
    assert_near(pages[3].y1, top, "half column, top edge");
    assert_near(
        pages[3].y0,
        400.0 - DESCENT - PAD,
        "half column, bottom edge",
    );
    assert_near(
        pages[4].y0,
        640.0 - DESCENT - PAD,
        "six-line column, bottom edge",
    );
}

#[test]
fn the_cut_shares_the_page_content_instead_of_copying_it() {
    let mut doc = three_page_paper();
    let before: Vec<Vec<u8>> = doc
        .get_pages()
        .into_values()
        .map(|id| doc.get_page_content(id))
        .collect();
    reflow(&mut doc).unwrap();
    let after: Vec<Vec<u8>> = doc
        .get_pages()
        .into_values()
        .map(|id| doc.get_page_content(id))
        .collect();

    // Both halves of a source page still draw the whole source page; only the
    // page box differs. This is what keeps text sharp and selectable.
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1], before[0]);
    assert_eq!(after[2], before[1]);
    assert_eq!(after[3], before[1]);
    assert_eq!(after[4], before[2]);
}

#[test]
fn a_sideways_page_comes_out_as_wide_as_a_column() {
    // Papers sometimes store a page turned on its side (a landscape table).
    // Such a page is not searched for columns, but it must still not be wider
    // than the others on screen.
    let mut doc = three_page_paper();
    let last = doc.get_pages().into_values().last().unwrap();
    doc.get_dictionary_mut(last).unwrap().set("Rotate", 90);

    let written = reflowed(doc);
    let ids: Vec<_> = written.get_pages().into_values().collect();
    let column_width = cut::page_box(&written, ids[0]).width();
    let sideways = cut::page_box(&written, *ids.last().unwrap());

    assert_eq!(cut::page_rotation(&written, *ids.last().unwrap()), 90);
    // A page turned a quarter is shown as wide as its box is tall.
    assert_near(
        sideways.height(),
        column_width,
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
        let pages = reflowed_pages(paper_with_a_wide_page());

        // The two ordinary pages are cut; the wide page stays in one piece,
        // which therefore has the proportions of the whole text block and not
        // of a column.
        assert_eq!(pages.len(), 5);
        let block_width = RIGHT + COLUMN_WIDTH - LEFT + 2.0 * PAD;
        let block_height = f64::from(TOP - BOTTOM) + ASCENT + DESCENT + 2.0 * PAD;
        let proportions = pages[4].height() / pages[4].width();
        assert!(
            (proportions - block_height / block_width).abs() < 0.02,
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
        let written = reflowed(paper_with_a_wide_page());
        let ids: Vec<_> = written.get_pages().into_values().collect();
        let pages: Vec<Rect> = ids.iter().map(|&id| cut::page_box(&written, id)).collect();

        let width = pages[0].width();
        for (index, page) in pages.iter().enumerate() {
            assert!(
                (page.width() - width).abs() < 0.1,
                "page {} is {} wide, the first is {width}",
                index + 1,
                page.width()
            );
        }

        // Shrinking must move the text along with the page: all 51 lines of
        // the wide page are drawn inside the shrunk page, edge to edge.
        let wide = pages[4];
        let lines: Vec<Rect> = ink::page_ink(&written, ids[4])
            .into_iter()
            .map(|ink| ink.rect)
            .filter(|rect| rect.y0 >= wide.y0 && rect.y1 <= wide.y1)
            .collect();
        assert_eq!(lines.len(), 51);
        let scale = width / (RIGHT + COLUMN_WIDTH - LEFT + 2.0 * PAD);
        for line in &lines {
            assert_near(line.x0, wide.x0 + PAD * scale, "shrunk line, left end");
            assert_near(line.x1, wide.x1 - PAD * scale, "shrunk line, right end");
        }
    }

    /// Two ordinary pages, then a page set in one wide column (an appendix, or
    /// a full-width abstract page): 84 characters from the left edge of the
    /// left column to the right edge of the right one.
    fn paper_with_a_wide_page() -> Document {
        let mut wide = furniture();
        wide.extend((BOTTOM..=TOP).rev().step_by(12).map(|y| Line {
            x: LEFT,
            y,
            chars: 84,
        }));
        let mut plain = furniture();
        plain.extend(column(LEFT, TOP, BOTTOM));
        plain.extend(column(RIGHT, TOP, BOTTOM));
        let mut plain_again = furniture();
        plain_again.extend(column(LEFT, TOP, BOTTOM));
        plain_again.extend(column(RIGHT, TOP, BOTTOM));
        paper(vec![plain, plain_again, wide])
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
    let mut doc = paper(vec![lines]);

    let result = reflow(&mut doc);

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

    let report = reflow(&mut doc).expect("the stand-in paper should reflow");

    // Every page of a two-column article yields one or two columns.
    assert!(report.output_pages > source.len());
    assert!(report.output_pages <= 2 * source.len());
    let page_width = source[0].width();
    for id in doc.get_pages().into_values() {
        let crop = cut::page_box(&doc, id);
        assert!(
            crop.width() < 0.5 * page_width,
            "a crop is wider than half the page"
        );
        assert!(
            crop.width() > 0.3 * page_width,
            "a crop is implausibly narrow"
        );
    }
}
