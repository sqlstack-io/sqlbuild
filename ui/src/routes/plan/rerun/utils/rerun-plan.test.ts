import { describe, expect, it } from 'vitest';
import type { RerunMode, RunDetail } from '$lib/api';
import { buildRequest, copyCommand, type Selection } from '$lib/selection/command';
import { rerunRows } from '../../../runs/[id]/utils/rerun-rows';
import type { RunFailure } from '../../../runs/[id]/utils/run-failures';
import { openingText, presetText, rerunItems } from './rerun-plan';

const RUN = '0192c1a4-7d3e-7b51-9a0c-5f1e2d3c4b5a';

const failure = (kind: RunFailure['kind'], name: string, where: string | null, nodeKey: string | null): RunFailure => ({
	id: `${kind}:${name}`,
	kind,
	name,
	where,
	code: null,
	message: null,
	cause: null,
	statementId: null,
	nodeKey,
	checkId: null,
	logQuery: ''
});

const FAILURES: RunFailure[] = [failure('check', 'a_matches_c', 'model:c', 'model:c'), failure('node', 'b', null, 'model:b')];

const DETAIL = {
	nodes: [
		{ nodeKey: 'model:a', status: 'success' },
		{ nodeKey: 'model:b', status: 'failed' },
		{ nodeKey: 'model:d', status: 'skipped' }
	]
} as unknown as RunDetail;

/** A POSIX shell's words for a command `shellWord` quoted: single quotes, with `'\''` for a quote inside. */
const shellWords = (command: string): string[] => (command.match(/(?:'(?:[^']|'\\'')*'|\S)+/g) ?? []).map((word) => word.replace(/^'(.*)'$/s, '$1').replaceAll(`'\\''`, "'"));

const PRESETS = rerunRows({ failures: FAILURES, unfinished: ['model:b', 'model:d'], total: 4, jobKey: 'nightly' }).map((row) => presetText(row, RUN));
const OPENED = (['all', 'from_failure', 'selected'] as RerunMode[]).map((mode) => openingText(mode, ['a_matches_c', 'model:b'], RUN, FAILURES));

describe('rerun plan', () => {
	it.each<{ description: string; selection: Selection }>([
		...PRESETS.map((select) => ({ description: `preset ${select}`, selection: { select, exclude: '' } })),
		...OPENED.map((select) => ({ description: `opened on ${select}`, selection: { select, exclude: '' } })),
		{ description: 'an edited selection with an exclusion and a glob', selection: { select: `failed:${RUN.slice(0, 8)}+ stg_*`, exclude: `skipped:${RUN} audit:it's` } }
	])('submits exactly the command it shows: $description', ({ selection }) => {
		expect(shellWords(copyCommand(selection))).toEqual(['sqb', ...buildRequest(selection).command]);
	});

	it.each<{ description: string; mode: RerunMode; picked: string[]; expected: string }>([
		{ description: 'Rerun opens on what did not succeed', mode: 'from_failure', picked: [], expected: `failed:${RUN}+` },
		{ description: 'rerunning the whole run opens on the run', mode: 'all', picked: [], expected: `run:${RUN}` },
		{ description: 'picked audits and nodes open as their own selectors', mode: 'selected', picked: ['a_matches_c', 'model:b'], expected: 'audit:a_matches_c model:b' }
	])('$description', ({ mode, picked, expected }) => {
		expect(openingText(mode, picked, RUN, FAILURES)).toBe(expected);
	});

	it('lists the audits the selection resolves to before its nodes, with each one’s last result', () => {
		expect(rerunItems(['audit:a_matches_c:model:c', 'model:b', 'model:d'], DETAIL, FAILURES).map((i) => [i.kind, i.name, i.where, i.last])).toEqual([
			['audit', 'a_matches_c', 'model:c', 'failed'],
			['node', 'b', null, 'failed'],
			['node', 'd', null, 'skipped']
		]);
	});
});
