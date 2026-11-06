// Pure helpers for the Changes page.

import type { OrchestrationChange } from '$lib/api';

export function orchestrationHref(change: OrchestrationChange): string | null {
	if (change.op === 'removed') return null;
	if (change.kind === 'job' || change.kind === 'group') return `/jobs/${change.key}`;
	return change.triggerId ? `/triggers/${change.triggerId}` : null;
}

export function percentChange(before: number, after: number): number {
	return before ? Math.round(((after - before) / before) * 100) : 0;
}
