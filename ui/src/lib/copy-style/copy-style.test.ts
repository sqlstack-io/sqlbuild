// The copy check over the UI's source (see ui/COPY.md). Report-only for now: it logs every finding
// and fails only on a finding missing from baseline.txt, or a baseline entry that no longer occurs,
// so the baseline can only shrink. `COPY_BASELINE=write npx vitest run src/lib/copy-style` rewrites it.
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';
import { baselineKey, findings, type Finding } from './check';

const SRC = new URL('../../', import.meta.url).pathname;
const BASELINE = new URL('./baseline.txt', import.meta.url).pathname;

/** Source the copy check reads: routes and lib, without tests, mocks, the API layer or UI kit primitives. */
function files(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		const rel = relative(SRC, path);
		if (statSync(path).isDirectory()) return /^lib\/(api|components\/ui|copy-style)$|\/mock$/.test(rel) ? [] : files(path);
		return /\.(svelte|ts)$/.test(name) && !/\.test\.ts$/.test(name) ? [path] : [];
	});
}

const isLabelFile = (rel: string) => /(^|\/)labels\.ts$|^lib\/status\.ts$|^lib\/platform\/maintenance\.ts$/.test(rel);

function scan(): { all: Finding[]; allowed: number } {
	let allowed = 0;
	const all = files(SRC).flatMap((path) => {
		const rel = relative(SRC, path);
		const result = findings(rel, readFileSync(path, 'utf8'), isLabelFile(rel));
		allowed += result.allowed;
		return result.findings;
	});
	return { all, allowed };
}

describe('copy', () => {
	const { all, allowed } = scan();
	const keys = [...new Set(all.map(baselineKey))].sort();

	it('has no finding outside the baseline, and the baseline only shrinks', () => {
		if (process.env.COPY_BASELINE === 'write') writeFileSync(BASELINE, `${keys.join('\n')}\n`);
		const baseline = new Set(readFileSync(BASELINE, 'utf8').split('\n').filter(Boolean));
		console.info(`copy check: ${keys.length} findings in the baseline's terms, ${allowed} allowed by copy-allow`);
		for (const f of all) console.info(`  ${f.file}:${f.line}  [${f.kind}] ${f.rule}: ${f.text}`);
		expect(keys.filter((k) => !baseline.has(k)), 'New copy findings: fix them, or mark a deliberate exception with copy-allow: <reason>').toEqual([]);
		expect([...baseline].filter((k) => !keys.includes(k)), 'Fixed findings still in baseline.txt: remove them so the baseline shrinks').toEqual([]);
	});
});
