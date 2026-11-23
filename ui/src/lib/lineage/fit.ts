// Where a graph's view goes when it is fitted. Pure; no side effects.

export type Rect = { x: number; y: number; width: number; height: number };
export type Viewport = { x: number; y: number; zoom: number };

/** A card's title size on the canvas (`.name` in the node cards), and the smallest it may appear on screen. */
export const CARD_TITLE_PX = 12.5;
export const MIN_TITLE_PX = 11;
/** Every card's width on the canvas. */
export const CARD_WIDTH = 236;

/** Space kept between the card a large graph's view starts on and the edge of the view. */
export const ANCHOR_MARGIN = 24;

/**
 * The smallest zoom an automatic fit goes to, where card titles are still at least `MIN_TITLE_PX` on
 * screen. Zooming out by hand can go further.
 */
export const READABLE_ZOOM = MIN_TITLE_PX / CARD_TITLE_PX;

/**
 * A graph of up to this many cards opens, and resets, showing all of it; only a larger one opens at
 * `READABLE_ZOOM` on its start card. Zoomed out further, cards take their compact form (`compactLevel`).
 */
export const WHOLE_FIT_CARDS = 150;

/** The lowest zoom down to which titles are counter-scaled to `MIN_TITLE_PX`, and the lowest a whole-graph fit goes. */
export const LABEL_MIN_ZOOM = 0.35;

/**
 * Below this zoom cards take their compact form, a one-line pill (CompactFace), since the cards' detail
 * lines (11 px) would show under 9 px. Above it cards are drawn in full.
 */
export const COMPACT_ZOOM = 0.82;
/**
 * The compact levels' lowest zooms. A pill's name is fitted once per level, at that level's lowest zoom,
 * and keeps its size on screen through the level (`--sb-title-unzoom`); the last level runs on down,
 * the pill shrinking with the card below `LABEL_MIN_ZOOM`.
 */
export const COMPACT_LEVELS = [0.6, 0.45, LABEL_MIN_ZOOM] as const;

/** How cards are drawn at `zoom`: 0 in full, else the compact level, from 1. */
export function compactLevel(zoom: number): number {
	if (zoom >= COMPACT_ZOOM) return 0;
	const level = COMPACT_LEVELS.findIndex((low) => zoom >= low);
	return level < 0 ? COMPACT_LEVELS.length : level + 1;
}

type FitOptions = {
	/** Share of the view left empty on each side. */
	padding?: number;
	maxZoom?: number;
	minZoom?: number;
	/** What to centre on when the graph doesn't fit at `minZoom`, such as its largest box. */
	focus?: Rect | null;
	/**
	 * Without a focus, the card to start from, put at the view's top-left: the card nearest the
	 * graph's top-left (see `firstCard`). The corner itself can be empty, as a layered layout
	 * centres its columns, and a view on it shows only lines passing by.
	 */
	start?: Rect | null;
};

/**
 * Fits `bounds` into a `width` by `height` view: the whole graph centred, no closer than `maxZoom`.
 * A graph too large to show whole at `minZoom` is shown at `minZoom` instead of shrinking past it,
 * centred on `focus` when given, else from its top-left, and never panned past its own edges, so
 * the view always starts on cards and the person pans or zooms from there.
 */
export function fitViewport(bounds: Rect, width: number, height: number, { padding = 0.08, maxZoom = 1, minZoom = READABLE_ZOOM, focus = null, start = null }: FitOptions = {}): Viewport {
	const [padX, padY] = [width * padding, height * padding];
	const fits = (marginX: number, marginY: number) => Math.min(maxZoom, (width - 2 * marginX) / Math.max(1, bounds.width), (height - 2 * marginY) / Math.max(1, bounds.height));
	let whole = fits(padX, padY);
	// The margin alone mustn't tip a graph into compact cards: one that fits whole at the compact zoom
	// with `ANCHOR_MARGIN` around it opens there, its cards drawn in full.
	if (whole < COMPACT_ZOOM && fits(ANCHOR_MARGIN, ANCHOR_MARGIN) >= COMPACT_ZOOM) whole = COMPACT_ZOOM;
	if (whole >= minZoom) {
		return { zoom: whole, x: width / 2 - (bounds.x + bounds.width / 2) * whole, y: height / 2 - (bounds.y + bounds.height / 2) * whole };
	}
	const zoom = minZoom;
	/** The offset along one axis: centred on the focus, else the start card at the view's start, kept within the graph's edges where the graph is larger than the view. */
	const axis = (start: number, size: number, view: number, pad: number, centre: number | null, first: number | null) => {
		const wanted = centre !== null ? view / 2 - centre * zoom : pad - (first ?? start) * zoom;
		if (size * zoom + 2 * pad <= view) return view / 2 - (start + size / 2) * zoom;
		const most = pad - start * zoom; // the graph's start at the view's start
		const least = view - pad - (start + size) * zoom; // the graph's end at the view's end
		return Math.min(most, Math.max(least, wanted));
	};
	// The card the view starts on (the focus, else the start card) stays wholly in view, `ANCHOR_MARGIN`
	// from the edges; one larger than the view shows its top-left.
	const anchor = focus ?? start;
	const keep = (offset: number, from: number, size: number, view: number) => {
		const [lo, hi] = [from * zoom + offset, (from + size) * zoom + offset];
		if (hi - lo > view - 2 * ANCHOR_MARGIN) return ANCHOR_MARGIN - from * zoom;
		if (lo < ANCHOR_MARGIN) return offset + ANCHOR_MARGIN - lo;
		if (hi > view - ANCHOR_MARGIN) return offset - (hi - (view - ANCHOR_MARGIN));
		return offset;
	};
	const x = axis(bounds.x, bounds.width, width, padX, focus ? focus.x + focus.width / 2 : null, start?.x ?? null);
	const y = axis(bounds.y, bounds.height, height, padY, focus ? focus.y + focus.height / 2 : null, start?.y ?? null);
	return anchor ? { zoom, x: keep(x, anchor.x, anchor.width, width), y: keep(y, anchor.y, anchor.height, height) } : { zoom, x, y };
}

/** The largest box among `items`, the cluster a large graph's view starts on; null when there is no box. */
export function largestBox(items: readonly (Rect & { box: boolean })[]): Rect | null {
	let best: Rect | null = null;
	for (const item of items) if (item.box && (!best || item.width * item.height > best.width * best.height)) best = item;
	return best && { x: best.x, y: best.y, width: best.width, height: best.height };
}

/** The card nearest the top-left of `items`' bounds, where a large graph's view starts without a box to centre on. */
export function firstCard<R extends Rect>(items: readonly R[]): R | null {
	if (!items.length) return null;
	// The leftmost column: cards starting within half a card of the leftmost edge. Its top card, not the
	// card nearest the corner, which can sit in a later column and leave the first one off screen.
	const left = Math.min(...items.map((r) => r.x));
	const reach = Math.min(...items.map((r) => r.width)) / 2;
	const column = items.filter((r) => r.x <= left + reach);
	return column.reduce((best, r) => (r.y < best.y ? r : best));
}

type Placed = { id: string; position: { x: number; y: number }; parentId?: string };

/** A node's position on the graph, adding up the boxes around it, or null when it isn't drawn. */
export function absolutePosition(nodes: readonly Placed[], id: string): { x: number; y: number } | null {
	const byId = new Map(nodes.map((n) => [n.id, n]));
	let at = byId.get(id);
	if (!at) return null;
	let { x, y } = at.position;
	while (at.parentId && (at = byId.get(at.parentId))) {
		x += at.position.x;
		y += at.position.y;
	}
	return { x, y };
}

/**
 * The view after a re-layout that keeps a box where it was on screen: the same zoom, panned by the
 * box's layout shift, so its top-left stays under the pointer that opened or closed it.
 */
export function anchoredViewport(view: Viewport, before: { x: number; y: number }, after: { x: number; y: number }): Viewport {
	return { zoom: view.zoom, x: view.x - (after.x - before.x) * view.zoom, y: view.y - (after.y - before.y) * view.zoom };
}

// ---- mockup: `?cards=new`, staged levels of detail ----
//
// Each level is kept while the card's name renders at about 10 px or more on screen:
// full card (name and status line) down to `NEW_FULL_ZOOM`, where the 11 px status line would read
// under 9 px; then the same card with its name alone, enlarged up to `NEW_NAME_MAX_PX` canvas px so it
// stays 10 px on screen, down to `NEW_NAME_ZOOM`; then a pill across the card's full width, down to
// `NEW_PILL_LEVELS`' last zoom; then a dot.

export const NEW_FULL_ZOOM = 0.8;
/** The largest a name-only card's name grows, in canvas px: the card's height leaves room for it. */
export const NEW_NAME_MAX_PX = 26;
/** The name's size on screen a level keeps. */
export const NEW_NAME_SCREEN_PX = 10;
export const NEW_NAME_ZOOM = NEW_NAME_SCREEN_PX / NEW_NAME_MAX_PX;
/** The pill levels' lowest zooms; below the last, cards are dots. */
export const NEW_PILL_LEVELS = [0.315] as const;

export type CardStage = 'full' | 'name' | 'pill' | 'dot';

export function newCardStage(zoom: number): CardStage {
	if (zoom >= NEW_FULL_ZOOM) return 'full';
	if (zoom >= NEW_NAME_ZOOM) return 'name';
	return zoom >= NEW_PILL_LEVELS[NEW_PILL_LEVELS.length - 1] ? 'pill' : 'dot';
}

/**
 * The canvas's compact level under `cards=new`: 0 for full cards; 1 also through the name-only stage,
 * where cards other than node cards (folders, member jobs) show their pill; then a level per pill
 * zoom; past the last, dots.
 */
export function newCompactLevel(zoom: number): number {
	if (zoom >= NEW_FULL_ZOOM) return 0;
	if (zoom >= NEW_NAME_ZOOM) return 1;
	const level = NEW_PILL_LEVELS.findIndex((low) => zoom >= low);
	return level < 0 ? NEW_PILL_LEVELS.length + 1 : level + 1;
}
