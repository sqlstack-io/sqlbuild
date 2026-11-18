# UI performance rules

The UI must stay responsive on a modest machine: a few cores and an old, low-end graphics card
driving large screens. The layout and performance end-to-end tests run with the CPU slowed 6× and
fail when a page breaks these budgets; keep them passing rather than raising the budgets.

## Budgets (6× slower CPU, realistic data)

- Opening a list page, first or repeat visit: settled within 2 s.
- Graph pan, wheel zoom, trackpad momentum and pinch: 90% of frames ≤ 50 ms, none over 200 ms.
- Typing in a search or selection field: suggestions update without a visible stall.
- A page open while a run goes on (a change about every second): under half the time busy, so it keeps
  up; the run page's timeline scrolls at the graph budget above.
- The plan page on a real-sized plan: switching section, or Show more, has its rows on screen within 1 s.
  A large section loads one page of rows when it is opened, never all of them.

## Rules

1. **No work proportional to options × rows.** Count filter options in one pass (`countBy` in
   `lib/list/count-by.ts`); never `options.map(o => rows.filter(...).length)`. A unit check flags it.
2. **Draw only what is visible.** Lists above ~200 rows use the shared `DataTable` windowing; big
   option lists use the windowed `ChoiceFilter` list; graphs draw at the visible level (closed boxes,
   aggregated edges).
3. **Pan and zoom move a picture; they don't redraw it.** During a gesture only the viewport
   transform changes (a promoted layer). Anything that depends on zoom (label scale, compact cards) is
   one CSS custom property or class on the container, applied after the view settles, in coarse steps.
   No per-node state, style or data updates per zoom step; no SVG background repainted per frame.
   A card's zoomed-out pill (its name's tail among its siblings, and its size per compact level) is
   worked out once per card, and again only when its font loads, and shown by the class.
   Edge routes are worked out when the nodes change (a layout, a measure, a drop), never per frame.
4. **No DOM measuring in reactive paths** that run on scroll, pan or zoom.
5. **Reload in proportion to what changed.** A running run sends a change about every second. A read
   that fetches a whole listing (all checks, all of a run's events) reloads at most every few seconds
   (`createResource(..., { every })`, `throttled`), and app-wide badges at most every 10 s; small reads
   (a run's node states) reload after each burst. A reload never shares a request started before the
   change it answers.
6. **Fetch once, derive locally.** Per-snapshot data (node lists, selection vocabulary) is cached per
   location and snapshot; debounce server evaluation of typed input (~150 ms).
7. **Measure on realistic data.** Mock and benchmark data must be sized like a real project (thousands
   of nodes, long names, populated state). An optimisation proved only on small or empty data does not
   count.
8. **Prove a guard can fail.** When adding a performance check, break the thing it protects once and
   confirm the check fails, then revert.
