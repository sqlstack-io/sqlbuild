// Keeps buttons consistent: every button is the shared `Button` (or a shared control built on a raw
// <button>, listed below), at its fixed set of types and sizes. A raw <button> anywhere else fails, as
// does a menu, popover or tooltip trigger not styled with `buttonVariants`, unless the line before it
// explains why, e.g.
//   <!-- raw-button: a graph node that is clickable as a whole -->
// Button labels use Lucide icons, never typed symbols such as ↻ or ▷.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = new URL('../../../../', import.meta.url).pathname;

/** Shared controls that own a raw <button> and its look; pages use these instead. Everything under ui/ is shared. */
const SHARED = new Set([
	'lib/components/disclosure-button.svelte',
	'lib/components/copy-button.svelte',
	'lib/components/list/filter-button.svelte',
	'lib/components/list/fitting-items.svelte',
	'lib/components/list/choice-filter.svelte',
	'lib/components/selection/selection-chip.svelte',
	'lib/components/help-tip.svelte',
	'lib/components/shell/scope-switcher.svelte',
	'lib/components/list/segmented-filter.svelte',
	'lib/components/platform/page-tabs.svelte'
]);
/** Shared classes from layout.css that style a raw <button> as a shared control. */
const SHARED_CLASSES = ['sb-toggle'];
/** Typed symbols that stand in for icons; `→` in a link's text is fine. */
const isShared = (file: string) => file.startsWith('lib/components/ui/') || SHARED.has(file);
const TYPED_SYMBOLS = /[↻↺▷▶▸▾▴◂＋✕×⋯↗✓✔−⤢⌕]/;

function svelteFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? svelteFiles(path) : path.endsWith('.svelte') ? [path] : [];
	});
}

/** The end of the tag that starts at `start`, skipping `>` inside `{…}` expressions and quotes. */
function tagEnd(source: string, start: number): number {
	let depth = 0;
	let quote: string | null = null;
	for (let i = start; i < source.length; i++) {
		const ch = source[i];
		if (quote) {
			if (ch === quote) quote = null;
		} else if (ch === '"' || ch === "'" || (ch === '`' && depth > 0)) quote = ch;
		else if (ch === '{') depth++;
		else if (ch === '}') depth--;
		else if (ch === '>' && depth === 0) return i;
	}
	return source.length;
}

/** Whether a `<!-- raw-button: reason -->` comment comes right before `index`, past any `{#if …}`-style block tags. */
function excused(source: string, index: number): boolean {
	let before = source.slice(0, index);
	for (;;) {
		const trimmed = before.replace(/\s+$/, '').replace(/\{[#:][^{}]*\}$/, '');
		if (trimmed === before) break;
		before = trimmed;
	}
	return /<!--\s*raw-button:\s*\S[\s\S]*?-->$/.test(before);
}

const line = (source: string, index: number) => source.slice(0, index).split('\n').length;

type Finding = { file: string; line: number; tag: string };

function scan(): { raw: Finding[]; symbols: Finding[] } {
	const raw: Finding[] = [];
	const symbols: Finding[] = [];
	for (const path of svelteFiles(SRC)) {
		const file = relative(SRC, path);
		const source = readFileSync(path, 'utf8');
		for (const match of source.matchAll(/<([A-Za-z]+\.Trigger)[\s>]/g)) {
			const start = match.index;
			const tag = source.slice(start, tagEnd(source, start) + 1);
			if (isShared(file) || !/\bclass=/.test(tag) || tag.includes('buttonVariants(') || excused(source, start)) continue;
			raw.push({ file, line: line(source, start), tag: tag.replace(/\s+/g, ' ').slice(0, 100) });
		}
		for (const match of source.matchAll(/<(button|Button|GatedButton)[\s>]/g)) {
			const start = match.index;
			const end = tagEnd(source, start);
			const tag = source.slice(start, end + 1);
			const close = source.indexOf(`</${match[1]}>`, end);
			const label = close === -1 ? '' : source.slice(end + 1, close).replace(/\{[^{}]*\}/g, '');
			if (TYPED_SYMBOLS.test(label)) symbols.push({ file, line: line(source, start), tag: label.trim() });
			if (match[1] !== 'button' || isShared(file)) continue;
			const classes = /\bclass="([^"]*)"/.exec(tag)?.[1].split(/\s+/) ?? [];
			if (SHARED_CLASSES.some((c) => classes.includes(c))) continue;
			if (!excused(source, start)) raw.push({ file, line: line(source, start), tag: tag.replace(/\s+/g, ' ').slice(0, 100) });
		}
	}
	return { raw, symbols };
}

describe('buttons', () => {
	const { raw, symbols } = scan();

	it('are the shared Button, a shared control, or an explained exception', () => {
		const report = raw.map((f) => `${f.file}:${f.line}  ${f.tag}`);
		expect(report, 'Use Button from $lib/components/ui/button, or put `<!-- raw-button: why -->` on the line before').toEqual([]);
	});

	it('use Lucide icons, not typed symbols, in their labels', () => {
		expect(symbols.map((f) => `${f.file}:${f.line}  ${f.tag}`)).toEqual([]);
	});

	it('finds raw buttons and honours exceptions (self-test)', () => {
		const sample = '<div>\n<!-- raw-button: a graph node -->\n{#if a}<button onclick={() => (x = y > 1)}>A</button>{/if}\n<button class="x">B</button>\n</div>';
		const starts = [...sample.matchAll(/<button[\s>]/g)].map((m) => m.index);
		expect(starts.map((i) => excused(sample, i))).toEqual([true, false]);
		expect(sample.slice(starts[0], tagEnd(sample, starts[0]) + 1)).toBe('<button onclick={() => (x = y > 1)}>');
	});
});
