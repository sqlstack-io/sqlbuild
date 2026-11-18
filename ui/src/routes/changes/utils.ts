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
	return graphNode ? `/lineage?focus=${encodeURIComponent(change.key)}` : null;
}

/** A commit hash as its first 7 characters; anything that isn't a full-length hash, such as a tag, as it is. */
export function shortRevision(revision: string): string {
	return /^[0-9a-f]{8,}$/i.test(revision) ? revision.slice(0, 7) : revision;
}
