import type { RunNode } from '$lib/api';
import type { NodeGroup } from '../types';

const GROUPS: Omit<NodeGroup, 'nodes'>[] = [
	{ key: 'running', label: 'Running', tone: 'var(--sb-running)', statuses: ['running'] },
	{ key: 'waiting', label: 'Waiting', tone: 'var(--sb-text-faint)', statuses: ['pending'] },
	{ key: 'failed', label: 'Failed', tone: 'var(--sb-error)', statuses: ['failed'] },
	{ key: 'skipped', label: 'Skipped', tone: 'var(--sb-warning)', statuses: ['skipped'] },
	{ key: 'succeeded', label: 'Succeeded', tone: 'var(--sb-success)', statuses: ['success', 'warn'] },
	{ key: 'not_run', label: 'Not run', tone: 'var(--sb-text-faint)', statuses: ['cancelled', 'not_run'] }
];

/**
 * Nodes by outcome, in panel order; empty groups are dropped. While the run is `active` its pending
 * nodes are waiting their turn; once it has finished, a node still pending never ran.
 */
export function groupNodes(nodes: readonly RunNode[], active: boolean): NodeGroup[] {
	const groups = active ? GROUPS : GROUPS.filter((g) => g.key !== 'waiting').map((g) => (g.key === 'not_run' ? { ...g, statuses: ['pending' as const, ...g.statuses] } : g));
	return groups.map((g) => ({ ...g, nodes: nodes.filter((n) => g.statuses.includes(n.status)) })).filter((g) => g.nodes.length);
}

/** Why a node hasn't run, in words: while the run is `active` a pending node is waiting its turn. */
export function notRunReason(n: RunNode, active = false): string | null {
	if (n.status === 'pending') return active ? 'Waiting' : 'Not run';
	if (n.status === 'cancelled') return 'Cancelled';
	if (n.status === 'not_run') return 'Not run';
	return null;
}

/** Nodes done (any outcome) out of all. */
export function progressOf(nodes: readonly RunNode[]): { done: number; total: number } {
	return { done: nodes.filter((n) => n.status !== 'pending' && n.status !== 'running').length, total: nodes.length };
}
