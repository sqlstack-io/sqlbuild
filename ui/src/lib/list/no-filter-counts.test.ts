// Keeps filter option counts cheap: a count worked out by filtering the whole list once per option is
// quadratic when the options grow with the project (one per node or folder), and took Checks 12–19 s
// to open on a large project. Count in one pass with `countBy` (count-by.ts) instead. A fixed, small
// option list may keep a filter if the line (or the line before) says why, e.g.
//   <!-- count-allow: three fixed options over a short list -->
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = new URL('../../', import.meta.url).pathname;

/** A count written as a filter over a list: `count: all.filter(…)`. */
const COUNT_FILTER = /\bcount:\s*[\w$.?]+\.filter\(/g;

/** The `options={…}` attributes in a Svelte file, as [offset, expression]. */
function optionAttributes(source: string): [number, string][] {
	const found: [number, string][] = [];
	for (const m of source.matchAll(/\boptions=\{/g)) {
		let depth = 1;
		let i = m.index + m[0].length;
		for (; i < source.length && depth; i++) depth += source[i] === '{' ? 1 : source[i] === '}' ? -1 : 0;
		found.push([m.index, source.slice(m.index, i)]);
	}
	return found;
}

/** Line numbers of filter-counted options in `source`, without those excused by `count-allow: reason`. */
export function filterCounts(source: string): number[] {
	const lines = source.split('\n');
	const lineOf = (index: number) => source.slice(0, index).split('\n').length;
	const excused = (line: number) => [lines[line - 1], lines[line - 2]].some((l) => /count-allow:\s*\S/.test(l ?? ''));
	const at = new Set<number>();
	for (const m of source.matchAll(COUNT_FILTER)) at.add(lineOf(m.index));
	for (const [offset, expr] of optionAttributes(source)) {
		const m = /\.filter\([^]*?\)\.length/.exec(expr);
		if (m) at.add(lineOf(offset + m.index));
	}
	return [...at].filter((line) => !excused(line)).sort((a, b) => a - b);
}

function svelteFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? svelteFiles(path) : path.endsWith('.svelte') ? [path] : [];
	});
}

describe('filter option counts', () => {
	it('flags a count filtered from the whole list per option, unless excused', () => {
		expect(filterCounts('<F options={nodes.map((n) => ({ value: n, count: all.filter((c) => c.node === n).length }))} />')).toEqual([1]);
		expect(filterCounts("const o = $derived(kinds.map((k) => ({ value: k, count: nodes.filter((n) => n.kind === k).length })));")).toEqual([1]);
		expect(filterCounts('<F options={[{ value: 1, n: all.filter((c) => c.a).length }]} />')).toEqual([1]);
		expect(filterCounts('<F options={nodes.map((n) => ({ value: n, count: byNode.get(n) ?? 0 }))} />')).toEqual([]);
		expect(filterCounts('<!-- count-allow: two fixed options -->\n<F options={[{ value: 1, count: all.filter((c) => c.a).length }]} />')).toEqual([]);
		expect(filterCounts('<!-- count-allow -->\n<F options={[{ value: 1, count: all.filter((c) => c.a).length }]} />')).toEqual([2]);
	});

	it('finds none in the UI', () => {
		const found = svelteFiles(SRC).flatMap((path) => filterCounts(readFileSync(path, 'utf8')).map((line) => `${relative(SRC, path)}:${line}`));
		expect(found, 'Count filter options with countBy (lib/list/count-by.ts), or mark a fixed small list with count-allow: <reason>').toEqual([]);
	});
});
