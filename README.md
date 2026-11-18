# pdfreflow

Turn a two-column PDF into single-column pages that are easy to read on a phone.

Research papers are typeset in two narrow columns on a large page. On a phone
that means zooming in and panning left, down, up, right, down, for every page.
pdfreflow finds the two columns and writes a new PDF in which each column is
its own page, tightly cropped, in reading order: left column, right column,
next page. Reading becomes one continuous scroll.

It runs entirely on your machine. The input is never uploaded anywhere.

## Usage

```sh
pdfreflow paper.pdf                 # writes paper.reflow.pdf
pdfreflow paper.pdf -o phone.pdf
pdfreflow paper.pdf --sideways      # turn wide figures to show them larger
pdfreflow paper.pdf --explain       # also print the columns and crops it found
```

`--explain` prints coordinates only, never any of the document's text, so its
output is safe to share when the paper itself is not.

## What it does, and does not do

- Each column becomes a page cropped tightly to its text and figures. Running
  headers, footers and page numbers are left out.
- Nothing is re-rendered. Each new page is the original page seen through a
  narrower window, so text stays sharp at any zoom and can still be selected
  and searched. A typical paper takes a few hundredths of a second.
- Anything that spans both columns — the title on the first page, a wide
  figure or table with its caption — is kept whole, on a page of its own, in
  its place in the reading order. So is a page that has no columns at all (an
  appendix set in one wide column, a page of figures).
- Every page of the result has the same width, so a phone reader keeps one
  zoom level from start to finish. Content wider than a column is shrunk to
  fit, to about half size.
- With `--sideways`, wide figures and tables are turned a quarter turn and
  shown large instead of shrunk small. Read them with the phone held sideways,
  with its screen rotation locked.
- A document with no two-column layout at all is refused with a message, not
  cut.
- Links inside the document are dropped for now.
- Scanned documents (pictures of pages, with no text inside) are not supported
  yet.

## Building

Requires a recent Rust toolchain (edition 2024).

```sh
cargo build --release               # the binary is target/release/pdfreflow
just gate                           # formatting, lints, tests, repository checks
```

## How this project is run

Decisions and open questions live in [`docs/adr/`](docs/adr/), next to the code:

- [ADR-0001](docs/adr/0001-implementation-language-rust-over-go.md) — why Rust
- [ADR-0002](docs/adr/0002-mvp-pipeline-detect-the-two-columns-and-re-emit-them-as-pages.md) — how the columns are found and cut

The repository was started from the
[ADRs4AI human-AI collaboration template](https://github.com/ADRs4AI/human-ai-collaboration-template-A);
[`CLAUDE.md`](CLAUDE.md) and [`docs/METHODOLOGY.md`](docs/METHODOLOGY.md) describe how
the human and AI collaborators work together here.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
