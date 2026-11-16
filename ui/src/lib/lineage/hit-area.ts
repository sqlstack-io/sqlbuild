// Click targets on a graph that stay easy to hit when zoomed out. Pure; no side effects.

/** The least on-screen size of a control on a graph card, such as a folder's chevron, in pixels. */
export const MIN_HIT = 28;

/** How much a graph control grows in graph units so it keeps its on-screen size: 1 at full size and above, more when zoomed out. */
export const unzoom = (zoom: number) => (zoom > 0 && zoom < 1 ? 1 / zoom : 1);
