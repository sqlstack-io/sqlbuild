// Pure helpers for the Changes page.

import type { NodeChange, OrchestrationChange } from '$lib/api';

export function orchestrationHref(change: OrchestrationChange): string | null {
	if (change.op === 'removed') return null;
	if (change.kind === 'job' || change.kind === 'group') return `/jobs/${change.key}`;
	return change.triggerId ? `/triggers/${change.triggerId}` : null;
}

export function percentChange(before: number, after: number): number {
	return before ? Math.round(((after - before) / before) * 100) : 0;
}

/** A changed node's page: its catalog detail, or the lineage graph when the server has no detail read. */
export function nodeHref(change: NodeChange, catalogDetail: boolean): string | null {
	if (change.op === 'removed') return null;
	if (catalogDetail) return `/catalog/${change.key}${change.op === 'changed' ? '?tab=drift' : ''}`;
	const graphNode = change.kind !== null || change.key.startsWith('model:') || !change.key.includes(':');
	return graphNode ? `/lineage?select=${encodeURIComponent(change.key)}` : null;
}
