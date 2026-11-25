// A rerun is a selection like any other: `failed:<run>+` reruns what didn't succeed, `audit:<name>` one
// audit, `run:<run>` the whole run. Presets only write that text; what runs is what the server resolves
// it to, and which run it came from is read from the text alone. Pure.

import type { RerunMode, RunDetail } from '$lib/api';
import type { SuggestionItem } from '$lib/selection/suggest';
import { nodeLabel } from '$lib/operate/labels';
import type { RerunRow } from '../../../runs/[id]/utils/rerun-rows';
import type { RunFailure } from '../../../runs/[id]/utils/run-failures';

export type RerunItem = { key: string; name: string; kind: 'audit' | 'node'; where: string | null; last: string | null };

/** The selection text a preset writes into the box. */
export function presetText(row: RerunRow, runId: string): string {
	if (row.mode === 'all') return `run:${runId}`;
	if (row.mode === 'from_failure') return `failed:${runId}+`;
	return row.nodes.map((n) => (row.key === 'check' ? `audit:${n}` : n)).join(' ');
}

/** The text Rerun opens the box on: `run:<run>` for the whole run, the picked nodes and `audit:<name>`s, else `failed:<run>+`. */
export function openingText(mode: RerunMode, picked: readonly string[], runId: string, failures: readonly RunFailure[]): string {
	const audits = new Set(failures.filter((f) => f.kind === 'check').map((f) => f.name));
	if (mode === 'all') return `run:${runId}`;
	if (mode === 'selected' && picked.length) return picked.map((n) => (audits.has(n) ? `audit:${n}` : n)).join(' ');
	return `failed:${runId}+`;
}

/** A written run ID (full, or a prefix of 8 or more) is this run's. */
export const namesRun = (written: string, runId: string) => written === runId || (written.length >= 8 && runId.startsWith(written));

/** The presets as the box suggests them: each one only writes its text, like any other suggestion. */
export function presetSuggestions(rows: readonly RerunRow[], runId: string): SuggestionItem[] {
	return rows.map((row) => ({ insert: presetText(row, runId), type: row.mode === 'all' ? 'run' : row.mode === 'from_failure' ? 'failed' : null, label: row.label, hint: row.count, complete: true }));
}

/** The rows a selection runs, read from the keys it resolves to: its audits, then its nodes, each with its last result in that run. */
export function rerunItems(keys: readonly string[], detail: RunDetail, failures: readonly RunFailure[]): RerunItem[] {
	const failed = new Map(failures.filter((f) => f.kind === 'check').map((f) => [f.name, f]));
	const status = new Map(detail.nodes.map((n) => [n.nodeKey, n.status]));
	const audits = keys.filter((key) => key.startsWith('audit:'));
	const nodes = keys.filter((key) => !key.startsWith('audit:'));
	return [
		...audits.map((key): RerunItem => {
			// `audit:<name>` runs after the build; `audit:<name>:<kind>:<target>[:<column>]` on its target.
			const [name, ...target] = key.slice('audit:'.length).split(':');
			const where = target.length ? target.slice(0, 2).join(':') : (failed.get(name)?.where ?? null);
			return { key, name, kind: 'audit', where, last: failed.has(name) ? 'failed' : (status.get(key) ?? null) };
		}),
		...nodes.map((key): RerunItem => ({ key, name: nodeLabel(key), kind: 'node', where: null, last: status.get(key) ?? null }))
	];
}
