<!-- adr template version: "adr 3.12.0" -->
<!--
adr teaching map: the example is not the teaching — these are the up-links from this file to what teaches its form; load them rather than winging it. (Sibling: the template-version marker dates the mold this file was minted from.)
  skill: adr-authoring
  mcp: adrs4ai
  plugin: adr@adrs4ai
-->

# MVP pipeline: detect the two columns and re-emit them as pages

- **Date**: 2026-11-18 (UTC)
- **Iteration**: 2
- **Status**: Implemented
- **Deciders**: Jérémie Lumbroso, Galley 5.2

**TL;DR**: For each page, find the gap between the two columns, then write two output pages that reuse the original page's content under a narrower page box — nothing is re-rendered, text stays text; the gap is found once per document from where text sits on the typical page, and each column is cropped tightly, leaving out headers, footers and page numbers.

---

## Originating Context

**Source**: Human dump, in conversation at the seat's founding (dictated; verbatim excerpts):

> at first we just need an MVP and we don't need to deal with like figures that span the whole [page], but the mission is to take a PDF that is two columns, detect where the two columns are, cut them out and create like a PDF that's like [...] the columns one after the other to make it easier to read PDFs on the phone.

> I am needing this in the context of reviewing papers that I cannot share with you, so this PDF can work as a stand-in for a 2 column typical article

**Agency Grant**: "the mission is for you to design something". Design is the seat's; the two questions below are the parts that are about how the human will read, which only the human can say.

---

## Explicitation

**What I understand**:

1. **Input**: a PDF typeset in two columns (a conference or journal paper). **Output**: a PDF in which each column is its own narrow piece, in reading order — left column, then right column, page after page.
2. **"Cut them out"**: I read this as cropping, not re-typesetting. The words are not re-flowed into a new line length; each column keeps its original line breaks and simply fills the phone's width.
3. **Out of scope for the MVP**, by the human's word: figures that span both columns.
4. **Confidentiality**: the real inputs are papers under review. The tool runs entirely on the local machine and never opens a network connection. Real papers are never committed to this repository (it is public on GitHub), never pasted into records, and never shown to the seat.
5. **The stand-in paper** (`~/Downloads/3724363.3729109.pdf`) is a published ACM article: 7 letter-size pages, made by pdfTeX with the `acmart` class. It is the development sample. It is copyrighted, so it also stays out of the repository; tests that need it read its path from an environment variable and are skipped when it is absent.

**Assumptions**:

- Pages have exactly two columns of equal width. One-column and three-column pages are passed through unchanged for now.
- Pages are upright (no `/Rotate`) in the first version.
- The human reads in an ordinary phone PDF viewer that can fit a page to the screen width.

**Confirm**: the human — especially point 2. If "cut them out" was meant as true re-flow (new line breaks at a phone-sized line length), this whole record is the wrong design.

---

## Questions

### QST-OUTPUT-SHAPE: What shape should the output pages have?
- Status: answered
- Why asking: "the columns one after the other" can be laid out three ways, and they differ in how scrolling feels on the phone and in how much machinery the tool needs. A two-column letter page holds two columns each about 3.3 inches wide and 9 inches tall.
- Need: pick a letter (or override — any shape answers)

Options:
- **A — one column per page**: each source page becomes two output pages, left column then right, each cropped to its column. A 7-page paper becomes 14 tall narrow pages. Works in every viewer; there is a page break between the two columns of one source page. Needs only changes to the page list and page boxes.
- **B — one tall page per source page**: the left column sits above the right column on a single output page twice as tall. No page break mid-source-page, so page numbers still match the paper's. Needs each column wrapped as a reusable drawing and placed twice, which is more code and more ways to go wrong.
- **C — one continuous strip**: the whole paper as a single very tall page. The smoothest scroll, but some viewers refuse or stutter on pages that tall, and "go to page 5" stops meaning anything.

**Recommendation**: (by Galley 5.2, claude-fable-5-2, 2026-11-18)

**A — one column per page.** *Rationale*: it is the only option that touches nothing but the page list and page boxes, so the first version cannot damage content, and phone viewers in continuous-scroll mode already hide most of the page break. B and C both build on A's column rectangles, so starting at A wastes nothing. *Confidence*: 0.75 — because the reasoning is from how PDF page boxes work, not from trying the result in the human's own reader. *If wrong*: if the break between a page's left and right column is annoying in the reader the human actually uses, or if matching the paper's page numbers matters for writing the review, this flips to B.

**ANS:** (by Jérémie Lumbroso)
Yes, Option A, and ideally, tightly cropped. This will mean ignoring some small amount of noise (header/footer/page number/etc.).

---

### QST-FULL-WIDTH: What should the first version do with content that spans both columns?
- Status: answered
- Why asking: the human ruled spanning figures out of the MVP. But one spanning element is on the first page of nearly every paper: the title and author block. A straight cut down the middle slices it in half. The same mechanism that would fix the title also fixes spanning figures and tables, so this is a question of when, not whether.
- Need: pick a letter (or override — any shape answers)

Options:
- **A — cut straight through**: every page is cut into exactly two crops, top to bottom. On page 1 the title and author names are split across the two crops; the abstract and body are unaffected in the common layouts where the abstract sits inside a column. Spanning figures are split the same way. Smallest first version.
- **B — first-page top band only**: on page 1, find where the two-column body starts; emit everything above it as its own full-width page, then the two columns. All other pages are cut as in A.
- **C — bands on every page**: split every page into horizontal bands, each either full-width or two-column, and emit them in reading order. Handles the title, spanning figures and tables, and footers. This is the real design for after the MVP; it needs the most detection work.

**Recommendation**: (by Galley 5.2, claude-fable-5-2, 2026-11-18)

**A — cut straight through**, with C as the planned next step. *Rationale*: it matches the scope the human set ("we don't need to deal with figures that span"), and for a reviewer the split title costs little. B is a special case of C, so building B first means building the same detector twice. *Confidence*: 0.7 — because I have not yet seen the stand-in paper's page 1 after a straight cut. *If wrong*: if the straight cut makes page 1 unusable on the stand-in paper, or the title block throws off column detection on page 1, this flips to B.

**ANS:** (by Jérémie Lumbroso)
Definitely Option A. The first part of this is figuring out how to ignore this "noise" and cut through it rather than being overly cautious and not seeing the two columns.

---

## Decision

### Context

Two steps: **detect** (where is the gap between the columns?) and **cut** (write the pieces). The cut step is decided here. The detect step has a leading candidate and an experiment that settles it — see QST-DETECTION under Open Follow-ups.

### The cut step

**Option A**: crop by page box
- Each output page is a copy of the source page's entry in the page list, pointing at the *same* content and resources, with its page box (`MediaBox` / `CropBox`) narrowed to one column.
- Pros: lossless — fonts, vector figures and selectable text are untouched. Fast — no content is rewritten. The file grows by a few hundred bytes per page.
- Cons: the other column is hidden, not removed, so the file is no smaller, and a viewer's text search may still find words from the hidden half of a page. Links and other annotations belong to one page each and must be copied and filtered per crop.

**Option B**: render and re-encode
- Rasterize each column to an image and build a new PDF from the images.
- Pros: simplest possible output; works identically on every input.
- Cons: text becomes pixels — no selection, no search, blurry when zoomed, much larger files, far slower.

### Chosen: Option A — crop by page box

**Rationale**: reading on a phone means zooming, and only vector text stays sharp. Option A is also the fast one, which is the human's stated priority.

**Trade-offs accepted**: hidden content stays in the file. For the first version, annotations (links) are dropped from the output pages; restoring the ones that fall inside each crop is a follow-up.

### Consequences
- The tool never needs to understand fonts or drawing to *write* its output — only to *find* the gap.
- Page properties that a page inherits from its parent in the page tree (`MediaBox`, `Resources`, `Rotate`) must be resolved before a page is copied.
- ~~A page where no two-column gap is found is passed through unchanged.~~ *Corrected in Iteration 2*: a single-column page inside a two-column paper is kept in one piece (cropped to the text block); a document with no two-column layout at all is refused with a message.

### The detect step (added in Iteration 2, after the human's answers)

The human's answers set two requirements beyond the options: crops should be **tight**, and detection should **cut through noise** (headers, footers, page numbers, the title) "rather than being overly cautious and not seeing the two columns". Both shaped what was built (`src/ink.rs`, `src/layout.rs`):

1. **Where the ink is.** Each page's drawing instructions are read to get the bounding box of every run of text (with real glyph widths from the font), every image and every painted line or shape. Nothing is rendered.
2. **The columns, once for the whole document.** For each horizontal position, take how much text sits above it on the *typical* page (the median over pages). The gap is the widest empty strip near the middle of the text; the columns are what lies on either side. Because it is the typical page that counts, a title crossing the gap on page 1 or a page number sitting in the gap does not hide it.
3. **The text block.** The vertical range that holds ink on some page, minus running headers and footers (thin strips at the very top or bottom with white space next to them on every page).
4. **The crops.** Each column of each page is cropped to the column's width and to the height of the ink actually present, plus 4 points of white space. An empty column gives no page. A half-empty column gives a short page.

**What was measured** (2026-11-18, release build, Apple silicon laptop):

| Paper | Layout | Result | Time |
|---|---|---|---|
| The stand-in paper (ACM `acmart`, 7 pages) | two columns, gap at x 295–318 | 14 pages | 0.01 s |
| arXiv 1512.03385 (CVPR style, 12 pages) | two columns, gap at x 287–309 | 24 pages | 0.02 s |
| arXiv 1810.04805 (ACL style, 16 pages) | two columns, gap at x 291–307 | 32 pages | 0.02 s |
| arXiv 1706.03762 (NeurIPS style) | one column | refused: "no gap between two columns found" | — |
| arXiv 2010.11929 (ICLR style) | one column | refused, same message | — |

Rendered pages of the three two-column outputs were looked at: columns are whole, headers and page numbers are gone, a figure at the top of a column is kept, and the title and spanning figures are cut down the middle as agreed. All five inputs were made by pdfTeX; no paper made by Word or another producer has been tried.

**A design error caught on the way**: the first version found the columns by *adding up* ink over all pages. A test for a real case — a single-column page inside a two-column paper — failed: with one page in three single-column, the sum filled the gap and the whole paper was refused. Taking the median over pages fixed it. The test stays as a regression test (`tests/reflow.rs`, `regression_tests`), and the story is told in `docs/vignettes/2026-11-18-the-appendix-page-that-filled-the-gap.md`.

---

## Open Follow-ups

### QST-DETECTION: How is the gap between the columns found?
- Status: answered
- Why deferring: the two candidates can only be told apart by running them on a real file, which has not happened yet.
- Need: nothing from the human unless they want to override

Options:
- **A — text positions**: read each page's drawing instructions, note the horizontal position where every run of text starts, and look near the middle of the page for the widest stretch where no text starts. Its right edge is where the right column begins; the leftmost text start is where the left column begins. No rendering, no extra dependency, the fastest. Blind on scanned pages (no text to read) and unaware of figures.
- **B — pixels**: render each page small and grey with a pure-Rust rasterizer (`hayro`), add up the ink in each pixel column, and take the widest empty valley near the middle. Works on anything, scans included, and sees figures. Costs a render per page and a larger dependency.
- **C — text first, pixels as fallback**: A on every page; B only on pages where A finds no text or no clear gap.

**Recommendation**: (by Galley 5.2, claude-fable-5-2, 2026-11-18)

**A — text positions**, growing into C if needed. *Rationale*: it needs only `lopdf`, which the cut step already uses, and it needs no font metrics: taking the distance between the two columns' left edges as the width of both crops gives each crop its column plus one gap-width of white space. *Confidence*: 0.6 — because nothing has been run; I am reasoning from how pdfTeX writes text, and other PDF producers position text differently. *If wrong*: if text start positions on the stand-in paper's body pages do not show one clean gap near the page centre, this flips to B.

**ANS:** (by Jérémie Lumbroso)
Yes, as recommended Option A growing into Option C.

---

- [x] Unverified assumption: `lopdf` loads and re-saves the stand-in paper without damage (ADR-0001's falsifier). *Verified in Iteration 2 on three papers; `qpdf --check` reports no errors on any output.*
- [ ] Future task: keep link annotations that fall inside a crop.
- [ ] Future task: rotated pages (currently passed through untouched); pages with three columns.
- [x] Future task: trimming the white space left in each crop. *Done in Iteration 2 — crops are tight, at the human's word.*
- [x] Concern: page headers and footers span both columns and will be split by a straight cut. *Resolved in Iteration 2: running headers, footers and page numbers are left out of the crops altogether.*
- [ ] Future task (QST-FULL-WIDTH option C): split pages into full-width and two-column bands, so titles and spanning figures stay whole.
- [ ] Future task (QST-DETECTION option C): the pixel fallback, for scanned papers. Today they are refused with "no text found".
- [ ] Concern: a header or footer is recognised as a strip no taller than a line or two, at the very top or bottom, with white space under or over it on every page. On a one-page document there is only one page to compare, so a short first or last line standing apart from the rest could be mistaken for one and left out.
- [ ] Concern: the columns are assumed to sit at the same place on every page. A paper whose odd and even pages have different margins would blur the detection.
- [ ] Unverified assumption: glyph widths are read from the font's own width table, which covers the Latin-script papers tried so far. Fonts for Chinese, Japanese or Korean text take a different path in the code that no test or sample has exercised.
- [ ] Future task: the output is written without compressed object tables, so it can be slightly larger than it needs to be (the stand-in paper came out 2% *smaller* than its input, so this has not mattered yet).

---

## Action Items

- [x] Run the QST-DETECTION experiment on the stand-in paper - Owner: Galley 5.2. *Run through the tool's own `--explain` flag instead of a throwaway script, so the experiment stays repeatable on any paper.*
- [x] Implement the cut step and a first end-to-end run - Owner: Galley 5.2
- [x] Build a small synthetic two-column PDF that can live in `tests/` (the stand-in paper cannot) - Owner: Galley 5.2. *Built in code by `tests/reflow.rs`; no PDF file is stored.*

---

## Iterations

### Iteration 1 (2026-11-18)
- Trigger: the human gave the mission in conversation, then offered a stand-in paper and the confidentiality constraint.
- Contributors: Jérémie Lumbroso, Galley 5.2
- Changes: initial record; cut step decided; two questions opened for the human, one held by the seat.
- Outcome: `Draft`

### Iteration 2 (2026-11-18)
- Trigger: the human answered all four questions (commit `2de1a87`) and said "Go for it!".
- Contributors: Jérémie Lumbroso, Galley 5.2
- Changes: QST-OUTPUT-SHAPE answered A, with tight crops and headers/footers/page numbers left out; QST-FULL-WIDTH answered A, with the instruction to cut through noise; QST-DETECTION answered A growing into C. The MVP was built to those answers and run on five papers; "The detect step" section records the design as built, the measurements, and a design error caught by a test. Follow-ups and action items updated.
- Outcome: `Draft → Implemented` (the MVP as decided here exists and passes its gate; the unchecked follow-ups are later work, not part of this decision).

---

## Glossary

- **Gutter / gap**: the vertical strip of white space between the two columns.
- **Page box**: the rectangle a PDF page declares as its visible area (`MediaBox`, optionally narrowed by `CropBox`).
- **Crop**: one output page — a source page shown through a page box narrowed to a single column.
- **Stand-in paper**: the published article the human supplied for development, in place of papers under review.

---

## Links

- Related ADRs: [ADR-0001](0001-implementation-language-rust-over-go.md) — Rust, `lopdf`
- Related code: `src/main.rs`
