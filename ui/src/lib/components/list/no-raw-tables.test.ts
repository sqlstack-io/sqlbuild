// Keeps lists consistent: every list is the shared DataTable, which fits the page, keeps a sticky
// header, windows long lists and truncates cells the same way everywhere. A hand-built <table>
// anywhere else fails, unless the line before it explains why it isn't a list, e.g.
//   <!-- raw-table: a permission matrix grouped by risk, not a list of records -->
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = new URL('../../../', import.meta.url).pathname;

/** The shared table, and the UI kit's own primitives, own a raw <table>. */
const isShared = (file: string) => file === 'lib/components/list/data-table.svelte' || file.startsWith('lib/components/ui/');

function svelteFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? svelteFiles(path) : path.endsWith('.svelte') ? [path] : [];
	});
}

/** Whether a `<!-- raw-table: reason -->` comment comes right before `index`, past any `{#if …}`-style block tags. */
function excused(source: string, index: number): boolean {
	let before = source.slice(0, index);
	for (;;) {
		const trimmed = before.replace(/\s+$/, '').replace(/\{[#:][^{}]*\}$/, '');
		if (trimmed === before) break;
		before = trimmed;
	}
	return /<!--\s*raw-table:\s*\S[\s\S]*?-->$/.test(before);
}

const line = (source: string, index: number) => source.slice(0, index).split('\n').length;

function scan(): string[] {
	const found: string[] = [];
	for (const path of svelteFiles(SRC)) {
		const file = relative(SRC, path);
		if (isShared(file)) continue;
		const source = readFileSync(path, 'utf8');
		for (const match of source.matchAll(/<table[\s>]/g)) {
			if (!excused(source, match.index)) found.push(`${file}:${line(source, match.index)}`);
		}
	}
	return found;
}

describe('list tables', () => {
	it('are the shared DataTable, or an explained exception', () => {
		expect(scan(), 'Use DataTable from $lib/components/list, or put `<!-- raw-table: why -->` on the line before').toEqual([]);
	});

	it('finds raw tables and honours exceptions (self-test)', () => {
		const sample = '<div>\n<!-- raw-table: a key/value list -->\n{#if a}<table class="kv">{/if}\n<table class="sb-list">\n</div>';
		const starts = [...sample.matchAll(/<table[\s>]/g)].map((m) => m.index);
		expect(starts.map((i) => excused(sample, i))).toEqual([true, false]);
	});
});
