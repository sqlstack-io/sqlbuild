// What a graph shows, for its sidebar when nothing is selected. Pure; no side effects.

import type { CatalogNode, NodeKind } from '$lib/api';
import { NODE_KINDS } from '$lib/build-area/labels';

/** How many nodes need building: changed since their last build, and only behind (or downstream of) a change. */
export type NeedsBuilding = { changed: number; behind: number };

/** Counts of the nodes that need building; a node both changed and behind counts once, as changed. */
export function needsBuilding(keys: Iterable<string>, byKey: ReadonlyMap<string, CatalogNode>): NeedsBuilding {
	let changed = 0;
	let behind = 0;
	for (const key of new Set(keys)) {
		const status = byKey.get(key)?.status;
		if (status === 'changed') changed++;
		else if (status === 'behind') behind++;
	}
	return { changed, behind };
}

export type ViewSummary = NeedsBuilding & { total: number; byKind: { kind: NodeKind; count: number }[]; failed: number };

/** The nodes in view: how many of each kind, in the usual kind order, and how many need building or failed. */
export function viewSummary(nodes: readonly CatalogNode[]): ViewSummary {
	const counts = Map.groupBy(nodes, (n) => n.kind);
	const byKind = NODE_KINDS.filter((k) => counts.has(k)).map((kind) => ({ kind, count: counts.get(kind)!.length }));
	return {
		total: nodes.length,
		byKind,
		...needsBuilding(nodes.map((n) => n.key), new Map(nodes.map((n) => [n.key, n]))),
		failed: nodes.filter((n) => n.status === 'failed').length
	};
}
