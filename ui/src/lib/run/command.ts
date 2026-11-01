// Parses the run panel's command line into a launch request, and produces
// autocomplete suggestions from the command table and the snapshot's nodes.
// Pure: no side effects.

import type { AdhocRunRequest, LaunchVerb, SnapshotNode } from '$lib/api';
import { flagSpec, VERBS, type FlagSpec } from './command-table';

export type ParsedCommand = {
	request: AdhocRunRequest;
	/** Problems the panel reports before asking the server. */
	errors: string[];
};

const SELECTOR_FLAGS = new Set(['--select', '-s', '--exclude']);

export function tokenize(text: string): string[] {
	return text.trim().split(/\s+/).filter(Boolean);
}

export function parseCommand(text: string): ParsedCommand {
	const tokens = tokenize(text);
	const errors: string[] = [];
	if (tokens[0] === 'sqb') tokens.shift();
	const verbName = tokens.shift() ?? 'build';
	const verb = (VERBS.find((v) => v.name === verbName)?.name ?? 'build') as LaunchVerb;
	if (!VERBS.some((v) => v.name === verbName)) errors.push(`${verbName} can't be launched from the panel; use ${VERBS.map((v) => v.name).join(', ')}`);
	const select: string[] = [];
	const exclude: string[] = [];
	const flags: Record<string, string | true> = {};
	let mode: 'select' | 'exclude' | null = 'select';
	for (let i = 0; i < tokens.length; i++) {
		const token = tokens[i];
		if (token.startsWith('-')) {
			mode = null;
			if (token === '--target') {
				errors.push('The target is set by the location; remove --target');
				i++;
				continue;
			}
			if (SELECTOR_FLAGS.has(token)) {
				mode = token === '--exclude' ? 'exclude' : 'select';
				continue;
			}
			const spec = flagSpec(verb, token);
			if (!spec) {
				flags[token] = true;
				continue;
			}
			if (spec.type === 'bool') flags[spec.flag] = true;
			else {
				const value = tokens[i + 1];
				if (value === undefined || value.startsWith('--')) flags[spec.flag] = true;
				else {
					flags[spec.flag] = value;
					i++;
				}
			}
			continue;
		}
		if (mode === 'exclude') exclude.push(token);
		else if (mode === 'select') select.push(token);
		else errors.push(`Unexpected ${token}; selectors go after --select or --exclude`);
	}
	return { request: { verb, select, exclude, flags }, errors };
}

export function buildCommand(request: AdhocRunRequest): string {
	const parts = ['sqb', request.verb];
	if (request.select.length) parts.push('--select', ...request.select);
	if (request.exclude.length) parts.push('--exclude', ...request.exclude);
	for (const [flag, value] of Object.entries(request.flags)) parts.push(value === true ? flag : `${flag} ${value}`);
	return parts.join(' ');
}

/** Switches the verb, keeping the selection and the flags the new verb accepts. */
export function withVerb(text: string, verb: LaunchVerb): string {
	const { request } = parseCommand(text);
	const flags = Object.fromEntries(Object.entries(request.flags).filter(([flag]) => flagSpec(verb, flag)));
	return buildCommand({ ...request, verb, flags }) + ' ';
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

function selectorPool(nodes: SnapshotNode[]): { value: string; hint: string }[] {
	const unique = (values: string[]) => [...new Set(values)].sort();
	return [
		...unique(nodes.flatMap((n) => n.tags)).map((t) => ({ value: `tag:${t}`, hint: 'tag' })),
		...unique(nodes.map((n) => n.group)).map((g) => ({ value: `group:${g}`, hint: 'group' })),
		...unique(nodes.map((n) => n.kind)).map((k) => ({ value: `kind:${k}`, hint: 'kind' })),
		...unique(nodes.map((n) => n.path.replace(/\/[^/]+$/, '/'))).map((p) => ({ value: `path:${p}`, hint: 'path' })),
		...nodes.map((n) => ({ value: n.key, hint: n.kind }))
	];
}

export function suggest(text: string, caret: number, nodes: SnapshotNode[]): Suggestion[] {
	const { request } = parseCommand(text);
	const ctx = tokenAt(text, caret);
	const verb = VERBS.find((v) => v.name === request.verb)!;
	const tokens = tokenize(text);
	if (ctx.text.startsWith('-')) {
		return verb.flags
			.filter((f) => f.flag.startsWith(ctx.text) && (f.repeatable || !tokens.includes(f.flag) || ctx.text === f.flag))
			.slice(0, 14)
			.map((f) => ({ value: f.flag, label: f.flag, hint: `${f.type}${f.rewritesHistory ? ' · needs run.rewrite_history' : ''} · ${f.description}` }));
	}
	const spec: FlagSpec | undefined = ctx.previousFlag ? flagSpec(request.verb, ctx.previousFlag) : undefined;
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
