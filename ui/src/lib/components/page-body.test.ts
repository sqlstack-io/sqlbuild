// Every page scrolls in the shared PageBody (or a list's DataTable), whose scroller has no padding, so
// sticky headers stick to its very top. A page building its own padded scroller fails here.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const ROUTES = new URL('../../routes/', import.meta.url).pathname;

function svelteFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? svelteFiles(path) : path.endsWith('.svelte') ? [path] : [];
	});
}

/** A page-sized scroller written by hand: a flex-1 box that scrolls, as a class list. */
const HAND_SCROLLER = /class="[^"]*\bflex-1\b[^"]*\boverflow-(?:y-)?auto\b|class="[^"]*\boverflow-(?:y-)?auto\b[^"]*\bflex-1\b/;

describe('page bodies', () => {
	it('are the shared PageBody, not a hand-built scroller', () => {
		const found = svelteFiles(ROUTES).flatMap((path) => {
			const source = readFileSync(path, 'utf8');
			return source.split('\n').flatMap((text, i) => (HAND_SCROLLER.test(text) ? [`${relative(ROUTES, path)}:${i + 1}`] : []));
		});
		expect(found, 'Use PageBody from $lib/components/page-body.svelte').toEqual([]);
	});
});
