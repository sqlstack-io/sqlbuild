// The Rerun menu's rows for a run that didn't succeed: verb-first labels, what each runs, and how
// much. Rows that don't apply are left out; two rows that would run the same thing show once. Pure.

import { nodeLabel } from '$lib/operate/labels';
import type { RunFailure } from './run-failures';

export type RerunRow = {
	key: string;
	label: string;
	description: string;
	/** How much it runs, such as `1 audit` or `93 nodes`. */
	count: string;
	mode: 'from_failure' | 'selected' | 'all';
	/** The nodes or checks a `selected` row runs. */
	nodes: string[];
};

const count = (n: number, one: string, many: string) => `${n.toLocaleString('en-GB')} ${n === 1 ? one : many}`;

export function rerunRows({ failures, unfinished, total, jobKey }: { failures: readonly RunFailure[]; unfinished: readonly string[]; total: number; jobKey: string | null }): RerunRow[] {
	const checks = failures.filter((f) => f.kind === 'check');
	const failedNodes = failures.filter((f) => f.kind === 'node');
	const firstNode = failedNodes[0];
	const firstCheck = checks[0];
	// What "what didn't succeed" runs: the unfinished nodes and every failed check, as one set to compare.
	const runsAgain = [...unfinished, ...checks.map((c) => `check:${c.name}`)].sort();
	const candidates: (RerunRow & { runs: string[] })[] = [];
	if (runsAgain.length) {
		const parts = [unfinished.length ? count(unfinished.length, 'node', 'nodes') : null, checks.length ? count(checks.length, 'audit', 'audits') : null].filter(Boolean);
		candidates.push({
			key: 'unfinished',
			label: "Rerun what didn't succeed",
			description: unfinished.length ? 'Failed, cancelled and skipped nodes, and failed audits' : `The failed ${checks.length === 1 ? `audit ${firstCheck.name}` : 'audits'}`,
			count: parts.join(' · '),
			mode: 'from_failure',
			nodes: [],
			runs: runsAgain
		});
	}
	if (firstNode?.nodeKey) {
		const name = nodeLabel(firstNode.nodeKey);
		candidates.push({ key: 'node', label: 'Rerun the failed node and its audits', description: name, count: '1 node', mode: 'selected', nodes: [name], runs: [firstNode.nodeKey] });
	}
	if (firstCheck) {
		const where = firstCheck.where === 'after the build' ? 'after the build' : `on ${firstCheck.where}`;
		candidates.push({ key: 'check', label: 'Rerun the failed audit only', description: `${firstCheck.name}, ${where}`, count: '1 audit', mode: 'selected', nodes: [firstCheck.name], runs: [`check:${firstCheck.name}`] });
	}
	candidates.push({ key: 'all', label: 'Rerun the whole run', description: jobKey ? `Every node and audit of ${jobKey}` : 'Every node and audit of the selection', count: count(total, 'node', 'nodes'), mode: 'all', nodes: [], runs: ['*'] });
	const seen = new Set<string>();
	return candidates.flatMap(({ runs, ...row }) => {
		const same = runs.join(' ');
		if (seen.has(same)) return [];
		seen.add(same);
		return [row];
	});
}
