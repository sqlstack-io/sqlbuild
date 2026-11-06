import type { Edge } from '@xyflow/svelte';

/** Immediate neighbours (edges touching the node) — used for hover. */
export function neighbourEdgeIds(nodeId: string, edges: Edge[]): Set<string> {
	const ids = new Set<string>();
	for (const e of edges) {
		if (e.source === nodeId || e.target === nodeId) ids.add(e.id);
	}
	return ids;
}

/** Full upstream + downstream path from a set of nodes — used for selection. */
export function tracePath(
	nodeIds: Set<string>,
	edges: Edge[]
): { nodes: Set<string>; edges: Set<string> } {
	const outgoing = new Map<string, Edge[]>();
	const incoming = new Map<string, Edge[]>();
	for (const e of edges) {
		(outgoing.get(e.source) ?? outgoing.set(e.source, []).get(e.source)!).push(e);
		(incoming.get(e.target) ?? incoming.set(e.target, []).get(e.target)!).push(e);
	}

	const litNodes = new Set<string>(nodeIds);
	const litEdges = new Set<string>();

	// downstream (follow outgoing)
	const downStack = [...nodeIds];
	while (downStack.length) {
		const id = downStack.pop()!;
		for (const e of outgoing.get(id) ?? []) {
			litEdges.add(e.id);
			if (!litNodes.has(e.target)) {
				litNodes.add(e.target);
				downStack.push(e.target);
			}
		}
	}
	// upstream (follow incoming)
	const upStack = [...nodeIds];
	while (upStack.length) {
		const id = upStack.pop()!;
		for (const e of incoming.get(id) ?? []) {
			litEdges.add(e.id);
			if (!litNodes.has(e.source)) {
				litNodes.add(e.source);
				upStack.push(e.source);
			}
		}
	}

	return { nodes: litNodes, edges: litEdges };
}
