// What a run did, for the line under its header: what it built, what failed or was skipped, and its checks. Pure.

import type { RunEvent, RunNode } from '$lib/api';

export type RunSummary = {
	built: number;
	incremental: number;
	incrementalRows: number;
	failed: number;
	warned: number;
	skipped: number;
	notRun: number;
	running: number;
	waiting: number;
	checksPassed: number;
	checksWarned: number;
	checksFailed: number;
};

/** What the run did, for the line under the header: built nodes and the rows incrementals added, failures, skips and checks. */
export function summaryOf(nodes: readonly RunNode[], events: readonly RunEvent[], active: boolean): RunSummary {
	const built = nodes.filter((n) => n.status === 'success' || n.status === 'warn');
	const incremental = built.filter((n) => n.kind === 'incremental');
	const checks = events.filter((e) => e.type === 'audit_completed');
	return {
		built: built.length,
		incremental: incremental.length,
		incrementalRows: incremental.reduce((sum, n) => sum + (n.rowsAffected ?? 0), 0),
		failed: nodes.filter((n) => n.status === 'failed').length,
		warned: nodes.filter((n) => n.status === 'warn').length,
		skipped: nodes.filter((n) => n.status === 'skipped').length,
		notRun: nodes.filter((n) => n.status === 'not_run' || n.status === 'cancelled' || (!active && n.status === 'pending')).length,
		running: nodes.filter((n) => n.status === 'running').length,
		waiting: active ? nodes.filter((n) => n.status === 'pending').length : 0,
		checksPassed: checks.filter((e) => e.level === 'info').length,
		checksWarned: checks.filter((e) => e.level === 'warn').length,
		checksFailed: checks.filter((e) => e.level === 'error').length
	};
}
