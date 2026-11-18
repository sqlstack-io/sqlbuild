// `crypto.randomUUID` and `navigator.clipboard` exist only in secure contexts (HTTPS or localhost), and the
// UI is also served over plain HTTP behind a proxy, where calling them breaks every action that makes an
// ID or copies text. Use `newRequestId` (request-id.ts) and `copyText` (clipboard.ts), which fall back.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = new URL('../../', import.meta.url).pathname;
const BANNED = /\bcrypto\.randomUUID\b|\bnavigator\.clipboard\b/;
const HELPERS = new Set(['lib/browser/request-id.ts', 'lib/browser/clipboard.ts']);

function files(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return files(path);
		return /\.(ts|svelte|js)$/.test(name) && !/\.test\.ts$/.test(name) ? [path] : [];
	});
}

describe('secure-context-only browser APIs', () => {
	it('are called only from the helpers that fall back without them', () => {
		const found = files(SRC).flatMap((path) => {
			const file = relative(SRC, path);
			if (HELPERS.has(file)) return [];
			return readFileSync(path, 'utf8')
				.split('\n')
				.flatMap((line, i) => (BANNED.test(line) ? [`${file}:${i + 1}: ${line.trim()}`] : []));
		});
		expect(found, 'use newRequestId or copyText from $lib/browser').toEqual([]);
	});
});
