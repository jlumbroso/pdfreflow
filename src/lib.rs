//! pdfreflow — turn a two-column PDF into single-column pages for reading on a phone.
//!
//! Three steps, one module each:
//!
//! 1. [`ink`] reads each page's drawing instructions and reports where the ink is.
//! 2. [`layout`] finds the two columns and the text block from all pages
//!    together, and decides what to cut out of each page.
//! 3. [`cut`] rewrites the page list so that each cut-out is its own page.
//!
//! Nothing is rendered and nothing leaves this machine.
//! Design record: `docs/adr/0002-mvp-pipeline-detect-the-two-columns-and-re-emit-them-as-pages.md`.

pub mod cut;
pub mod geom;
pub mod ink;
pub mod layout;

use std::fmt;
use std::path::Path;

use lopdf::{Document, ObjectId};

use layout::{Layout, LayoutError, PageInk, Piece};

#[derive(Debug)]
pub enum Error {
    /// The file could not be read, understood or written as a PDF.
    Pdf(lopdf::Error),
    /// The result could not be written.
    Io(std::io::Error),
    /// The file is password-protected.
    Encrypted,
    /// The pages do not look like a two-column document.
    Layout(LayoutError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Pdf(error) => write!(f, "{error}"),
            Error::Io(error) => write!(f, "{error}"),
            Error::Encrypted => write!(f, "the PDF is password-protected"),
            Error::Layout(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<lopdf::Error> for Error {
    fn from(error: lopdf::Error) -> Error {
        Error::Pdf(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::Io(error)
    }
}

impl From<LayoutError> for Error {
    fn from(error: LayoutError) -> Error {
        Error::Layout(error)
    }
}

/// What was found and what was done. Holds only geometry, never content, so it
/// is safe to print for a confidential document.
#[derive(Debug)]
pub struct Report {
    pub source_pages: usize,
    pub output_pages: usize,
    pub layout: Layout,
    /// For each source page, in order, the pieces that replace it.
    pub plans: Vec<Vec<Piece>>,
}

/// Rewrites `doc` in place so that each column is its own page.
pub fn reflow(doc: &mut Document) -> Result<Report, Error> {
    let page_ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
    let pages: Vec<PageInk> = page_ids
        .iter()
        .map(|&id| PageInk {
            media: cut::page_box(doc, id),
            rotation: cut::page_rotation(doc, id).rem_euclid(360),
            boxes: ink::page_ink(doc, id),
        })
        .collect();
    let layout = layout::detect(&pages)?;
    let plans = layout::plan(&pages, &layout);
    let output_pages = cut::apply(doc, &page_ids, &plans)?;
    doc.prune_objects();
    Ok(Report {
        source_pages: page_ids.len(),
        output_pages,
        layout,
        plans,
    })
}

/// Reads `input`, reflows it and writes the result to `output`.
pub fn reflow_file(input: &Path, output: &Path) -> Result<Report, Error> {
    let mut doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(Error::Encrypted);
    }
    let report = reflow(&mut doc)?;
    doc.save(output)?;
    Ok(report)
}
