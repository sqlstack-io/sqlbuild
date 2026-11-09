import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// The UI imports its logos and favicon from the website's approved files through `$website`, so a
// branding change reaches both. A copy under ui/ would drift again, so none may exist.
const ui = join(import.meta.dirname, '..', '..');

const files = (dir: string): string[] =>
	readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
		entry.isDirectory() ? files(join(dir, entry.name)) : [join(dir, entry.name)]
	);

describe('branding', () => {
	it('keeps no image copies in the UI', () => {
		const images = [...files(join(ui, 'static')), ...files(join(ui, 'src'))].filter((path) => /\.(png|svg|ico|jpe?g|webp)$/i.test(path));
		expect(images).toEqual([]);
	});

	it('loads every logo and favicon from the website', () => {
		const references = files(join(ui, 'src'))
			.filter((path) => path.endsWith('.svelte') || path.endsWith('.html'))
			.flatMap((path) => [...readFileSync(path, 'utf8').matchAll(/(?:src|href)="([^"{]*(?:logo|favicon|icon)[^"]*)"/g)].map((match) => `${path}: ${match[1]}`));
		expect(references).toEqual([]);
	});
});
