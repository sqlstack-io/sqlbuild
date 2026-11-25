// A node's build status as every page shows it: the server works it out once (`NodeState.status`),
// and pages only name and colour it here, never deriving it again. Pure.

import type { CatalogNode, GroupStatus, NodeBuildStatus } from '$lib/api';

/** Each status's name and colour. */
export const NODE_STATUS: Record<NodeBuildStatus, { label: string; tone: string }> = {
	current: { label: 'Current', tone: 'var(--sb-success)' },
	changed: { label: 'Changed', tone: 'var(--sb-stale)' },
	behind: { label: 'Behind', tone: 'var(--sb-behind)' },
	failed: { label: 'Failed', tone: 'var(--sb-error)' },
	not_built: { label: 'Not built', tone: 'var(--muted-foreground)' },
	source: { label: 'Source', tone: 'var(--sb-text-faint)' }
};

/** A node's status, or null when the server has not reported it (an older server, or before its state is read). */
export const statusOf = (node: Pick<CatalogNode, 'status'>): NodeBuildStatus | null => node.status ?? null;

/** A node's status label and colour; null when it is not known. */
export function statusLabel(node: Pick<CatalogNode, 'status'>): { label: string; tone: string } | null {
	const status = statusOf(node);
	return status ? NODE_STATUS[status] : null;
}

/** Adds a node to a folder's counts: one of current, changed, behind or failed, or none when not built. */
export function countStatus(counts: Omit<GroupStatus, 'group'>, node: Pick<CatalogNode, 'status'>): void {
	const status = statusOf(node);
	counts.total += 1;
	if (status === 'current' || status === 'changed' || status === 'behind' || status === 'failed') counts[status] += 1;
}
