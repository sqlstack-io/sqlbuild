// A rerun is a selection like any other: `failed:<run>+` reruns what didn't succeed, `audit:<name>` one
// audit, `run:<run>` the whole run. Presets only write that text; what runs, and which run it came from,
// is read back from the text alone. Pure.

import type { RunDetail } from '$lib/api';
import { nodeLabel } from '$lib/operate/labels';
import type { RerunRow } from '../../../runs/[id]/utils/rerun-rows';
import type { RunFailure } from '../../../runs/[id]/utils/run-failures';

export type RerunItem = { key: string; name: string; kind: 'audit' | 'node'; where: string | null; last: string | null };

const RUN_TYPES = ['run', 'failed', 'skipped'];
const typed = (text: string) =>
	text
		.split(/\s+/)
		.filter(Boolean)
		.map((t) => t.replace(/^\+/, '').replace(/\+$/, ''))
		.flatMap((t) => (t.indexOf(':') > 0 ? [{ type: t.slice(0, t.indexOf(':')), value: t.slice(t.indexOf(':') + 1) }] : []));

/** The selection text a preset writes into the box. */
export function presetText(row: RerunRow, runId: string): string {
	if (row.mode === 'all') return `run:${runId}`;
	if (row.mode === 'from_failure') return `failed:${runId}+`;
	return row.nodes.map((n) => (row.key === 'check' ? `audit:${n}` : n)).join(' ');
}

/** The runs a selection reruns from (`run:`, `failed:`, `skipped:`), as written: the provenance a plan shows. */
export function runsNamed(text: string): string[] {
	return [...new Set(typed(text).filter((t) => RUN_TYPES.includes(t.type)).map((t) => t.value))];
}

/** A written run ID (full, or a prefix of 8 or more) is this run's. */
export const namesRun = (written: string, runId: string) => written === runId || (written.length >= 8 && runId.startsWith(written));

/** The rows the box's text runs: the audits it names, then the nodes it selects, each with its last result in that run. */
export function rerunItems(text: string, nodeKeys: readonly string[], detail: RunDetail, failures: readonly RunFailure[]): RerunItem[] {
	const failed = new Map(failures.filter((f) => f.kind === 'check').map((f) => [f.name, f]));
	const audits = new Set<string>();
	for (const t of typed(text)) {
		if (t.type === 'audit') audits.add(t.value);
		// `failed:` and `run:` bring the run's failed audits with them.
		if ((t.type === 'failed' || t.type === 'run') && namesRun(t.value, detail.run.id)) for (const name of failed.keys()) audits.add(name);
	}
	const status = new Map(detail.nodes.map((n) => [n.nodeKey, n.status]));
	return [
		...[...audits].map((name): RerunItem => ({ key: `audit:${name}`, name, kind: 'audit', where: failed.get(name)?.where ?? null, last: failed.has(name) ? 'failed' : null })),
		...nodeKeys.map((key): RerunItem => ({ key, name: nodeLabel(key), kind: 'node', where: null, last: status.get(key) ?? null }))
	];
}
