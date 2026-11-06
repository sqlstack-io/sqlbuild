// Which selected runs each bulk action applies to. Pure.

import type { Capability, RunRow } from '$lib/api';
import { isActiveRun } from '$lib/status';
import type { BulkAction } from '../types';

export const bulkLabel: Record<BulkAction, string> = {
	cancel: 'Cancel',
	rerun: 'Rerun',
	from_failure: 'Rerun from failure'
};

export const bulkCapability: Record<BulkAction, Capability> = {
	cancel: 'run.cancel',
	rerun: 'run.rerun',
	from_failure: 'run.rerun'
};

/** Whether `action` applies to the run at all (permissions aside). */
export function eligible(action: BulkAction, row: RunRow): boolean {
	const active = isActiveRun(row.run.status);
	if (action === 'cancel') return active && !row.cancelRequestedAt;
	if (action === 'rerun') return !active;
	return !active && row.rerunFromFailureCount > 0;
}

/** Why an otherwise eligible run can't be acted on by this principal, or null. */
export function blockedBy(action: BulkAction, row: RunRow, can: (c: Capability) => boolean): Capability | null {
	if (!can(bulkCapability[action])) return bulkCapability[action];
	if (action !== 'cancel' && row.run.rewriteHistory && !can('run.rewrite_history')) return 'run.rewrite_history';
	return null;
}

export function eligibleIds(action: BulkAction, rows: readonly RunRow[], can: (c: Capability) => boolean): string[] {
	return rows.filter((r) => eligible(action, r) && !blockedBy(action, r, can)).map((r) => r.run.id);
}
