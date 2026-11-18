// Group member attempts: a member the group retried has later attempts as sibling job runs.

import type { Run } from '$lib/api';

/** How many member jobs of a group run the group retried. */
export function retriedMembers(children: readonly Run[]): number {
	return new Set(children.flatMap((child) => (child.memberAttempt && child.jobKey ? [child.jobKey] : []))).size;
}

/** A member run's attempt, for its row: `attempt 2`, `attempt 1 · retried` once a later attempt replaced it, or null for a member the group never retried. */
export function attemptLabel(run: Run, siblings: readonly Run[]): string | null {
	const replaced = siblings.some((sibling) => sibling.rerunOfRunId === run.id && sibling.parentRunId === run.parentRunId);
	if (!run.memberAttempt && !replaced) return null;
	const attempt = `attempt ${run.memberAttempt ?? 1}`;
	return replaced ? `${attempt} · retried` : attempt;
}
