import type { CheckOutcome, NodeCounts, NodeOutcome, StaleReason, RunKind, RunOrigin, RunStatus, TickOutcome, TriggerState } from '$lib/api/types';

export const runStatusTone: Record<RunStatus, string> = {
	queued: 'var(--sb-text-faint)',
	launched: 'var(--sb-running)',
	running: 'var(--sb-running)',
	unresponsive: 'var(--sb-warning)',
	success: 'var(--sb-success)',
	warn: 'var(--sb-warning)',
	partial: 'color-mix(in srgb, var(--sb-error) 55%, var(--sb-warning))',
	failed: 'var(--sb-error)',
	cancelled: 'var(--sb-text-faint)',
	presumed_failed: 'var(--sb-error)'
};

export const runStatusLabel: Record<RunStatus, string> = {
	queued: 'Queued',
	launched: 'Starting',
	running: 'Running',
	unresponsive: 'No signal',
	success: 'Succeeded',
	warn: 'Warnings',
	partial: 'Partial',
	failed: 'Failed',
	cancelled: 'Cancelled',
	presumed_failed: 'Lost contact'
};

/** Tooltip for each status: what it means, with the server's status name for debugging. */
export const runStatusHelp: Record<RunStatus, string> = {
	queued: 'Waiting for its turn (queued)',
	launched: 'The container was started and hasn’t reported in yet (launched)',
	running: 'Running (running)',
	unresponsive: 'The run stopped sending heartbeats; it is marked failed if the silence continues (unresponsive)',
	success: 'Every node succeeded (success)',
	warn: 'Finished; some checks warned (warn)',
	partial: 'Some nodes failed or were skipped; the rest finished (partial)',
	failed: 'Failed (failed)',
	cancelled: 'Cancelled (cancelled)',
	presumed_failed: 'The run went silent and was marked failed; rerunning is safe (presumed_failed)'
};

const ACTIVE: ReadonlySet<RunStatus> = new Set(['queued', 'launched', 'running', 'unresponsive']);
const PROBLEM: ReadonlySet<RunStatus> = new Set(['failed', 'partial', 'presumed_failed']);

/** Not yet terminal: waiting, starting or executing. */
export function isActiveRun(status: RunStatus): boolean {
	return ACTIVE.has(status);
}

/** Terminal with at least one failure. */
export function isProblemRun(status: RunStatus): boolean {
	return PROBLEM.has(status);
}

/** Sort weight: problems and live runs first. */
export function runStatusRank(status: RunStatus): number {
	if (PROBLEM.has(status)) return 3;
	if (ACTIVE.has(status)) return 2;
	if (status === 'warn') return 1;
	return 0;
}

export const tickOutcomeLabel: Record<TickOutcome, string> = {
	requested: 'Started a run',
	skipped: 'Skipped',
	covered: 'Merged into a waiting run',
	failed: 'Couldn’t start a run'
};

export const triggerStateLabel: Record<TriggerState, string> = {
	running: 'On',
	paused: 'Paused'
};

export const checkOutcomeTone: Record<CheckOutcome, string> = {
	pass: 'var(--sb-success)',
	warn: 'var(--sb-warning)',
	fail: 'var(--sb-error)',
	error: 'var(--sb-error)',
	insufficient: 'var(--sb-text-faint)',
	skipped: 'var(--sb-text-faint)'
};

export const checkOutcomeLabel: Record<CheckOutcome, string> = {
	pass: 'Passed',
	warn: 'Warning',
	fail: 'Failed',
	error: 'Error',
	insufficient: 'Insufficient data',
	skipped: 'Skipped'
};

export const nodeOutcomeTone: Record<NodeOutcome, string> = {
	success: 'var(--sb-success)',
	warn: 'var(--sb-warning)',
	failed: 'var(--sb-error)',
	skipped: 'var(--sb-text-faint)'
};

export const nodeOutcomeLabel: Record<NodeOutcome, string> = {
	success: 'Success',
	warn: 'Warning',
	failed: 'Failed',
	skipped: 'Skipped'
};

/** Short label for a stale reason, e.g. `changed`, `changed (upstream)`, `behind`. */
export function staleReasonLabel(reason: StaleReason): string {
	if (reason.reason === 'changed') return reason.change === 'compiled' ? 'changed (compiled)' : 'changed';
	if (reason.reason === 'changed_upstream') return 'changed (upstream)';
	return 'behind';
}

/** One-line explanation of a stale reason. */
export function staleReasonDetail(reason: StaleReason): string {
	if (reason.reason === 'changed') return reason.detail;
	if (reason.reason === 'changed_upstream') return `Upstream definition changed: ${reason.upstream.join(', ')}`;
	return `Newer data in ${reason.upstream.join(', ')}`;
}

/** e.g. `8 ok · 1 warn · 1 failed · 3 skipped`, or `4 running · 10 pending`. */
export function formatNodeCounts(counts: NodeCounts): string {
	const parts: string[] = [];
	if (counts.running) parts.push(`${counts.running} running`);
	if (counts.success) parts.push(`${counts.success} ok`);
	if (counts.warn) parts.push(`${counts.warn} warn`);
	if (counts.failed) parts.push(`${counts.failed} failed`);
	if (counts.skipped) parts.push(`${counts.skipped} skipped`);
	if (counts.pending) parts.push(`${counts.pending} pending`);
	return parts.join(' · ') || '—';
}

export const runKindLabel: Record<RunKind, string> = {
	job: 'Job',
	group: 'Group',
	adhoc: 'Ad hoc'
};

export const runOriginLabel: Record<RunOrigin, string> = {
	manual: 'Manual',
	schedule: 'Schedule',
	sensor: 'Sensor',
	rerun: 'Rerun'
};

/** One-line "what · why" label, e.g. "Group · Schedule". */
export function runKindOriginLabel(run: { kind: RunKind; origin: RunOrigin }): string {
	return `${runKindLabel[run.kind]} · ${runOriginLabel[run.origin]}`;
}
