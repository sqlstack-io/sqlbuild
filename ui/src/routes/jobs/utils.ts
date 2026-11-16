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

/** One selection token split for display: a dimmed `tag:`-style prefix and the name that stands out. */
export interface SelectionToken {
	/** The whole token, e.g. `+tag:orders`. */
	token: string;
	/** Graph operators and the method, e.g. `+tag:`; empty for a bare model name or path. */
	prefix: string;
	name: string;
}

/** Splits a job's selection (space-separated tokens) into tokens with their method prefix. */
export function selectionTokens(select: string | null | undefined): SelectionToken[] {
	return (select ?? '')
		.trim()
		.split(/\s+/)
		.filter(Boolean)
		.map((token) => {
			const m = /^([@+\d]*[a-z][a-z0-9_]*:)(.+)$/i.exec(token);
			return m ? { token, prefix: m[1], name: m[2] } : { token, prefix: '', name: token };
		});
}

/** A job owner shown as a person: initials for the circle, a readable name and the circle's colour. */
export interface OwnerChip {
	initials: string;
	name: string;
	colour: string;
	/** The owner as declared, e.g. the full email, for the hover text. */
	full: string;
}

/** Light enough for dark initials on either theme. */
const OWNER_COLOURS = ['#7cc4fa', '#a78bfa', '#3ecf8e', '#f5b13d', '#f0a3c4', '#8bd3c7'];

/** The person chip for an owner (`jane.doe@example.com` → `JD`, `Jane Doe`), or null when there's none. */
export function ownerChip(owner: string | null | undefined): OwnerChip | null {
	const full = owner?.trim() ?? '';
	if (!full) return null;
	const local = full.includes('@') ? full.slice(0, full.indexOf('@')) || full : full;
	const words = local.split(/[\s._-]+/).filter(Boolean);
	const name = words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(' ') || local;
	const letters = words.length > 1 ? words[0].charAt(0) + words[1].charAt(0) : (words[0] ?? local).slice(0, 2);
	let hash = 0;
	for (const ch of local.toLowerCase()) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
	return { initials: letters.toUpperCase(), name, colour: OWNER_COLOURS[hash % OWNER_COLOURS.length], full };
}
