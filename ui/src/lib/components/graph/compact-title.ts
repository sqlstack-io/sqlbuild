// Text measured to fit a card: the status line's parts, a name-only card's name and a zoomed-out
// pill's name. A name too long loses its end, keeping its start, except a discovered step's, which
// first drops the prefix it shares with its siblings. Widths are measured once per font, size and
// text (sizes come in half or quarter px steps), so zooming a large graph doesn't measure every card
// again. They aren't scaled from one size: the browser rounds each glyph's advance at small sizes.

import { CARD_WIDTH, LABEL_MIN_ZOOM, NAME_MAX_PX, NAME_SCREEN_PX, PILL_ZOOM } from '$lib/lineage/fit';

/** The width of `text` at `font` px (and `weight`), in px. */
export type Measure = (text: string, font: number, weight?: number) => number;

/** A pill's parts on screen: a status dot, padding, and the name's size range. */
export const PILL = { dot: 10, padLeft: 6, gap: 6, padRight: 8, border: 1, minFont: 10, maxFont: 12.5, weight: 500 } as const;

/** Room for a pill's name on screen at `zoom`: the card's full width less the dot and padding, which shrink with the card below `LABEL_MIN_ZOOM`. */
export function pillRoom(zoom: number): number {
	const parts = 2 * PILL.border + PILL.padLeft + PILL.dot + PILL.gap + PILL.padRight;
	return CARD_WIDTH * zoom - parts * Math.min(1, zoom / LABEL_MIN_ZOOM);
}

/** The largest size, in quarter px from `PILL.maxFont` down to `PILL.minFont`, at which `text` fits `room`. */
export function pillFont(text: string, room: number, measure: Measure): number {
	let font: number = PILL.maxFont;
	while (font > PILL.minFont && measure(text, font, PILL.weight) > room) font -= 0.25;
	return font;
}

/**
 * `text` as it fits `room` at `font`: whole; else, with a shared `lead` (a discovered step's),
 * cut inside the lead so the distinct rest stays whole, then the rest's end; else its end cut,
 * keeping its start.
 */
export function pillText(text: string, font: number, room: number, measure: Measure, lead = 0, weight: number = PILL.weight): string {
	const fits = (t: string) => measure(t, font, weight) <= room;
	if (fits(text)) return text;
	const endCut = (t: string, before = '') => {
		let [lo, hi] = [0, t.length - 1];
		while (lo < hi) {
			const mid = Math.ceil((lo + hi) / 2);
			if (fits(`${before}${t.slice(0, mid)}…`)) lo = mid;
			else hi = mid - 1;
		}
		return `${before}${t.slice(0, lo)}…`;
	};
	if (lead > 0) {
		// A discovered key's closing bracket goes with its prefix: `forecast_eu[gbm_gru]` reads `…gbm_gru`.
		const rest = text.slice(lead).replace(/\]$/, '');
		for (let k = lead - 1; k >= 0; k--) if (fits(`${text.slice(0, k)}…${rest}`)) return `${text.slice(0, k)}…${rest}`;
		return endCut(rest, '…');
	}
	return endCut(text);
}

/** A pill name's fit at the pill stage's lowest zoom, where the card is narrowest. */
export function pillFit(text: string, lead: number, measure: Measure): { px: number; text: string } {
	const room = pillRoom(PILL_ZOOM);
	const px = pillFont(text, room, measure);
	return { px, text: pillText(text, px, room, measure, lead) };
}

/** A name-only card's name size in canvas px at `zoom`, rounded up to half a px so a fit at it holds on screen. */
export function nameOnlyPx(zoom: number): number {
	return Math.ceil(Math.min(NAME_MAX_PX, Math.max(12.5, NAME_SCREEN_PX / zoom)) * 2) / 2;
}

/** A name-only card's icon size in canvas px at `zoom`. */
export const nameOnlyIcon = (zoom: number) => 14 * Math.min(1 / zoom, 1.8);

const nameOnlyFits = new Map<string, string>();
const KEPT_FITS = 20_000;
/** A name-only card's name as it fits the card at `zoom`, worked out once per name, size and lead. */
export function nameOnlyText(label: string, zoom: number, lead: number, measure: Measure): string {
	const px = nameOnlyPx(zoom);
	const key = `${px}\u0000${lead}\u0000${label}`;
	let text = nameOnlyFits.get(key);
	if (text === undefined) {
		const room = CARD_WIDTH - 2 - 20 - 8 - nameOnlyIcon(zoom);
		text = pillText(label, px, room, measure, lead, 600);
		if (nameOnlyFits.size > KEPT_FITS) nameOnlyFits.clear();
		if (measured(measure)) nameOnlyFits.set(key, text);
	}
	return text;
}

let context: CanvasRenderingContext2D | null | undefined;
const loading = new Set<string>();
/** Fonts known to have loaded, so they aren't checked again. */
const loaded = new Set<string>();
/** Text widths by font (weight, size and family), then text; a font's are dropped past `KEPT_WIDTHS`, as times in status lines keep changing. */
const widths = new Map<string, Map<string, number>>();
const KEPT_WIDTHS = 20_000;
/** Measurers that measured real widths, as opposed to estimating while their font loads. */
const real = new WeakSet<Measure>();
const measured = (measure: Measure) => real.has(measure);

/** A measurer in the family `family()` names, estimating generously until the font has loaded. */
function measureIn(family: () => string, fallback: number): Measure {
	const measure: Measure = (text, font, weight = 600) => {
		const name = family();
		if (context === undefined) context = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
		if (!context) return text.length * font * fallback;
		const face = `${weight} ${font}px ${name}`;
		if (!loaded.has(face) && !document.fonts?.check(face)) {
			// Ask for the font, so it loads (and the text is fitted again) even before it first shows.
			if (!loading.has(face)) {
				loading.add(face);
				void document.fonts?.load(face).catch(() => {});
			}
			real.delete(measure);
			return text.length * font * fallback;
		}
		loaded.add(face);
		real.add(measure);
		let known = widths.get(face);
		if (!known || known.size > KEPT_WIDTHS) widths.set(face, (known = new Map()));
		let width = known.get(text);
		if (width === undefined) {
			context.font = face;
			width = context.measureText(text).width;
			known.set(text, width);
		}
		return width;
	};
	return measure;
}

/** The family the page sets CSS variable `name` to, read once: it doesn't change while the page runs. */
function pageFamily(name: string, fallback: string): () => string {
	let family: string | undefined;
	return () => {
		if (family !== undefined) return family;
		if (typeof document === 'undefined') return fallback;
		const set = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
		if (set) family = set;
		return set || fallback;
	};
}
/** The cards' font, as the page sets `--font-mono` (IBM Plex Mono). */
const cardFamily = pageFamily('--font-mono', 'monospace');
/** The cards' name and status line font, as the status line measures it. */
export const measureText: Measure = measureIn(cardFamily, 0.62);

/** The pills' name font: whatever the page sets `--font-node-compact` to. */
export const measureCompact: Measure = measureIn(pageFamily('--font-node-compact', 'sans-serif'), 0.66);
