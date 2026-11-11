// Splits the plan page's command line into the tokens the server checks, and
// produces autocomplete suggestions from the command table and the snapshot's
// nodes. The server resolves selectors and refuses flags; nothing here decides
// whether a command may run. Pure: no side effects.

import type { CursorWindow, LaunchVerb, SnapshotNode } from '$lib/api';
import { flagSpec, VERBS, type FlagSpec } from './command-table';

/** The command line read for the panel's own controls: verb tabs, flag hints and verb switching. */
export type CommandParts = {
	verb: LaunchVerb;
	select: string[];
	exclude: string[];
	/** Other flags exactly as typed, e.g. `{ '--full-refresh': true, '--max-microbatches': '4' }`. */
	flags: Record<string, string | true>;
};

const SELECTOR_FLAGS = new Set(['--select', '-s', '--exclude']);

export function tokenize(text: string): string[] {
	return text.trim().split(/\s+/).filter(Boolean);
}

/** The argument tokens after `sqb`, exactly as the server receives them. */
export function commandTokens(text: string): string[] {
	const tokens = tokenize(text);
	return tokens[0] === 'sqb' ? tokens.slice(1) : tokens;
}

export function parseCommand(text: string): CommandParts {
	const tokens = commandTokens(text);
	const verbName = tokens.shift() ?? 'build';
	const verb = VERBS.find((v) => v.name === verbName)?.name ?? 'build';
	const select: string[] = [];
	const exclude: string[] = [];
	const flags: Record<string, string | true> = {};
	let mode: 'select' | 'exclude' = 'select';
	for (let i = 0; i < tokens.length; i++) {
		const token = tokens[i];
		if (token.startsWith('-')) {
			if (SELECTOR_FLAGS.has(token)) {
				mode = token === '--exclude' ? 'exclude' : 'select';
				continue;
			}
			mode = 'select';
			const spec = flagSpec(verb, token);
			const value = tokens[i + 1];
			if (!spec || spec.type === 'bool' || value === undefined || value.startsWith('-')) flags[spec?.flag ?? token] = true;
			else {
				flags[spec.flag] = value;
				i++;
			}
			continue;
		}
		if (mode === 'exclude') exclude.push(token);
		else select.push(token);
	}
	return { verb, select, exclude, flags };
}

export function buildCommand(parts: CommandParts): string {
	const out = ['sqb', parts.verb];
	if (parts.select.length) out.push('--select', ...parts.select);
	if (parts.exclude.length) out.push('--exclude', ...parts.exclude);
	for (const [flag, value] of Object.entries(parts.flags)) out.push(value === true ? flag : `${flag} ${value}`);
	return out.join(' ');
}

/** Switches the verb, keeping the selection and the flags the new verb accepts. */
export function withVerb(text: string, verb: LaunchVerb): string {
	const parts = parseCommand(text);
	const flags = Object.fromEntries(Object.entries(parts.flags).filter(([flag]) => flagSpec(verb, flag)));
	return buildCommand({ ...parts, verb, flags }) + ' ';
}

export type Suggestion = { value: string; label: string; hint: string };

export type TokenContext = { start: number; end: number; text: string; previousFlag: string | null };

export function tokenAt(text: string, caret: number): TokenContext {
	const before = text.slice(0, caret);
	const start = before.search(/\S*$/);
	const after = text.slice(caret).search(/\s|$/);
	const end = caret + (after < 0 ? 0 : after);
	const previous = tokenize(before.slice(0, start)).reverse();
	const lastFlag = previous.find((t) => t.startsWith('-')) ?? null;
	return { start, end, text: text.slice(start, caret), previousFlag: lastFlag };
}

/** The plan page for a command, opened by every Run button before anything runs; with `job`, a run of that job narrowed by the command. */
export function planHref(selection: string[] = [], verb: LaunchVerb = 'build', narrowing: { job?: string; exclude?: string[] } = {}): string {
	const command = buildCommand({ verb, select: selection.map(selectorOf), exclude: (narrowing.exclude ?? []).map(selectorOf), flags: {} });
	return `/plan?cmd=${encodeURIComponent(command)}${narrowing.job ? `&job=${encodeURIComponent(narrowing.job)}` : ''}`;
}

/** An RFC 3339 timestamp with a zone, the form the server reads a job run's window in. */
const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

/** The cursor-window flags of each cursor type, as `sqb build` names them. */
const WINDOW_FLAGS = {
	timestamp: ['--start-cursor-ts', '--end-cursor-ts'],
	integer: ['--start-cursor-int', '--end-cursor-int']
} as const;

/** The options a job run adds to the job's own settings: a rebuild from scratch, or a cursor window. */
export const JOB_RUN_FLAGS: readonly string[] = ['--full-refresh', ...WINDOW_FLAGS.timestamp, ...WINDOW_FLAGS.integer];

/** The command's cursor window, from its window flags, or null when it has none. */
export function windowOf(parts: CommandParts): CursorWindow | null {
	for (const type of ['timestamp', 'integer'] as const) {
		const [startFlag, endFlag] = WINDOW_FLAGS[type];
		const start = parts.flags[startFlag];
		const end = parts.flags[endFlag];
		if (start !== undefined || end !== undefined) return { type, start: typeof start === 'string' ? start : null, end: typeof end === 'string' ? end : null };
	}
	return null;
}

/** The command with its cursor window replaced by `window`, or removed when it is null. */
export function withWindow(text: string, window: CursorWindow | null): string {
	const parts = parseCommand(text);
	const flags = Object.fromEntries(Object.entries(parts.flags).filter(([flag]) => flag === '--full-refresh' || !JOB_RUN_FLAGS.includes(flag)));
	if (window) {
		const [startFlag, endFlag] = WINDOW_FLAGS[window.type];
		if (window.start) flags[startFlag] = window.start;
		if (window.end) flags[endFlag] = window.end;
	}
	return buildCommand({ ...parts, flags });
}

/** The command with `--full-refresh` set or removed. */
export function withFullRefresh(text: string, on: boolean): string {
	const parts = parseCommand(text);
	const flags = Object.fromEntries(Object.entries(parts.flags).filter(([flag]) => flag !== '--full-refresh'));
	if (on) flags['--full-refresh'] = true;
	return buildCommand({ ...parts, flags });
}

/** Options a job run takes, for telling an option still being typed from one it can't take. */
const JOB_OPTIONS = ['--select', '--exclude', ...JOB_RUN_FLAGS];

const unfinished = (flag: string) => !JOB_RUN_FLAGS.includes(flag) && JOB_OPTIONS.some((o) => o.startsWith(flag) && o !== flag);

/**
 * A job run's command without options still being typed, such as `--` or `--sel`, so typing one neither
 * reports a problem nor replans; `typing` says one is there, which holds Run until it is finished.
 */
export function settledJobCommand(parts: CommandParts): { parts: CommandParts; typing: boolean } {
	const flags = Object.fromEntries(Object.entries(parts.flags).filter(([flag]) => !unfinished(flag)));
	return { parts: { ...parts, flags }, typing: Object.keys(flags).length !== Object.keys(parts.flags).length };
}

/** The `sqb build` command that selects what a narrowed run of `job` builds: each selector intersected with the job. */
export function jobBuildTokens(job: string, parts: CommandParts): string[] {
	const out = ['build', '--select', ...(parts.select.length ? parts.select.map((s) => `job:${job},${s}`) : [`job:${job}`])];
	if (parts.exclude.length) out.push('--exclude', ...parts.exclude);
	for (const [flag, value] of Object.entries(parts.flags)) out.push(...(value === true ? [flag] : [flag, value]));
	return out;
}

/** Why the command cannot narrow a job run, or null. */
export function jobRunProblem(parts: CommandParts): string | null {
	if (parts.verb !== 'build') return `A job run is a build; remove the job to run sqb ${parts.verb}.`;
	const other = Object.keys(parts.flags).filter((f) => !JOB_RUN_FLAGS.includes(f));
	if (other.length) return `A job run uses the job's own settings; ${other.join(', ')} can't be added. Remove the job to run it as a command.`;
	const window = windowOf(parts);
	if (window && parts.flags['--full-refresh']) return 'A run rebuilds from scratch or reprocesses a date range, not both.';
	if (window && !(window.start && window.end)) return 'Give the range a start and an end.';
	const valid = (v: string | null) => (window?.type === 'integer' ? /^-?\d+$/.test(v ?? '') && Number.isSafeInteger(Number(v)) : TIMESTAMP.test(v ?? ''));
	if (window && ![window.start, window.end].every(valid)) {
		return window.type === 'integer' ? 'Range bounds are whole numbers.' : 'Range bounds are full timestamps, such as 2026-11-03T00:00:00Z.';
	}
	return null;
}

/** How `sqb` names a node in a selector: seeds and sources by kind, everything else by bare name. */
export function selectorOf(key: string): string {
	return key.startsWith('seed:') || key.startsWith('source:') ? key : key.slice(key.indexOf(':') + 1);
}

function selectorPool(nodes: SnapshotNode[]): { value: string; hint: string }[] {
	const unique = (values: string[]) => [...new Set(values)].sort();
	return [
		...unique(nodes.flatMap((n) => n.tags)).map((t) => ({ value: `tag:${t}`, hint: 'tag' })),
		...unique(nodes.filter((n) => n.path.includes('/')).map((n) => n.path.replace(/\/[^/]+$/, ''))).map((p) => ({ value: `path:${p}`, hint: 'folder' })),
		...nodes.map((n) => ({ value: selectorOf(n.key), hint: n.kind }))
	];
}

export function suggest(text: string, caret: number, nodes: SnapshotNode[]): Suggestion[] {
	const parts = parseCommand(text);
	const ctx = tokenAt(text, caret);
	const verb = VERBS.find((v) => v.name === parts.verb)!;
	const tokens = tokenize(text);
	if (ctx.text.startsWith('-')) {
		return verb.flags
			.filter((f) => f.flag.startsWith(ctx.text) && (f.repeatable || !tokens.includes(f.flag) || ctx.text === f.flag))
			.slice(0, 14)
			.map((f) => ({ value: f.flag, label: f.flag, hint: `${f.type}${f.requires ? ` · needs ${f.requires}` : ''} · ${f.description}` }));
	}
	const spec: FlagSpec | undefined = ctx.previousFlag ? flagSpec(parts.verb, ctx.previousFlag) : undefined;
	const prevToken = tokenize(text.slice(0, ctx.start)).at(-1) ?? '';
	if (spec && spec.type !== 'selector' && prevToken === ctx.previousFlag) {
		if (spec.choices) return spec.choices.filter((c) => c.startsWith(ctx.text)).map((c) => ({ value: c, label: c, hint: spec.flag }));
		if (spec.type === 'timestamp' && !ctx.text) return [{ value: new Date().toISOString().slice(0, 10) + 'T00:00:00Z', label: 'today 00:00 UTC', hint: 'timestamp' }];
		return [];
	}
	const inSelector = !ctx.previousFlag || SELECTOR_FLAGS.has(ctx.previousFlag);
	if (!inSelector || !ctx.text) return [];
	const match = ctx.text.match(/^(\+?)(.*?)(\+?)$/);
	const [, pre = '', body = '', post = ''] = match ?? [];
	if (!body) return [];
	return selectorPool(nodes)
		.filter((p) => p.value.toLowerCase().includes(body.toLowerCase()))
		.slice(0, 12)
		.map((p) => ({ value: `${pre}${p.value}${post}`, label: `${pre}${p.value}${post}`, hint: p.hint }));
}

/** Inserts `value` in place of the token at the caret; returns the new text and caret. */
export function applySuggestion(text: string, caret: number, value: string): { text: string; caret: number } {
	const ctx = tokenAt(text, caret);
	const next = `${text.slice(0, ctx.start)}${value} ${text.slice(ctx.end).replace(/^\s+/, '')}`;
	return { text: next, caret: ctx.start + value.length + 1 };
}
