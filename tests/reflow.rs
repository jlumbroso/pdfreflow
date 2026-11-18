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
use pdfreflow::{Error, cut, reflow};

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

/// Reflows a document, writes it to a PDF in memory, reads that PDF back, and
/// returns the visible rectangle of each page of the result, in order.
fn reflowed_pages(mut doc: Document) -> Vec<Rect> {
    reflow(&mut doc).expect("the paper should reflow");
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("the result should save");
    let written = Document::load_mem(&bytes).expect("the result should be a readable PDF");
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
        //
        // Two ordinary pages, then a page set in one wide column (an appendix,
        // or a full-width abstract page): 84 characters from the left edge of
        // the left column to the right edge of the right one.
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

        let pages = reflowed_pages(paper(vec![plain, plain_again, wide]));

        // The two ordinary pages are cut; the wide page stays in one piece.
        assert_eq!(pages.len(), 5);
        assert_near(pages[4].x0, LEFT - PAD, "wide page, left edge");
        assert_near(
            pages[4].x1,
            RIGHT + COLUMN_WIDTH + PAD,
            "wide page, right edge",
        );
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
