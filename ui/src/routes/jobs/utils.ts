// Pure helpers for the Jobs list.

/** One selection token split for display: a dimmed `tag:`-style prefix and the name that stands out. */
export interface SelectionToken {
	/** The whole token, e.g. `+tag:orders`. */
	token: string;
	/** Graph operators and the method, e.g. `+tag:`; empty for a bare model name or path. */
	prefix: string;
	name: string;
}

/** Splits a job's selection into one token per selector, at spaces and commas, with its method prefix. */
export function selectionTokens(select: string | null | undefined): SelectionToken[] {
	return (select ?? '')
		.trim()
		.split(/[\s,]+/)
		.filter(Boolean)
		.map((token) => {
			const m = /^([@+\d]*[a-z][a-z0-9_]*:)(.+)$/i.exec(token);
			return m ? { token, prefix: m[1], name: m[2] } : { token, prefix: '', name: token };
		});
}

/**
 * A job owner: a person (an email) with initials in a coloured circle and a readable name, or a team
 * (anything else, such as `team:orders`) shown as a label without initials.
 */
export interface OwnerChip {
	kind: 'person' | 'team';
	/** Null for a team. */
	initials: string | null;
	name: string;
	colour: string;
	/** The owner as declared, e.g. the full email, for the hover text. */
	full: string;
}

/** Light enough for dark initials on either theme. */
const OWNER_COLOURS = ['#7cc4fa', '#a78bfa', '#3ecf8e', '#f5b13d', '#f0a3c4', '#8bd3c7'];

/**
 * The chip for an owner: a person for an email (`jane.doe@example.com` → `JD`, `Jane Doe`), a team for
 * anything else (`team:orders` → `orders`), or null when there's none.
 */
export function ownerChip(owner: string | null | undefined): OwnerChip | null {
	const full = owner?.trim() ?? '';
	if (!full) return null;
	if (!full.includes('@')) {
		// A `kind:` prefix such as `team:` or `group:` says what it is; the label is the rest.
		const name = full.replace(/^[a-z]+:/i, '') || full;
		return { kind: 'team', initials: null, name, colour: '', full };
	}
	const local = full.slice(0, full.indexOf('@')) || full;
	const words = local.split(/[\s._-]+/).filter(Boolean);
	const name = words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(' ') || local;
	const letters = words.length > 1 ? words[0].charAt(0) + words[1].charAt(0) : (words[0] ?? local).slice(0, 2);
	let hash = 0;
	for (const ch of local.toLowerCase()) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
	return { kind: 'person', initials: letters.toUpperCase(), name, colour: OWNER_COLOURS[hash % OWNER_COLOURS.length], full };
}
