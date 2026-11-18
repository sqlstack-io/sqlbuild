// Pure windowing maths for long lists drawn with fixed-height rows.

export interface WindowRange {
	/** Index of the first row to draw. */
	first: number;
	/** One past the index of the last row to draw. */
	last: number;
}

/**
 * The rows to draw for a scroll position: those in view plus `overscan` rows either side,
 * clamped to the list. `first` never passes `last`, even when the list shrank below the old
 * scroll position.
 */
export function windowRange(scrollTop: number, viewHeight: number, rowHeight: number, overscan: number, count: number): WindowRange {
	const last = Math.max(0, Math.min(count, Math.ceil((scrollTop + viewHeight) / rowHeight) + overscan));
	const first = Math.min(last, Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
	return { first, last };
}
