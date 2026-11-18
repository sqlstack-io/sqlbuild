// Text measured to fit a card: the status line's parts, and a zoomed-out pill's name. A pill's name is
// sized on screen, as large as fits between `PILL.minFont` and `PILL.maxFont`, then cut with an
// ellipsis at its end. Worked out once per compact level, never per zoom step.

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
export const measureText: Measure = measureIn(() => '"Inter Variable", sans-serif', 0.64);

/** The pills' name font: whatever the page sets `--font-node-compact` to. */
function compactFamily(): string {
	if (typeof document === 'undefined') return 'sans-serif';
	return getComputedStyle(document.documentElement).getPropertyValue('--font-node-compact').trim() || 'sans-serif';
}
export const measureCompact: Measure = measureIn(compactFamily, 0.66);
