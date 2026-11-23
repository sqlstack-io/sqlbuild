// Text measured to fit a card: the status line's parts, and a zoomed-out pill's name. A pill's name is
// sized on screen, as large as fits between `PILL.minFont` and `PILL.maxFont`; a name too long even
// then loses its middle, keeping its start and, more of, its end: siblings differ at either end
// (`stg_orders`, `dim_orders`; `region_check`, `region_country`). Worked out once per compact level,
// never per zoom step.

import { CARD_WIDTH } from '$lib/lineage/fit';

/** The width of `text` at `font` px (and `weight`), in px. */
export type Measure = (text: string, font: number, weight?: number) => number;

/** A zoomed-out pill's parts, in px on screen. */
export const PILL = { height: 30, ring: 20, padLeft: 5, gap: 8, padRight: 14, border: 1, minFont: 12, maxFont: 14, weight: 500 } as const;

/** Room for a pill's name on screen at `zoom`: the card's width less the pill's ring and padding. */
export function pillRoom(zoom: number): number {
	return CARD_WIDTH * zoom - 2 * PILL.border - PILL.padLeft - PILL.ring - PILL.gap - PILL.padRight;
}

/** The largest size, in quarter px from `PILL.maxFont` down to `PILL.minFont`, at which `text` fits `room`. */
export function pillFont(text: string, room: number, measure: Measure): number {
	const { minFont, maxFont, weight } = PILL;
	const widest = measure(text, maxFont, weight);
	if (widest <= room) return maxFont;
	// Width grows with the size, so start near where it fits and step down to be sure.
	let font = Math.max(minFont, Math.min(maxFont, Math.floor(((maxFont * room) / widest) * 4) / 4));
	while (font > minFont && measure(text, font, weight) > room) font -= 0.25;
	return font;
}

/** `text` without its middle: `kept` characters, a third from its start and the rest from its end. */
function middleCut(text: string, kept: number): string {
	const head = Math.floor(kept / 3);
	return `${text.slice(0, head)}…${kept - head ? text.slice(-(kept - head)) : ''}`;
}

/**
 * `text` at `font` as it fits `room`: whole, or with its middle cut to the most characters that fit.
 * With a `lead` (mockup), the cut falls in the first `lead` characters, shared with a sibling, so the
 * rest, which tells it apart, stays whole: `genea…trainer`.
 */
export function pillText(text: string, font: number, room: number, measure: Measure, lead = 0): string {
	if (measure(text, font, PILL.weight) <= room) return text;
	if (lead > 0) {
		const rest = text.slice(lead);
		// As much of the shared prefix's start as fits, then the distinct rest whole.
		for (let k = lead - 1; k >= 0; k--) if (measure(`${text.slice(0, k)}…${rest}`, font, PILL.weight) <= room) return `${text.slice(0, k)}…${rest}`;
		// Still too long: keep as much of the rest's start as fits, since siblings differ there.
		let kept = rest.length - 1;
		while (kept > 1 && measure(`…${rest.slice(0, kept)}…`, font, PILL.weight) > room) kept--;
		return `…${rest.slice(0, kept)}…`;
	}
	let [lo, hi] = [0, text.length - 1];
	while (lo < hi) {
		const mid = Math.ceil((lo + hi) / 2);
		if (measure(middleCut(text, mid), font, PILL.weight) <= room) lo = mid;
		else hi = mid - 1;
	}
	return middleCut(text, lo);
}

let context: CanvasRenderingContext2D | null | undefined;
const loading = new Set<string>();
/** A measurer in `family` (or the family `family()` names when it is measured), estimating generously until the font has loaded. */
function measureIn(family: () => string, fallback: number): Measure {
	return (text, font, weight = 600) => {
		const spec = `${weight} ${font}px ${family()}`;
		if (context === undefined) context = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
		if (!context) return text.length * font * fallback;
		if (!document.fonts?.check(spec)) {
			// Ask for the font, so it loads (and the text is fitted again) even before it first shows.
			const face = `${weight} 16px ${family()}`;
			if (!loading.has(face)) {
				loading.add(face);
				void document.fonts?.load(face).catch(() => {});
			}
			return text.length * font * fallback;
		}
		context.font = spec;
		return context.measureText(text).width;
	};
}

/** The cards' own font, Inter, as the status line measures it. */
/** The cards' font: Inter, or the mock font switcher's choice (`--card-font`, `?cards=new&font=`). */
function cardFamily(): string {
	if (typeof document === 'undefined') return '"Inter Variable", sans-serif';
	return document.documentElement.style.getPropertyValue('--card-font').trim() || '"Inter Variable", sans-serif';
}
export const measureText: Measure = measureIn(cardFamily, 0.64);

/** The pills' name font: whatever the page sets `--font-node-compact` to. */
function compactFamily(): string {
	if (typeof document === 'undefined') return 'sans-serif';
	return getComputedStyle(document.documentElement).getPropertyValue('--font-node-compact').trim() || 'sans-serif';
}
export const measureCompact: Measure = measureIn(compactFamily, 0.66);

// ---- mockup: `?cards=new` pills ----

/** A `cards=new` pill's parts on screen: a small status dot instead of the icon ring, and tighter padding. */
export const NEW_PILL = { ring: 10, padLeft: 6, gap: 6, padRight: 8, border: 1, minFont: 10, maxFont: 12.5, weight: 500 } as const;

/** Room for a `cards=new` pill's name on screen at `zoom`: the card's full width less the dot and padding, which shrink with the card below `LABEL_MIN_ZOOM`. */
export function newPillRoom(zoom: number, labelMinZoom: number): number {
	const parts = 2 * NEW_PILL.border + NEW_PILL.padLeft + NEW_PILL.ring + NEW_PILL.gap + NEW_PILL.padRight;
	return CARD_WIDTH * zoom - parts * Math.min(1, zoom / labelMinZoom);
}

/** The largest size from `NEW_PILL.maxFont` down to `NEW_PILL.minFont` at which `text` fits `room`. */
export function newPillFont(text: string, room: number, measure: Measure): number {
	let font: number = NEW_PILL.maxFont;
	while (font > NEW_PILL.minFont && measure(text, font, NEW_PILL.weight) > room) font -= 0.25;
	return font;
}

/**
 * `text` as it fits `room` on a `cards=new` pill: whole; else, with a shared `lead` (discovered steps),
 * cut inside the lead so the distinct rest stays whole; else its end cut, keeping its start.
 */
export function newPillText(text: string, font: number, room: number, measure: Measure, lead = 0): string {
	const fits = (t: string) => measure(t, font, NEW_PILL.weight) <= room;
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
