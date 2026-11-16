// Where a graph's view goes when it is fitted. Pure; no side effects.

export type Rect = { x: number; y: number; width: number; height: number };
export type Viewport = { x: number; y: number; zoom: number };

/** The smallest zoom an automatic fit goes to, where card names stay readable. Zooming out by hand can go further. */
export const READABLE_ZOOM = 0.6;

type FitOptions = {
	/** Share of the view left empty on each side. */
	padding?: number;
	maxZoom?: number;
	minZoom?: number;
	/** What to centre on when the graph doesn't fit at `minZoom`, such as its largest box; the graph's top-left otherwise. */
	focus?: Rect | null;
};

/**
 * Fits `bounds` into a `width` by `height` view: the whole graph centred, no closer than `maxZoom`.
 * A graph too large to show whole at `minZoom` is shown at `minZoom` instead of shrinking past it,
 * centred on `focus` when given, else from its top-left, and never panned past its own edges, so
 * the view always starts on cards and the person pans or zooms from there.
 */
export function fitViewport(bounds: Rect, width: number, height: number, { padding = 0.08, maxZoom = 1, minZoom = READABLE_ZOOM, focus = null }: FitOptions = {}): Viewport {
	const [padX, padY] = [width * padding, height * padding];
	const whole = Math.min(maxZoom, (width - 2 * padX) / Math.max(1, bounds.width), (height - 2 * padY) / Math.max(1, bounds.height));
	if (whole >= minZoom) {
		return { zoom: whole, x: width / 2 - (bounds.x + bounds.width / 2) * whole, y: height / 2 - (bounds.y + bounds.height / 2) * whole };
	}
	const zoom = minZoom;
	/** The offset along one axis: centred on the focus, kept within the graph's edges where the graph is larger than the view. */
	const axis = (start: number, size: number, view: number, pad: number, centre: number | null) => {
		const wanted = centre === null ? pad - start * zoom : view / 2 - centre * zoom;
		if (size * zoom + 2 * pad <= view) return view / 2 - (start + size / 2) * zoom;
		const most = pad - start * zoom; // the graph's start at the view's start
		const least = view - pad - (start + size) * zoom; // the graph's end at the view's end
		return Math.min(most, Math.max(least, wanted));
	};
	return {
		zoom,
		x: axis(bounds.x, bounds.width, width, padX, focus ? focus.x + focus.width / 2 : null),
		y: axis(bounds.y, bounds.height, height, padY, focus ? focus.y + focus.height / 2 : null)
	};
}

/** The largest box among `items`, the cluster a large graph's view starts on; null when there is no box. */
export function largestBox(items: readonly (Rect & { box: boolean })[]): Rect | null {
	let best: Rect | null = null;
	for (const item of items) if (item.box && (!best || item.width * item.height > best.width * best.height)) best = item;
	return best && { x: best.x, y: best.y, width: best.width, height: best.height };
}
