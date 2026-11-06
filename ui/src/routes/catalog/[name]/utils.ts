// Pure helpers for the catalog node page.

import type { CatalogNode } from '$lib/api';

export type Tab = 'overview' | 'columns' | 'sql' | 'drift' | 'lineage' | 'collineage' | 'runs' | 'checks' | 'warehouse' | 'history';

export type DriftKind = 'direct' | 'compiled' | 'upstream';

/** Drift against what was last built in this target, from the node's stale reasons. */
export function driftOf(node: Pick<CatalogNode, 'stale'>): DriftKind | null {
	for (const r of node.stale) if (r.reason === 'changed') return r.change === 'compiled' ? 'compiled' : 'direct';
	return node.stale.some((r) => r.reason === 'changed_upstream') ? 'upstream' : null;
}

export const driftLabel: Record<DriftKind, string> = {
	direct: 'Definition changed',
	compiled: 'Compiled SQL differs',
	upstream: 'Upstream definition changed'
};

/** The SQL viewer's note when only an upstream definition changed. */
export function upstreamNote(node: Pick<CatalogNode, 'stale'>): string | undefined {
	const r = node.stale.find((s) => s.reason === 'changed_upstream');
	return r && r.reason === 'changed_upstream' ? `${r.upstream.join(', ')} changed since this node’s last build.` : undefined;
}
