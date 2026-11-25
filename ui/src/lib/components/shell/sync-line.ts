import type { WarehouseSync } from '$lib/api';

/** The top bar's words for a failing run-history sync: what stands, since when (`since`, already formatted), how it retries, and why. */
export function syncLine(failing: NonNullable<WarehouseSync['failing']>, since: string): string {
	if (failing.halted) return `Run history sync stopped since ${since} · ${failing.reason}`;
	const what = failing.unreachable ? `Run history can't reach the warehouse since ${since}` : `Run history not syncing since ${since}`;
	const retry = failing.retrySeconds ? ` · retrying every ${failing.retrySeconds} s` : '';
	return `${what}${retry} · ${failing.reason}`;
}
