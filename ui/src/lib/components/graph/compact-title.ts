// A compact card's title, fitted to the card once: the largest font that fits the card, wrapped at
// name separators onto as many lines as fit, and shortened in the middle when even the smallest font
// doesn't fit. Pure apart from the default text measurer; worked out once per card, never per zoom step.

/** The width of `text` at `font` px (and `weight`, 600 by default), in px. */
export type Measure = (text: string, font: number, weight?: number) => number;

export type TitleBox = {
	/** Room for the title, in the card's own px. */
	width: number;
	height: number;
	/** The smallest font, in the card's px, and the largest the title may grow to. */
	minFont: number;
	maxFont: number;
};

export type TitleFit = { font: number; lines: string[] };

export const LINE_HEIGHT = 1.15;
const ELLIPSIS = '…';

/** Splits after each separator a name may wrap at: `_`, `.`, `/`, `-` and spaces. */
function tokens(text: string): string[] {
	return text.split(/(?<=[_./\- ])/).filter(Boolean);
}

/**
 * `text` wrapped greedily at its separators, or anywhere in a part too wide for a line; with `soft`,
 * null instead when a part is too wide.
 */
export function wrap(text: string, width: number, font: number, measure: Measure, soft = false): string[] | null {
	const lines: string[] = [];
	let line = '';
	for (const token of tokens(text)) {
		if (measure(line + token, font) <= width) {
			line += token;
			continue;
		}
		if (line) lines.push(line);
		line = token;
		if (soft && measure(line, font) > width) return null;
		while (line.length > 1 && measure(line, font) > width) {
			let n = line.length - 1;
			while (n > 1 && measure(line.slice(0, n), font) > width) n--;
			lines.push(line.slice(0, n));
			line = line.slice(n);
		}
	}
	if (line) lines.push(line);
	return lines;
}

/** `text` shortened in the middle to `kept` characters and an ellipsis. */
function middle(text: string, kept: number): string {
	const head = Math.ceil(kept / 2);
	return text.slice(0, head) + ELLIPSIS + text.slice(text.length - (kept - head));
}

export function fitTitle(text: string, box: TitleBox, measure: Measure = measureText): TitleFit {
	const fits = (lines: string[] | null, font: number): lines is string[] => !!lines && lines.length * font * LINE_HEIGHT <= box.height;
	// The largest font at which the title wraps only at its separators; failing that, at the smallest
	// font, broken anywhere; failing that, shortened in the middle.
	const fonts = [];
	for (let font = box.maxFont; font > box.minFont; font = Math.ceil(font) - 1) fonts.push(font);
	for (const font of [...fonts, box.minFont]) {
		const lines = wrap(text, box.width, font, measure, true);
		if (fits(lines, font)) return { font, lines };
	}
	const font = box.minFont;
	const lines = wrap(text, box.width, font, measure);
	if (fits(lines, font)) return { font, lines };
	const most = Math.max(1, Math.floor(box.height / (font * LINE_HEIGHT)));
	// The most characters kept that still fit `most` lines.
	let [lo, hi] = [1, text.length - 1];
	while (lo < hi) {
		const mid = Math.ceil((lo + hi) / 2);
		if (wrap(middle(text, mid), box.width, font, measure)!.length <= most) lo = mid;
		else hi = mid - 1;
	}
	return { font, lines: wrap(middle(text, lo), box.width, font, measure)!.slice(0, most) };
}

// The cards' title font. Measured with a canvas once the font has loaded; before that (and outside a
// browser), estimated generously, so a title fitted early errs on the short side.
const FAMILY = '"Inter Variable", sans-serif';
let context: CanvasRenderingContext2D | null | undefined;
export const measureText: Measure = (text, font, weight = 600) => {
	const spec = `${weight} ${font}px ${FAMILY}`;
	if (context === undefined) context = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
	if (!context || !document.fonts?.check(spec)) return text.length * font * 0.64;
	context.font = spec;
	return context.measureText(text).width;
};
