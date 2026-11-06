import type { RunNode } from '$lib/api';
import type { NodeGroup } from '../types';

const GROUPS: Omit<NodeGroup, 'nodes'>[] = [
	{ key: 'running', label: 'Running', tone: 'var(--sb-running)', statuses: ['running'] },
	{ key: 'failed', label: 'Failed', tone: 'var(--sb-error)', statuses: ['failed'] },
	{ key: 'skipped', label: 'Skipped', tone: 'var(--sb-warning)', statuses: ['skipped'] },
	{ key: 'succeeded', label: 'Succeeded', tone: 'var(--sb-success)', statuses: ['success', 'warn'] },
	{ key: 'not_run', label: 'Not run', tone: 'var(--sb-text-faint)', statuses: ['pending', 'cancelled', 'not_run'] }
];

/** Nodes by outcome, in panel order; empty groups are dropped. */
export function groupNodes(nodes: readonly RunNode[]): NodeGroup[] {
	return GROUPS.map((g) => ({ ...g, nodes: nodes.filter((n) => g.statuses.includes(n.status)) })).filter((g) => g.nodes.length);
}

/** Why a node didn't run, in words. */
export function notRunReason(n: RunNode): string | null {
	if (n.status === 'pending') return 'not started yet';
	if (n.status === 'cancelled') return 'stopped by the cancel';
	if (n.status === 'not_run') return 'the run ended before it started';
	return null;
}

/** Nodes done (any outcome) out of all. */
export function progressOf(nodes: readonly RunNode[]): { done: number; total: number } {
	return { done: nodes.filter((n) => n.status !== 'pending' && n.status !== 'running').length, total: nodes.length };
}
