// Pure helpers for the Jobs list.

export interface Capped<T> {
	shown: T[];
	/** The items left out, for the "+N more" tooltip. */
	hidden: T[];
}

/**
 * Items drawn one per line, at most `lines` lines: all of them when they fit, otherwise
 * `lines - 1` items and a "+N more" line.
 */
export function capLines<T>(items: readonly T[], lines = 2): Capped<T> {
	const keep = items.length <= lines ? items.length : Math.max(0, lines - 1);
	return { shown: items.slice(0, keep), hidden: items.slice(keep) };
}

/** The first `max` items, and the rest for a "+N more" line. */
export function capItems<T>(items: readonly T[], max: number): Capped<T> {
	return { shown: items.slice(0, max), hidden: items.slice(max) };
}
