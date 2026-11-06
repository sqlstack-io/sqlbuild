// Lineage cards from catalog nodes. Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode, StaleReason } from '$lib/api';
import { kindLine, nodeIconKey } from '$lib/build-area/labels';

export type CardStatus = 'ok' | 'fail' | 'warn' | 'src' | 'none';
export type CardStale = 'changed' | 'compiled' | 'upstream' | 'behind';

export function cardStatus(node: CatalogNode): CardStatus {
	if (node.kind === 'source') return 'src';
	if (node.lastOutcome === 'failed') return 'fail';
	if (node.lastOutcome === 'warn') return 'warn';
	if (node.lastOutcome === 'success' || node.lastOutcome === 'skipped') return node.lastBuild ? 'ok' : 'none';
	return node.lastBuild ? 'ok' : 'none';
}

/** The stale reason to show on a card: own changes first, then upstream changes, then behind. */
export function cardStale(reasons: StaleReason[]): CardStale | undefined {
	const changed = reasons.find((r) => r.reason === 'changed');
	if (changed && changed.reason === 'changed') return changed.change === 'compiled' ? 'compiled' : 'changed';
	if (reasons.some((r) => r.reason === 'changed_upstream')) return 'upstream';
	if (reasons.some((r) => r.reason === 'behind')) return 'behind';
	return undefined;
}

export function checksBadge(node: CatalogNode): { checks?: string; checksColor?: string } {
	const c = node.checks;
	if (!c.total) return {};
	if (c.failing) return { checks: `✕ ${c.failing}`, checksColor: 'var(--sb-error)' };
	if (c.warning) return { checks: `! ${c.warning}`, checksColor: 'var(--sb-warning)' };
	return c.passing === c.total ? { checks: `✓ ${c.passing}` } : { checks: `${c.passing}/${c.total}` };
}

export function cardData(node: CatalogNode): Record<string, unknown> {
	return {
		name: node.key,
		kind: kindLine(node),
		group: node.group,
		nodeKind: node.kind,
		status: cardStatus(node),
		stale: cardStale(node.stale),
		icon: nodeIconKey(node),
		lastBuiltAt: node.kind === 'source' ? (node.sourceFreshness?.loadedAt ?? undefined) : node.lastBuild?.builtAt,
		description: node.description || undefined,
		owner: node.owner,
		tags: node.tags,
		columnCount: node.columnCount || undefined,
		...checksBadge(node)
	};
}

export function catalogGraph(nodes: CatalogNode[]): { nodes: Node[]; edges: Edge[] } {
	return {
		nodes: nodes.map((n) => ({ id: n.key, type: 'asset', position: { x: 0, y: 0 }, data: cardData(n) })),
		edges: nodes.flatMap((n) => n.upstream.map((up) => ({ id: `${up}-${n.key}`, source: up, target: n.key, type: 'bezier' })))
	};
}
