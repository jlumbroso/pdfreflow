# pdfreflow

Turn a two-column PDF into single-column pages that are easy to read on a phone.

Research papers are typeset in two narrow columns on a large page. On a phone
that means zooming in and panning left, down, up, right, down, for every page.
pdfreflow finds the two columns on each page and writes a new PDF with the
columns one after the other, so reading is one continuous scroll.

It runs entirely on your machine. The input is never uploaded anywhere.

## Status

**Not usable yet.** As of 2026-11-18 the repository holds the design and a
command-line skeleton; the tool does not yet produce output.

The plan for the first version:

- find the gap between the two columns on each page;
- emit each column as its own page, by narrowing the page's visible box —
  nothing is re-rendered, so text stays sharp and selectable;
- leave content that spans both columns (titles, wide figures) for later.

## Usage (planned)

```sh
pdfreflow paper.pdf                 # writes paper.reflow.pdf
pdfreflow paper.pdf -o phone.pdf
```

## Building

Requires a recent Rust toolchain (edition 2024).

```sh
cargo build --release
just gate        # formatting, lints, tests, and the repository's own checks
```

## How this project is run

Decisions and open questions live in [`docs/adr/`](docs/adr/), next to the code:

- [ADR-0001](docs/adr/0001-implementation-language-rust-over-go.md) — why Rust
- [ADR-0002](docs/adr/0002-mvp-pipeline-detect-the-two-columns-and-re-emit-them-as-pages.md) — the first version's design

The repository was started from the
[ADRs4AI human-AI collaboration template](https://github.com/ADRs4AI/human-ai-collaboration-template-A);
[`CLAUDE.md`](CLAUDE.md) and [`docs/METHODOLOGY.md`](docs/METHODOLOGY.md) describe how
the human and AI collaborators work together here.
