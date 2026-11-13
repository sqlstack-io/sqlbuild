// Lineage cards from catalog nodes. Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode, StaleReason } from '$lib/api';
import { kindLine, nodeIconKey, shortName } from '$lib/build-area/labels';

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
		status: cardStatus(node),
		stale: cardStale(node.stale ?? []),
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

/** Above this many nodes the lineage opens with each folder collapsed into one card. */
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
	return slash > 0 ? path.slice(0, slash) : node.kind === 'seed' ? 'seeds' : 'models';
}

/** The outermost folder around a node, such as `marts`, which the catalog filters and Overview counts by. */
export const topFolder = (node: CatalogNode): string => folderOf(node).split('/')[0];

export type GroupCard = { group: string; count: number; ok: number; stale: number; failed: number; dimmed?: boolean };

const groupId = (group: string) => `group:${group}`;

/** A folder's card: how many nodes it holds, and how many last built fine, are stale or failed. */
function groupCard(group: string, members: CatalogNode[]): GroupCard {
	const built = members.filter((n) => n.kind !== 'source');
	return {
		group,
		count: members.length,
		ok: built.filter((n) => cardStatus(n) === 'ok' && !n.stale?.length).length,
		stale: built.filter((n) => !!n.stale?.length).length,
		failed: built.filter((n) => n.lastOutcome === 'failed').length
	};
}

/** `expanded` with every folder around the node open, so the node itself is drawn. */
export function withFoldersOpen(node: CatalogNode, expanded: ReadonlySet<string>): Set<string> {
	const parts = folderOf(node).split('/');
	return new Set([...expanded, ...parts.map((_, i) => parts.slice(0, i + 1).join('/'))]);
}

/** The outermost folder around a node that is not open, or null when every folder around it is. */
function closedFolder(node: CatalogNode, expanded: ReadonlySet<string>): string | null {
	const parts = folderOf(node).split('/');
	for (let depth = 1; depth <= parts.length; depth++) {
		const folder = parts.slice(0, depth).join('/');
		if (!expanded.has(folder)) return folder;
	}
	return null;
}

/**
 * The lineage with every folder that is not open drawn as one card. Folders nest, so opening
 * `marts` shows `marts/core` and the other folders inside it. An edge joins whatever stands for
 * each end, once per pair, and an edge inside a closed folder disappears.
 */
export function groupedGraph(nodes: CatalogNode[], expanded: ReadonlySet<string>): { nodes: Node[]; edges: Edge[] } {
	const standIn = new Map<string, string>();
	const folders = new Map<string, CatalogNode[]>();
	const cards: Node[] = [];
	for (const n of nodes) {
		const folder = closedFolder(n, expanded);
		standIn.set(n.key, folder === null ? n.key : groupId(folder));
		if (folder === null) cards.push({ id: n.key, type: 'asset', position: { x: 0, y: 0 }, data: cardData(n) });
		else if (folders.has(folder)) folders.get(folder)!.push(n);
		else {
			folders.set(folder, [n]);
			cards.push({ id: groupId(folder), type: 'folder', position: { x: 0, y: 0 }, selectable: false, data: {} });
		}
	}
	for (const card of cards) if (card.type === 'folder') card.data = groupCard(card.id.slice('group:'.length), folders.get(card.id.slice('group:'.length))!);
	const pairs = new Map<string, Edge>();
	for (const n of nodes) {
		for (const up of n.upstream) {
			const source = standIn.get(up);
			const target = standIn.get(n.key)!;
			if (!source || source === target) continue;
			const id = `${source}-${target}`;
			if (!pairs.has(id)) pairs.set(id, { id, source, target, type: 'bezier' });
		}
	}
	return { nodes: cards, edges: [...pairs.values()] };
}
