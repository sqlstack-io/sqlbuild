// Keeps every graph the same graph: the library is drawn only by GraphCanvas (graph-canvas.svelte),
// which brings the shared background, fit controls, readable fit and first-fit hiding, and pages put
// GraphToolbar over it for Lineage's controls. A page that draws <SvelteFlow> itself, or imports it,
// fails here.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = new URL('../../../', import.meta.url).pathname;
const CANVAS = 'lib/components/graph/graph-canvas.svelte';

/** Lines that draw or import the graph library's canvas. */
export function rawGraph(source: string): number[] {
	const lines = source.split('\n');
	return lines.flatMap((line, i) => (/<SvelteFlow\b/.test(line) || /import\s*\{[^}]*\bSvelteFlow\b[^}]*\}\s*from\s*'@xyflow\/svelte'/.test(line) ? [i + 1] : []));
}

function svelteFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? svelteFiles(path) : path.endsWith('.svelte') ? [path] : [];
	});
}

describe('graphs', () => {
	it('spots the library drawn or imported directly', () => {
		expect(rawGraph("import { SvelteFlow, type Node } from '@xyflow/svelte';\n<SvelteFlow bind:nodes />")).toEqual([1, 2]);
		expect(rawGraph("import type { Node } from '@xyflow/svelte';\n<GraphCanvas bind:nodes />")).toEqual([]);
	});

	it('are drawn only by GraphCanvas', () => {
		const found = svelteFiles(SRC).flatMap((path) => {
			const file = relative(SRC, path);
			return file === CANVAS ? [] : rawGraph(readFileSync(path, 'utf8')).map((line) => `${file}:${line}`);
		});
		expect(found, 'Draw graphs with GraphCanvas (lib/components/graph/graph-canvas.svelte), and GraphToolbar for the toolbar').toEqual([]);
	});
});
