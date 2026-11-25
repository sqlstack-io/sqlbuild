// The run log's one filter: the text in its box. Every quick filter on the run page (the header's
// figures, the Errors chip, the Pick chips) only adds its own words to that text or takes them out, so
// typing the same words by hand does exactly the same. What is shown, what is pressed and the
// "Showing …" line are all read back from the text. Pure.

import type { LogEntry } from './events';

/** What a row is, for `kind:`: a node's outcome, an audit, a query, the run's own steps, a node's log line, or printed output. */
export type LogKind = 'node' | 'audit' | 'query' | 'run' | 'log' | 'output';

export function kindOf(e: LogEntry): LogKind {
	if (e.category === 'output') return 'output';
	if (e.category === 'checks') return 'audit';
	if (e.category === 'queries') return 'query';
	if (e.category === 'nodes') return e.tag?.text === 'log' ? 'log' : 'node';
	return 'run';
}

const NODE_OUTCOME: Record<string, string> = { success: 'built', warn: 'built', failed: 'failed', skipped: 'skipped', cancelled: 'cancelled', not_run: 'not-run', running: 'building', pending: 'waiting' };

/** How a row ended, for `outcome:`: a node built, failed, skipped…; an audit passed, warned or failed; a query failed. */
export function outcomeOf(e: LogEntry): string | null {
	if (e.outcome) return e.outcome;
	if (e.node) return NODE_OUTCOME[e.node.status] ?? null;
	return null;
}

/** The values `outcome:` takes, in the order the box suggests them. */
export const OUTCOMES = ['built', 'failed', 'skipped', 'cancelled', 'not-run', 'building', 'waiting', 'passed', 'warned'];

const words = (text: string) => text.trim().split(/\s+/).filter(Boolean);
const keyOf = (word: string) => (word.includes(':') ? word.slice(0, word.indexOf(':')).toLowerCase() : null);

/** Whether the box's text holds every word of `text`. */
export function holdsText(query: string, text: string): boolean {
	const have = new Set(words(query).map((w) => w.toLowerCase()));
	return words(text).every((w) => have.has(w.toLowerCase()));
}

/**
 * A quick filter's click: when the text holds its words, they come out and everything else stays;
 * otherwise they go in, in place of any word with the same `key:` (one level, one outcome at a time).
 * The result ends in a space when it holds anything, as after picking a suggestion.
 */
export function toggleText(query: string, text: string): string {
	const mine = words(text);
	const lower = new Set(mine.map((w) => w.toLowerCase()));
	const kept = holdsText(query, text)
		? words(query).filter((w) => !lower.has(w.toLowerCase()))
		: [...words(query).filter((w) => { const k = keyOf(w); return !(k && mine.some((m) => keyOf(m) === k)); }), ...mine];
	return kept.length ? `${kept.join(' ')} ` : '';
}

/** A quick filter's words and what the "Showing …" line calls them. */
export type QuickFilter = { text: string; label: string };

/** What the "Showing …" line says for the text: a quick filter's name when the text is exactly its words, else the text itself. */
export function describeQuery(query: string, quick: readonly QuickFilter[]): { label: string; typed: boolean } | null {
	const own = words(query);
	if (!own.length) return null;
	const same = (a: string[], b: string[]) => a.length === b.length && a.every((w) => b.some((x) => x.toLowerCase() === w.toLowerCase()));
	const named = quick.find((q) => same(words(q.text), own));
	return named ? { label: named.label, typed: false } : { label: own.join(' '), typed: true };
}

/** The run page's quick filters, by what they show; the header's figures and the Pick chips read these. */
export const QUICK = {
	errors: { text: 'level:error', label: 'errors' },
	warnings: { text: 'level:warn', label: 'warnings' },
	auditsFailed: { text: 'kind:audit outcome:failed', label: 'failed audits' },
	failed: { text: 'kind:node outcome:failed', label: 'failed nodes' },
	built: { text: 'outcome:built', label: 'built' },
	skipped: { text: 'outcome:skipped', label: 'skipped' },
	notRun: { text: 'outcome:not-run', label: 'not run' },
	running: { text: 'outcome:building', label: 'building' },
	waiting: { text: 'outcome:waiting', label: 'waiting' },
	unfinished: { text: 'outcome:failed,cancelled,not-run', label: "what didn't succeed" }
} satisfies Record<string, QuickFilter>;

export const QUICK_LIST: QuickFilter[] = Object.values(QUICK);
