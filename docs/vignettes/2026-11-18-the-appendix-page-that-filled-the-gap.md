# The Appendix Page That Filled the Gap

**Date**: 2026-11-18
**Authors**: Galley 5.2 (Claude Fable 5.2), Jérémie Lumbroso
**Topic**: A test written for a real case, not for a suspected bug, overturned the core of a design within minutes of its first successful run.

## The Discovery

The first working version of pdfreflow found the two columns of a paper like this: for every horizontal position on the page, add up how much text sits above it, over all pages. Columns give tall piles; the gap between them gives almost nothing. Jérémie had asked that detection "cut through" noise such as titles and page numbers "rather than being overly cautious and not seeing the two columns", and adding everything up looked like exactly that: a title on page 1 is a pebble next to six pages of column text.

It worked on the stand-in paper on the first run. Seven pages in, fourteen out, the pages looked right.

Then came the tests. The Fourth Directive asks that each test be something a real user would meet, so the list was: columns come out in reading order; crops are tight; a one-column document is refused; and — because real papers have them — a single-column page inside a two-column paper, such as an appendix, is kept in one piece.

That last test was written to exercise a small per-page check. It failed somewhere else entirely:

```
the paper should reflow: Layout(NotTwoColumn)
```

The synthetic paper had two ordinary pages and one wide page. One page in three. The wide page's text ran straight across the gap, and in the *sum* that was enough to lift the gap above the "empty" threshold. The whole paper was refused. The per-page check never ran.

So the design that was supposed to be robust to noise was robust only to *thin* noise. A minority of pages with dense text across the gap defeated it — and a paper with a few single-column appendix pages is not exotic.

## Why This Matters

The tool's real inputs are papers under review, which the AI seat never sees. Whatever the detection gets wrong on those, nobody on the building side will watch it happen; the human would simply get "no gap between two columns found" on a paper that plainly has two columns, on a phone, with a review to write.

The fix was one idea: do not add the pages up, ask what the *typical* page looks like. For each horizontal position, take the median over pages. A minority of unusual pages then cannot move the result at all, however dense they are.

```rust
for (index, bin) in across.bins.iter_mut().enumerate() {
    let mut values: Vec<f64> = per_page.iter().map(|page| page.bins[index]).collect();
    *bin = median(&mut values).unwrap_or(0.0);
}
```

The same geometry came out on the stand-in paper to the point, and the failing test passed.

## The Fourth Directive at Work

The bug was not found by a test aimed at it. Nobody suspected the sum. It was found because the directive pushed the test list toward *situations* ("what happens when a paper has an appendix page?") and away from *mechanisms* ("does `is_single_column` return true?"). A test of the mechanism would have called the per-page check directly, passed, and left the sum in place.

The Third Directive then did its part cheaply: the test that failed already was the regression test. It only needed a comment saying what it had caught.

## Lessons Learned

- A sample that works on the first run proves less than it feels like. The stand-in paper has no single-column pages, so it could never have shown this.
- "Robust to noise" needs the noise named. Thin noise (a page number) and dense minority noise (an appendix page) call for different arithmetic: a sum forgives the first, only a median forgives the second.
- Writing the test for the user's situation costs the same as writing it for the function, and reaches further.

## Metacognitive Insight

The human's instruction — do not be so cautious that you miss the columns — was taken as a reason to aggregate boldly, and the first aggregate that came to mind was a sum. The instruction was about *outcome*; the sum was a guess at *method* that happened to satisfy the one example at hand. What separated the guess from a sound method was not more thought about the method but a second, different example, and the habit that produced the second example was a rule about tests.
