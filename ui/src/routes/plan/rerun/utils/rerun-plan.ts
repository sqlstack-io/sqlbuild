// What a rerun of one run runs, as selection text and as rows: each preset (the Rerun menu's rows) fills
// the selection box with exactly what it reruns, and the table lists the real nodes and checks. Pure.

import type { RunDetail } from '$lib/api';
import { nodeLabel } from '$lib/operate/labels';
import type { RerunRow } from '../../../runs/[id]/utils/rerun-rows';
import type { RunFailure } from '../../../runs/[id]/utils/run-failures';

export type RerunItem = { key: string; name: string; kind: 'check' | 'node'; where: string | null; last: string | null };

/** The selection text a preset fills the box with: names, `check:` for checks, or the run's own selection. */
export function presetText(row: RerunRow, detail: RunDetail, failures: readonly RunFailure[]): string {
	const checks = new Set(failures.filter((f) => f.kind === 'check').map((f) => f.name));
	const asToken = (name: string) => (checks.has(name) ? `check:${name}` : name);
	if (row.mode === 'all') return detail.selection.join(' ');
	if (row.mode === 'selected') return row.nodes.map(asToken).join(' ');
	return [...detail.rerunFromFailureNodes.map(nodeLabel), ...[...checks].map((c) => `check:${c}`)].join(' ');
}

/** The rows the box's text runs: the checks it names, then the nodes it selects, each with its last result in this run. */
export function rerunItems(text: string, nodeKeys: readonly string[], detail: RunDetail, failures: readonly RunFailure[]): RerunItem[] {
	const failed = new Map(failures.filter((f) => f.kind === 'check').map((f) => [f.name, f]));
	const whole = text.trim() === detail.selection.join(' ');
	const named = text.split(/\s+/).filter((t) => t.startsWith('check:')).map((t) => t.slice(6));
	// The whole run reruns its checks too; the failed ones are the ones this run knows about.
	const checkNames = [...new Set([...named, ...(whole ? failed.keys() : [])])];
	const status = new Map(detail.nodes.map((n) => [n.nodeKey, n.status]));
	return [
		...checkNames.map((name): RerunItem => ({ key: `check:${name}`, name, kind: 'check', where: failed.get(name)?.where ?? null, last: failed.has(name) ? 'failed' : null })),
		...nodeKeys.map((key): RerunItem => ({ key, name: nodeLabel(key), kind: 'node', where: null, last: status.get(key) ?? null }))
	];
}
