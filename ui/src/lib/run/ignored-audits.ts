// The audit failures a run let through (`ignore_audit_failures`), for its run page. Pure.

import type { Check } from '$lib/api';

/**
 * The checks whose newest result is a failure one of `runIds` let through. The server lists only each
 * check's newest result, so a failure a later run has since replaced isn't among them.
 */
export function ignoredFailuresOf(checks: Check[], runIds: ReadonlySet<string>): Check[] {
	return checks.filter((c) => !!c.latest?.ignored && (runIds.has(c.latest.runId ?? '') || runIds.has(c.latest.fingerprintRunId)));
}
