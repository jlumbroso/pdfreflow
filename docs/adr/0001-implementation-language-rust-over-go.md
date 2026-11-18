<!-- adr template version: "adr 3.12.0" -->
<!--
adr teaching map: the example is not the teaching — these are the up-links from this file to what teaches its form; load them rather than winging it. (Sibling: the template-version marker dates the mold this file was minted from.)
  skill: adr-authoring
  mcp: adrs4ai
  plugin: adr@adrs4ai
-->

# Implementation language: Rust over Go

- **Date**: 2026-11-18 (UTC)
- **Iteration**: 1
- **Status**: Accepted
- **Deciders**: Jérémie Lumbroso (delegated the choice), Galley 5.2 (made it)

**TL;DR**: pdfreflow is a Rust command-line tool (`lopdf` for PDF structure, `clap` for arguments); Go was the only other candidate and loses on PDF-library fit and on the path to running in a browser on the phone itself.

---

## Originating Context

**Source**: Human dump, in conversation at the seat's founding (dictated; verbatim excerpt):

> I think that we want to do it extremely efficiently if possible in Rust or something that's very, very, very fast. so that it's a command line tool. Or if you think it's better, go. You choose between Rust or Go.

**Agency Grant**: "You choose between Rust or Go." The choice is the seat's; this record exists so the choice can be checked.

---

## Explicitation

**What I understand**:

1. The tool is a single command-line binary; speed matters to the human ("very, very, very fast").
2. The candidates are exactly Rust and Go. The human's own lean is Rust ("if possible in Rust").
3. The human reviews papers they cannot share. So the tool must work entirely on the local machine: no network calls, no telemetry, nothing uploaded. That rules out any design that leans on a hosted service, in either language.

**Assumptions**:

- The work is dominated by reading and rewriting PDF structure, plus (possibly) rendering pages at low resolution to find the columns. See ADR-0002 for the pipeline.
- A later version may want to run on the phone itself. Not asked for; it only matters here as a tie-breaker.

**Confirm**: the human, by reading the Decision section below. No answer is needed unless the choice looks wrong.

---

## Questions

### QST-LICENSE: Which license should pdfreflow carry?
- Status: answered
- Why asking: the repository was born from the collaboration template and still holds the template's own `LICENSE` (Apache-2.0) and `NOTICE` (which names the template and its crews, not pdfreflow). Neither was chosen for this project. The answer also bounds which PDF libraries may be used: MuPDF's bindings are AGPL and are excluded under every option except a copyleft override. Until this is answered, `Cargo.toml` carries no `license` field and `publish = false`, and the two inherited files stay untouched.
- Need: pick a letter (or override — any shape answers)

Options:
- **A — dual MIT/Apache**: `MIT OR Apache-2.0`, the Rust ecosystem's usual pair. Replace `LICENSE` with `LICENSE-MIT` + `LICENSE-APACHE`, delete the template's `NOTICE`. Choose this if the tool may be published on crates.io and should be as easy as possible for others to reuse.
- **B — Apache only**: keep the inherited `LICENSE` text as is (it is the stock Apache-2.0 text), rewrite `NOTICE` to name pdfreflow and its author. Choose this to stay aligned with the license the template and its sibling tools use.
- **C — MIT only**: one short license file, delete `NOTICE`. Choose this for the least text and no patent clause.
- **D — none for now**: delete `LICENSE` and `NOTICE`; all rights reserved until decided. The GitHub repository is public today, so this means code that anyone can read and nobody may reuse. Choose this if it is too early to decide.

**Recommendation**: (by Galley 5.2, claude-fable-5-2, 2026-11-18)

**A — dual MIT/Apache.** *Rationale*: the two dependencies this ADR names are already under these terms (`lopdf` is MIT; `clap` is MIT OR Apache-2.0, per their crates.io listings), so nothing upstream constrains the choice, and the dual pair is what a Rust user expects to find. *Confidence*: 0.55 — because this is a preference about how the human wants their work reused, and I have no evidence of that preference beyond the template's own Apache-2.0 posture, which points at B. *If wrong*: if the human wants pdfreflow aligned with the rest of their tools' licensing, this flips to B; if the human would rather decide later, to D.

*Update, same day (by Galley 5.2, claude-fable-5-2, 2026-11-18)*: the human ruled in conversation that "the license of the template should have no bearing on the product itself". That removes the only evidence that pointed at B, so B now stands on its own merits (one license, patent grant, no GPLv2 compatibility) and not on alignment. The pick stays **A — dual MIT/Apache**; confidence rises to 0.75 — because the remaining uncertainty is only taste between A and C, which differ little for a tool this small. *If wrong*: if the human prefers a single short license file, this flips to C.

**ANS:** (by Jérémie Lumbroso)
Thanks for the clarification, both in-chat, and for recording it here. Thanks for updating your confidence level and documenting it. This helps me approve Option A, as recommended, because it seems to be standard in the Rust ecosystem (not because I hold a specific prior for either/both license).

---

## Decision

### Context

One binary that reads a two-column PDF and writes a single-column one, fast, on the human's own machine. The language decides which PDF libraries are within reach, and those libraries are most of the program.

### Options

**Option A**: Rust
- Pros: `lopdf` (0.47.0) is a pure-Rust library giving object-level access to page dictionaries, page boxes and content streams, which is exactly the level the "cut" step works at. If column detection needs pixels, `hayro` (0.9.0, a pure-Rust rasterizer) and `pdfium-render` (0.9.5) exist. Compiles to a single static binary with no runtime. Compiles to WebAssembly with small output, which keeps open a version that runs in the phone's browser with nothing uploaded. It is the human's stated lean.
- Cons: slower to compile and more verbose to write than Go. Anyone contributing needs Rust.

**Option B**: Go
- Pros: faster to write and to build. `pdfcpu` is a capable pure-Go PDF processor that already knows how to crop pages. Single static binary as well.
- Cons: `pdfcpu` is shaped as a toolkit of whole-file operations; walking a content stream operator by operator (which text-based column detection needs) is not what its public API is built around. Rendering in Go means cgo bindings to MuPDF (AGPL) or to PDFium. Go's WebAssembly output is several times larger.

### Chosen: Option A — Rust

**Rationale**: the two languages are close on raw speed for this workload, so speed does not decide it. Library fit does: the cut step and the likely detection step both want operator-level access to the PDF, which `lopdf` exposes directly. The WebAssembly path and the human's own lean both point the same way.

**What was checked, and what was not**: `cargo 1.92.0` and `go 1.25.5` are both installed on this machine. The four crate versions above were read from `cargo search` on 2026-11-18. No crate has been compiled or run against a real paper yet. The statement about `pdfcpu`'s API shape is from the seat's prior knowledge, not from reading its current source.

**If wrong**: if `lopdf` cannot load and re-save the stand-in paper (and the kinds of PDFs conference systems produce) without damage, and `pdfcpu` can, this decision flips to Go. The first implementation step tests exactly that.

**Trade-offs accepted**: longer builds; a steeper language for future contributors.

### Consequences
- The crate is a binary crate at the repository root (`Cargo.toml`, `src/`, `tests/`), edition 2024.
- The verification gate (`just gate`) runs `cargo fmt --check`, `cargo clippy` with warnings as errors, and `cargo test`.
- AGPL-licensed PDF libraries (MuPDF) are out unless QST-LICENSE is answered with a copyleft override.

---

## Action Items

- [x] Scaffold the crate and wire it into `just gate` - Owner: Galley 5.2
- [ ] Load and re-save the stand-in paper with `lopdf`; compare page count and visual output - Owner: Galley 5.2
- [ ] Apply the QST-LICENSE answer to `Cargo.toml`, `LICENSE`, `NOTICE` - Owner: Galley 5.2

---

## Iterations

### Iteration 1 (2026-11-18)
- Trigger: the human gave the mission in conversation and delegated the language choice.
- Contributors: Jérémie Lumbroso, Galley 5.2
- Changes: initial record; choice made; license question opened.
- Outcome: `Accepted` (the delegated choice is made; QST-LICENSE stays open and does not block work).

---

## Links

- Related ADRs: [ADR-0002](0002-mvp-pipeline-detect-the-two-columns-and-re-emit-them-as-pages.md) — the pipeline this language choice serves
- Related code: `Cargo.toml`, `src/main.rs`
