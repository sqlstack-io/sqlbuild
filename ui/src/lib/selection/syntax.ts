// SQLBuild's selection syntax, read the way `sqb build -s` reads it: whitespace-separated tokens are
// unioned; a token's comma parts are intersected; each part may carry `+` before (upstream) and
// after (downstream), which expands that part alone before the intersection; `a~b` is the nodes on
// the paths from `a` to `b`. Pure: it splits text and never decides what exists.

/** Selector types `sqb` accepts before a colon. */
export const SELECTOR_TYPES = ['tag', 'path', 'job', 'kind', 'source', 'seed', 'test', 'asset', 'loader', 'check', 'audit', 'run', 'failed', 'skipped'] as const;
export type SelectorType = (typeof SELECTOR_TYPES)[number];

/** One comma part of a token, such as `+tag:finance`. */
export type Part = {
	raw: string;
	/** `+` before: everything upstream too. */
	up: boolean;
	/** `+` after: everything downstream too. */
	down: boolean;
	/** The type before the colon; `null` for a bare name, which is a folder when it holds `/`. */
	type: string | null;
	value: string;
	/** `a~b`: the two ends of a path. */
	between: [string, string] | null;
};

/** A whitespace-separated token: its comma parts and where it sits in the text. */
export type Token = { raw: string; start: number; end: number; parts: Part[] };

export function parsePart(raw: string): Part {
	const up = raw.startsWith('+');
	const down = raw.length > 1 && raw.endsWith('+');
	const core = raw.replace(/^\+/, '').replace(/\+$/, '');
	const tilde = core.indexOf('~');
	if (tilde >= 0) return { raw, up, down, type: null, value: core, between: [core.slice(0, tilde), core.slice(tilde + 1)] };
	const colon = core.indexOf(':');
	if (colon > 0) return { raw, up, down, type: core.slice(0, colon), value: core.slice(colon + 1), between: null };
	return { raw, up, down, type: null, value: core, between: null };
}

/** A part's text, as `sqb` takes it. */
export function partText(p: Pick<Part, 'up' | 'down' | 'type' | 'value'>): string {
	return `${p.up ? '+' : ''}${p.type ? `${p.type}:` : ''}${p.value}${p.down ? '+' : ''}`;
}

export function parseToken(raw: string, start = 0): Token {
	return { raw, start, end: start + raw.length, parts: raw.split(',').map(parsePart) };
}

/** The tokens of a selection text, with their positions in it. */
export function parseSelection(text: string): Token[] {
	return [...text.matchAll(/\S+/g)].map((m) => parseToken(m[0], m.index));
}

/** The token list as one selection text, single-spaced. */
export const joinTokens = (tokens: readonly string[]) => tokens.filter(Boolean).join(' ');

/** A bare name holding `/` is a folder, as `sqb` reads `models/staging`. */
export const isFolder = (p: Part) => p.type === null && !p.between && p.value.includes('/');

export const isKnownType = (type: string): type is SelectorType => (SELECTOR_TYPES as readonly string[]).includes(type);
