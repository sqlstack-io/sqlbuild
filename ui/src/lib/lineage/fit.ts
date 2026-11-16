// Where a graph's view goes when it is fitted. Pure; no side effects.

export type Rect = { x: number; y: number; width: number; height: number };
export type Viewport = { x: number; y: number; zoom: number };

/** A card's title size on the canvas (`.name` in the node cards), and the smallest it may appear on screen. */
export const CARD_TITLE_PX = 12.5;
export const MIN_TITLE_PX = 11;

/** Space kept between the card a large graph's view starts on and the edge of the view. */
export const ANCHOR_MARGIN = 24;

/**
 * The smallest zoom an automatic fit goes to, where card titles are still at least `MIN_TITLE_PX` on
 * screen. Zooming out by hand can go further.
 */
export const READABLE_ZOOM = MIN_TITLE_PX / CARD_TITLE_PX;

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
	const whole = Math.min(maxZoom, (width - 2 * padX) / Math.max(1, bounds.width), (height - 2 * padY) / Math.max(1, bounds.height));
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
