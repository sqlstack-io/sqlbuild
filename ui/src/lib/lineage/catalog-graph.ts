// Lineage cards from catalog nodes. Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode, StaleReason } from '$lib/api';
import { FUNCTION_KINDS, PYTHON_KINDS, kindLine, nodeIconKey, shortName } from '$lib/build-area/labels';

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
	if (!c?.total) return {};
	if (c.failing) return { checks: `✕ ${c.failing}`, checksColor: 'var(--sb-error)' };
	if (c.warning) return { checks: `! ${c.warning}`, checksColor: 'var(--sb-warning)' };
	return c.passing === c.total ? { checks: `✓ ${c.passing}` } : { checks: `${c.passing}/${c.total}` };
}

export function cardData(node: CatalogNode): Record<string, unknown> {
	return {
		name: node.key,
		label: shortName(node.key),
		kind: kindLine(node),
		group: node.group,
		nodeKind: node.kind,
		status: node.liveStatus ?? cardStatus(node),
		// A node the active run is building, or has just built, shows that, not whether it was stale.
		stale: node.liveStatus ? undefined : cardStale(node.stale ?? []),
		live: !!node.liveStatus,
		icon: nodeIconKey(node),
		lastBuiltAt: node.kind === 'source' ? (node.sourceFreshness?.loadedAt ?? undefined) : node.lastBuild?.builtAt,
		description: node.description || undefined,
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

/** Above this many nodes the lineage opens with each folder closed into one card. */
export const GROUPED_ABOVE = 150;

/**
 * The folder a node lives in, such as `marts/core`, or its source file, such as `sources/orders`;
 * a declared group wins.
 */
export function folderOf(node: CatalogNode): string {
	if (node.group) return node.group;
	const path = node.path.replace(/^models\//, '');
	if (node.kind === 'source') return path.replace(/\.[^./]+$/, '') || 'sources';
	const slash = path.lastIndexOf('/');
	if (slash > 0) return path.slice(0, slash);
	return node.kind === 'seed' ? 'seeds' : PYTHON_KINDS.includes(node.kind) ? 'python' : FUNCTION_KINDS.includes(node.kind) ? 'functions' : 'models';
}

/** The outermost folder around a node, such as `marts`, which the catalog filters and Overview counts by. */
export const topFolder = (node: CatalogNode): string => folderOf(node).split('/')[0];

/** A closed folder's card; `label` is its path below the open folder around it, when there is one. */
/** `noun` names what the folder counts: nodes, or jobs on a group's graph. */
export type GroupCard = { group: string; label?: string; count: number; ok: number; stale: number; failed: number; dimmed?: boolean; noun?: string; empty?: string };

/** A folder's card: how many nodes it holds, and how many last built fine, are stale or failed. */
export function groupCard(group: string, members: CatalogNode[]): GroupCard {
	const built = members.filter((n) => n.kind !== 'source');
	return {
		group,
		count: members.length,
		ok: built.filter((n) => cardStatus(n) === 'ok' && !n.stale?.length).length,
		stale: built.filter((n) => !!n.stale?.length).length,
		failed: built.filter((n) => n.lastOutcome === 'failed').length
	};
}
