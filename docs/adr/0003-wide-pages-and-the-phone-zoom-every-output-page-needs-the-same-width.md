<!-- adr template version: "adr 3.12.0" -->
<!--
adr teaching map: the example is not the teaching — these are the up-links from this file to what teaches its form; load them rather than winging it. (Sibling: the template-version marker dates the mold this file was minted from.)
  skill: adr-authoring
  mcp: adrs4ai
  plugin: adr@adrs4ai
-->

# Wide pages and the phone zoom: every output page needs the same width

- **Date**: 2026-11-18 (UTC)
- **Iteration**: 3
- **Status**: Partially Implemented
- **Deciders**: Jérémie Lumbroso, Galley 5.2

**TL;DR**: One page kept at full width makes a phone reader zoom every page out to fit it, undoing the reflow; the output must have one page width throughout, and the open question is what to do with content that is wider than a column.

---

## Originating Context

**Source**: Human observation from first real use (dogfooding), in conversation, verbatim:

> The page that wasn't cut was twice as wide as the others. Because the PDF is displayed as a continuous succession of pages, my PDF reader allocated the width as a function of the max of the widths. As a result almost all pages were Zoomed out. It resulted in almost the same experience as not reflowing the paper: As the viewport was constrained by that page that wasn't split, or at least its width.

The human ran the tool on a manuscript under review and shared what `--explain` printed (coordinates only). The manuscript is typeset with the ACM class in review mode by pdfTeX. Nothing else about it is recorded here.

**Agency Grant**: "The choice is yours of what we do next." The human also has a solution in mind and, at the seat's request, has not said it yet, so that the two proposals are independent. The human's answer to QST-WIDE-PAGES is where theirs goes.

---

## Explicitation

**What I understand**:

1. **What happened.** Column crops in that run are 275 and 277 points wide. One page — full-width figures stacked one above the other — was kept in one piece, 567 points wide. The tool did this on purpose: ADR-0002's check for "a single-column page inside a two-column paper" fired, because text crosses the gap on that page about as densely as it fills the columns.
2. **Why it hurts.** A phone reader in continuous-scroll mode sizes its view to the widest page. One page at 567 points halves the zoom of every 275-point page. The result is close to the paper before reflowing.
3. **The rule this implies.** The output is only as good as its widest page. So: **every output page has the same width.** This holds for anything the tool will ever emit, including the full-width bands planned as ADR-0002's next step.
4. **Seen in the same run.** The columns came out as x 28–295 and 318–587, against 54–295 and 318–559 for the published stand-in. The extra 26 to 28 points on the outer side of each column are the review-mode line numbers. They were kept deliberately (they are dense enough to count as part of the column). That is QST-LINE-NUMBERS.

**Assumptions**:

- The reader behaviour described (view sized to the widest page) is common to phone PDF readers in continuous mode, not particular to one app. Not verified beyond the human's reader.

**Confirm**: the human, on point 3: is "one width throughout" the right rule, or is there a case where a wider page is wanted?

---

## Questions

### QST-WIDE-PAGES: What should the tool do with a page that is wider than a column?
- Status: answered
- Why asking: today a page whose text runs across the gap (a single-column page, or a page of full-width figures) is kept whole at full width, and that one page makes a phone reader zoom every other page out. The output needs one page width throughout. The options differ in what the wide content looks like once it is forced to column width. The seat has not heard the human's own idea; if it is none of these, it is the answer.
- Need: pick a letter (or override — any shape answers)

Options:
- **A — shrink to fit**: draw the whole wide page scaled down to the column width (about half size for a typical paper). Nothing is cut and the reading order is untouched; the figures and any text on that page are small, and the reader pinches to zoom on that page only. That page looks the way the whole paper looked before reflowing. Small change: the page's drawing is wrapped in one scaling instruction, still nothing is rendered.
- **B — cut it like the others**: never keep a page whole; cut every page down the middle. Everything stays full size and no new machinery is needed, but full-width figures are sliced in two across consecutive pages, and a page of single-column text becomes unreadable (each line broken in half).
- **C — turn it sideways**: rotate the wide page a quarter turn, so its width runs along the scroll direction, then fit it to the column width. Good for content that is wider than it is tall (a wide figure or table shows at full size, read by tilting the phone); no help for a full page, which is taller than wide and would end up smaller than with A.
- **D — split into bands, then shrink or turn each band**: cut the page into horizontal bands (each figure, each block of text), keep two-column bands as columns, and give each full-width band its own page — shrunk to fit, or turned sideways when it is wider than tall and would be larger that way. Best result; needs the band detector that ADR-0002 lists as the next step, so it is the largest piece of work.

**Recommendation**: (by Galley 5.2, claude-fable-5-2, 2026-11-18)

**A — shrink to fit, now; D as the step after.** *Rationale*: A is the smallest change that makes "one width throughout" true for every input, so it fixes the zoom problem the human reported (the 567-point page becomes 275 points wide) without cutting anything. It is also not thrown away by D: D needs exactly A's scaling for each full-width band, plus band detection, plus C's rotation as a per-band choice. B contradicts ADR-0002's own regression test (a single-column page cut in two is unreadable). C alone does nothing for the page in the report, which is 567 wide by 645 tall. *Confidence*: 0.7 — because the zoom diagnosis rests on the human's direct observation and the `--explain` numbers, but nobody has yet looked at half-size figures on a phone. *If wrong*: if figures at half size are too small to read without constant pinching, this flips to D straight away (with rotation for wide bands) and A is only its first commit.

**ANS:** (by Jérémie Lumbroso)
A now, D next, as recommended. In conversation, 2026-11-18 (recorded by Galley 5.2): "Your recommendation is chef's kiss. I had come up with A independently, but your combined concept is better. Endorsed!"

---

### QST-LINE-NUMBERS: Should the crops keep the review-mode line numbers?
- Status: unanswered
- Why asking: manuscripts under review often carry line numbers in the outer margins. In the human's run they were kept, which makes each crop about 27 points (10%) wider than the text itself, so the text is shown about 10% smaller than it could be. Dropping them makes crops tighter; keeping them lets a review cite "line 412".
- Need: pick a letter (or override — any shape answers)

Options:
- **A — keep them**: as today. The crop includes the line numbers beside each column. Text is about 10% smaller on screen than it could be.
- **B — drop them**: crop to the text only. Tighter and larger on screen; the reader can no longer see which line is which.
- **C — a switch**: keep them by default, with a command-line flag to drop them (or the reverse).

**Recommendation**: (by Galley 5.2, claude-fable-5-2, 2026-11-18)

**A — keep them.** *Rationale*: the tool exists for reviewing, and line numbers are how a review points at a passage; the cost is a tenth of the zoom. No extra code. *Confidence*: 0.6 — because this is a guess about how the human writes reviews, not something observed. *If wrong*: if the human does not cite line numbers, or finds the text too small, this flips to C with dropping as the default.

**ANS:** (by Jérémie Lumbroso)
[Fill this in]   <!-- literal placeholder — parser-significant, do not paraphrase -->

---

### QST-SIDEWAYS-DEFAULT: Should wide figures be turned sideways without being asked?
- Status: unanswered
- Why asking: the tool can now show a full-width figure or table two ways. Shrunk, it is upright and about half size (49% on the stand-in paper's spanning screenshot). Turned a quarter turn, it is much larger (144% for the same screenshot) but must be read with the phone held sideways, and with the phone's screen rotation locked, or the reader turns the page back upright and small. Today turning happens only with `--sideways`. Trying one paper both ways on the phone is the experiment that answers this: `pdfreflow paper.pdf -o a.pdf` and `pdfreflow paper.pdf --sideways -o b.pdf`.
- Need: pick a letter (or override — any shape answers)

Options:
- **A — only when asked**: as today. Figures are upright and small unless `--sideways` is given. Nothing surprising in the default output.
- **B — turned by default**: figures that gain at least 25% are turned unless `--upright` is given. Larger figures with no flag to remember; every paper with a wide figure then needs the phone turned at some point.
- **C — never**: remove the option. Choose this if sideways pages turn out to be more nuisance than help.

**Recommendation**: (by Galley 5.2, claude-fable-5-2, 2026-11-18)

**A — only when asked**, until the human has tried it. *Rationale*: the gain in size is measured (49% to 144% on the stand-in paper's figure, `--explain` output), but whether a sideways page is pleasant in a continuous scroll is something only a phone in a hand can show, and no one has looked yet. *Confidence*: 0.5 — because the evidence for the size gain is solid and the evidence about the reading experience is nil. *If wrong*: if the human tries `--sideways` and prefers it, this flips to B; if turned pages are disorienting, to C.

**ANS:** (by Jérémie Lumbroso)
[Fill this in]   <!-- literal placeholder — parser-significant, do not paraphrase -->

---

## Supporting Materials

### DOC: the run that showed the problem

What `--explain` printed, trimmed to what matters (coordinates in points; no content):

```
left column   x 28–295
gap           x 295–318
right column  x 318–587
ordinary page:   [24 69 299 711]  [314 69 591 711]     two crops, 275 and 277 wide
the wide page:   [24 67 591 712]                       one crop, 567 wide
```

### DOC: other producers tried the same day

All earlier test papers were made by pdfTeX, and so was the manuscript in this run. To try other producers without touching anyone's papers, a synthetic two-column article (filler text, a spanning figure, running headers) was compiled with XeTeX and with LuaTeX, both embedding OpenType fonts as two-byte composite fonts — the code path pdfTeX papers never reach. Both reflowed correctly: 4 pages to 8, columns at x 72–301 and 310–540, headers left out, output clean under `qpdf --check`. Papers written in Word, Typst or InDesign are still untried.

---

## Decision

~~Not made yet: waits on QST-WIDE-PAGES.~~ *Decided in Iteration 2.*

### Chosen: one width throughout — shrink to fit now (A), bands next (D)

The human had arrived at A independently and endorsed the combined plan. The rule is now a property of the output: **every page the tool writes is exactly as wide as a column.**

**As built (A)**, in `src/layout.rs` (`plan`, `Piece`) and `src/cut.rs`:

- Both columns are cut to one width (the wider of the two, measured outwards from the gap).
- A page that is not cut into columns — single-column text, a page of full-width figures, a page stored sideways, a blank page — becomes one piece, shrunk to that same width. The page's drawing is wrapped in a single scaling instruction; it is still not rendered, so text stays text.
- `--explain` marks such pieces, for example `[68 160 544 626] shrunk to 50%`.

**Checked**: on a synthetic article with a page of three stacked full-width figures (pdfTeX), all 11 output pages are 238 points wide, where before the figure pages would have been 476. The regression test fails with the fix disabled ("page 5 is 512 wide, the first is 248") and passes with it. Not yet checked on the manuscript that showed the problem: only the human can do that.

**Trade-off accepted**: figures on a shrunk page are about half size. On the synthetic page each figure is 476 wide and about 170 tall; turned sideways on its own page it could be shown at about 140% instead of 50%. That is what D is for.

**As built (D)**, in `src/layout.rs` (`bands`, `full_width`, `figure_blocks`), Iteration 3:

- A two-column page is read from the top down. Wherever ink runs unbroken across the gap — a title, a caption, a wide figure or table — that stretch is a **full-width band** and becomes a page of its own, whole, shrunk to column width. The stretches of two columns above and below it are cut into columns as before. So the title on page 1 and figures that span both columns are no longer cut down the middle (this is also ADR-0002's QST-FULL-WIDTH option C, done).
- A band takes with it the short last line of a caption, and the few lines that sit between two bands (authors' names between a title and the affiliations), so these do not come out as slivers.
- **Sideways, as an option** (`--sideways`): a band that holds a figure or a ruled table, and would show at least 25% larger on its side, is turned a quarter turn and magnified up to 150%. A region of stacked figures is first split into one piece per figure. On the stand-in paper, its spanning screenshot goes from 49% (shrunk) to 144% (turned).
- Turned pages are never wider on screen than a column; a few are narrower, when the 150% limit is reached first. So the rule is now precisely: **no page is wider than a column, and every page that is not turned is exactly that wide.**

**One departure from the option as endorsed, stated plainly**: option D said a band would be "shrunk or turned sideways, whichever shows it larger", automatically. As built, turning happens only when asked for. A turned figure has to be read with the phone held sideways and the screen's rotation locked, or the reader turns it back; the seat has not seen either on a phone and did not want to make every figure depend on a gesture nobody has tried. QST-SIDEWAYS-DEFAULT asks the human to settle it after trying both.

**Checked** (release build, both modes): seven documents — the stand-in, two arXiv two-column papers, three synthetic articles (pdfTeX, XeTeX, LuaTeX), one single-column paper. Without the option every page of every output has one width; with it no page is wider. All outputs pass `qpdf --check`; the single-column paper is still refused. Rendered pages were looked at for the stand-in, both arXiv papers and the stacked-figures article: titles and spanning figures come out whole, columns are unchanged, and no band was found inside ordinary column text. Still not checked on any manuscript the seat cannot see.

**Known rough edges**: a spanning table whose cells happen to leave the gap empty is found only through its rules, and may still be cut through as before; two figures side by side under one shared caption are treated as column content with the caption as a band of its own.

### Consequences
- ADR-0002's sentence "a single-column page … is kept in one piece (cropped to the text block)" still holds, with "and shrunk to column width" added.
- Rotated pages are no longer passed through untouched: they are shrunk too, since any full-width page breaks the zoom.

---

## Action Items

- [x] Implement the answer to QST-WIDE-PAGES, part A (shrink to fit), with a test that every output page of a paper containing a wide page has the same width - Owner: Galley 5.2
- [x] Implement part D: split pages into bands; give each full-width band its own page, shrunk or turned sideways - Owner: Galley 5.2. *Done in Iteration 3; turning is behind `--sideways` pending QST-SIDEWAYS-DEFAULT.*
- [ ] Try one paper with and without `--sideways` on the phone, and answer QST-SIDEWAYS-DEFAULT - Owner: Jérémie Lumbroso
- [x] Make the left and right crops exactly the same width (they differ by 2 points in the run above) - Owner: Galley 5.2
- [ ] Apply the answer to QST-LINE-NUMBERS - Owner: Galley 5.2
- [ ] Try a two-column paper made by Word, Typst or InDesign - Owner: whoever finds one first

---

## Iterations

### Iteration 1 (2026-11-18)
- Trigger: the human's first real use of the MVP, and their report of the zoom problem.
- Contributors: Jérémie Lumbroso, Galley 5.2
- Changes: problem recorded; the one-width rule proposed; two questions opened, each with the seat's recommendation written before hearing the human's own idea.
- Outcome: `Draft`

### Iteration 2 (2026-11-18)
- Trigger: the human answered QST-WIDE-PAGES in conversation: A now, D next; they had reached A independently.
- Contributors: Jérémie Lumbroso, Galley 5.2
- Changes: answer recorded; part A built, tested and described under Decision; left and right crops made the same width. QST-LINE-NUMBERS is still open and line numbers are still kept.
- Outcome: `Draft → Partially Implemented` (A is in; D is not).

### Iteration 3 (2026-11-18)
- Trigger: A committed; D was the endorsed next step.
- Contributors: Galley 5.2
- Changes: part D built and described under Decision: full-width bands get their own pages; `--sideways` turns figures and tables. One departure from the endorsed wording (turning is opt-in, not automatic) is stated there and put to the human as QST-SIDEWAYS-DEFAULT. Seven documents checked in both modes.
- Outcome: `Partially Implemented` (unchanged: both parts are built; two questions, QST-LINE-NUMBERS and QST-SIDEWAYS-DEFAULT, are open and may change defaults).

---

## Links

- Related ADRs: [ADR-0002](0002-mvp-pipeline-detect-the-two-columns-and-re-emit-them-as-pages.md) — the MVP; its single-column-page check is what produced the wide page, and its "bands" follow-up is option D here
- Related code: `src/layout.rs` (`is_single_column`, `plan`), `src/cut.rs`
