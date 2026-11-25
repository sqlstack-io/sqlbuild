// A selection outside the bar: in the URL, as a shell command to copy, and as the plan page's
// command line. Pure.

import { parseSelection } from './syntax';
import type { AdhocRunRequest, SelectionProblem } from '$lib/api';

export const SELECT_PARAM = 'select';
export const EXCLUDE_PARAM = 'exclude';

export type Selection = { select: string; exclude: string };

/** The selection in a URL: `?select=…&exclude=…`, each repeated parameter adding tokens. A link with one node name, as before, selects that node. */
export function readSelectionParams(params: URLSearchParams): Selection {
	const all = (key: string) => params.getAll(key).join(' ').split(/\s+/).filter(Boolean).join(' ');
	return { select: all(SELECT_PARAM), exclude: all(EXCLUDE_PARAM) };
}

/** `params` with the selection, or without it when empty. */
export function writeSelectionParams(params: URLSearchParams, s: Selection): URLSearchParams {
	const next = new URLSearchParams(params);
	next.delete(SELECT_PARAM);
	next.delete(EXCLUDE_PARAM);
	if (s.select.trim()) next.set(SELECT_PARAM, s.select.trim());
	if (s.exclude.trim()) next.set(EXCLUDE_PARAM, s.exclude.trim());
	return next;
}

/** A token quoted for a POSIX shell when it holds a character the shell would read. */
export function shellWord(token: string): string {
	return /^[\w.:/,+@=%-]+$/.test(token) && !token.startsWith('~') ? token : `'${token.replaceAll("'", `'\\''`)}'`;
}

/** The ad-hoc run of `sqb build -s … [--exclude …]`: the argument tokens after `sqb`, as the server receives them. */
export function buildRequest(s: Selection): AdhocRunRequest {
	const words = (text: string) => parseSelection(text).map((t) => t.raw);
	const command = ['build'];
	if (s.select.trim()) command.push('-s', ...words(s.select));
	if (s.exclude.trim()) command.push('--exclude', ...words(s.exclude));
	return { command, priority: 0 };
}

/** `sqb build -s … [--exclude …]` to paste into a shell: exactly the tokens {@link buildRequest} submits, quoted. */
export function copyCommand(s: Selection): string {
	return ['sqb', ...buildRequest(s).command.map(shellWord)].join(' ');
}

/** The plan page with `sqb build -s … [--exclude …]` filled in; its command line splits on spaces, so nothing is quoted. */
export function buildHref(s: Selection): string {
	const out = ['sqb', 'build'];
	if (s.select.trim()) out.push('-s', s.select.trim());
	if (s.exclude.trim()) out.push('--exclude', s.exclude.trim());
	return `/plan?cmd=${encodeURIComponent(out.join(' '))}`;
}

const NAMED: Record<string, string> = { tag: 'tag', job: 'job', kind: 'node kind', source: 'source', seed: 'seed', test: 'test' };

/**
 * The problem in the design's words, from its structured fields: "No tag called finanse.", "No node
 * called orders.", "No folder models/x.". Without them (a selection refused as a whole) it is `sqb`'s
 * own message.
 */
export function problemText(problem: SelectionProblem): string {
	const r = problem.refused;
	if (!r) return problem.message;
	if (r.type === 'path' || (r.type === null && r.value.includes('/'))) return `No folder ${r.value}.`;
	if (/[*?[]/.test(r.value)) return `Nothing matches ${r.type ? `${r.type}:` : ''}${r.value}.`;
	if (r.type === null) return `No node called ${r.value}.`;
	return NAMED[r.type] ? `No ${NAMED[r.type]} called ${r.value}.` : `Unknown selector type ${r.type}.`;
}

/** The token a refused selection points at, by index in its field's tokens; `null` when it names none. */
export function problemToken(problem: SelectionProblem | null, field: 'select' | 'exclude', text: string): number | null {
	const at = problem?.location;
	if (!at || at.field !== field) return null;
	const i = parseSelection(text).findIndex((t) => at.start < t.end && at.end > t.start);
	return i < 0 ? null : i;
}

/** Which parts of a refused token the problem is about: the refused part when known, else those the message quotes, else all of them. */
export function problemParts(problem: SelectionProblem, token: string): number[] {
	const parts = parseSelection(token)[0]?.parts ?? [];
	if (problem.refused && problem.refused.part < parts.length) return [problem.refused.part];
	const named = parts.flatMap((p, i) => (problem.message.includes(`'${p.value}'`) || problem.message.includes(`'${p.raw}'`) ? [i] : []));
	return named.length ? named : parts.map((_, i) => i);
}
