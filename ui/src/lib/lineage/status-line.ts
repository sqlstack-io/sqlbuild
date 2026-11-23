// A node card's status line kept to one line. Pure apart from the default text measurer. The time
// never shortens; when the parts don't fit, the lowest-priority ones shorten first: the drift chip to
// its icon, then the status word to its dot. (The checks badge is already an icon and a count.)

import { measureText, type Measure } from '$lib/components/graph/compact-title';

/** The room on a card's status line: the card's 236 px less its border and padding. */
export const STATUS_ROOM = 236 - 2 - 2 * 10;
const GAP = 6;
const DOT = 7;
/** A drift chip's icon, gap, padding and border around its word. */
const CHIP = 10 + 3 + 2 * 5 + 2;
/** The '·' and its margin before the time when it follows the status word, as drawn. */
const SEP = 12;
/** What the checks badge draws beyond its measured text: tabular figures, and a ✕ or ✓ from a fallback font. */
const CHECKS_EXTRA = 3;

export type StatusParts = { label?: string; when?: string; drift?: string; checks?: string };
export type StatusFit = { label: boolean; driftWord: boolean };

/** Which parts show in full: the status word, and the drift chip's word. */
export function fitStatus(parts: StatusParts, measure: Measure = measureText, room = STATUS_ROOM): StatusFit {
	const width = ({ label, driftWord }: StatusFit) => {
		const items = [
			parts.label !== undefined ? DOT + (label ? GAP + measure(parts.label, 11, 500) : 0) : null,
			parts.when ? measure(parts.when, 11, 400) + (parts.label !== undefined ? SEP : 0) : null,
			parts.drift ? CHIP + (driftWord ? measure(parts.drift, 10.5, 600) : -3) : null,
			parts.checks ? measure(parts.checks, 11, 400) + CHECKS_EXTRA : null
		].filter((w): w is number => w !== null);
		return items.reduce((a, b) => a + b, 0) + GAP * Math.max(0, items.length - 1);
	};
	for (const fit of [
		{ label: true, driftWord: true },
		{ label: true, driftWord: false },
		{ label: false, driftWord: false }
	])
		if (width(fit) <= room) return fit;
	return { label: false, driftWord: false };
}
