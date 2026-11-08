// Splits the run panel's command line into the tokens the server checks, and
// produces autocomplete suggestions from the command table and the snapshot's
// nodes. The server resolves selectors and refuses flags; nothing here decides
// whether a command may run. Pure: no side effects.

import type { LaunchVerb, SnapshotNode } from '$lib/api';
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
