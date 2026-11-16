// Edges at the level you can see, shared by Lineage's folders and a group's member jobs: node-level
// links are drawn between whatever stands for each end, once per visible pair, thicker the more links
// a pair stands for. Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';

/** One node-level link: `from` feeds `to`. */
export type NodeLink = { from: string; to: string };

/** What a drawn edge stands for: how many node links, and the words its tooltip shows. */
export type VisibleEdgeData = { count: number; title: string };

export type EdgeState = 'neutral' | 'on' | 'dim';

/** An edge's width: thicker the more node links it stands for, capped so a busy pair stays readable. */
export const edgeWidth = (count: number) => (count <= 1 ? 1.5 : Math.min(4.5, 1.5 + Math.log2(count) * 0.75));

/** An edge's style for its count, highlighted with the hovered or selected card, or dimmed with another. */
export function edgeStyle(count: number, state: EdgeState = 'neutral', dashed = false): string {
	const width = edgeWidth(count) + (state === 'on' ? 0.5 : 0);
	const colour = state === 'on' ? 'var(--primary)' : state === 'dim' ? 'var(--sb-edge-dim)' : 'var(--sb-edge)';
	return `stroke: ${colour}; stroke-width: ${width};${dashed ? ' stroke-dasharray: 5 4;' : ''}`;
}

/** "3 lineage links from orders to marts". */
export const linksTitle = (count: number, from: string, to: string) => `${count} lineage link${count === 1 ? '' : 's'} from ${from} to ${to}`;

/**
 * One edge per pair of visible ends: each link joins `standIn(from)` and `standIn(to)`, and links
 * whose ends stand in the same place (inside one closed box) are not drawn. `label` names an end in
 * the tooltip.
 */
export function visibleEdges(
	links: Iterable<NodeLink>,
	standIn: (key: string) => string,
	{ label = (id: string) => id, title = (count: number, source: string, target: string) => linksTitle(count, label(source), label(target)) }: { label?: (id: string) => string; title?: (count: number, source: string, target: string) => string } = {}
): (Edge & { data: VisibleEdgeData })[] {
	const pairs = new Map<string, Edge & { data: VisibleEdgeData }>();
	for (const link of links) {
		const [source, target] = [standIn(link.from), standIn(link.to)];
		if (source === target) continue;
		const id = `${source}-${target}`;
		const edge = pairs.get(id);
		if (edge) edge.data.count++;
		else pairs.set(id, { id, source, target, type: 'count', data: { count: 1, title: '' } });
	}
	return [...pairs.values()].map((e) => ({ ...e, style: edgeStyle(e.data.count), data: { ...e.data, title: title(e.data.count, e.source, e.target) } }));
}

/** The edges that touch a card, or a box or anything inside it, for highlighting what it connects to. */
export function edgesTouching(id: string, edges: readonly Edge[], nodes: readonly Pick<Node, 'id' | 'parentId'>[]): Set<string> {
	const parentOf = new Map(nodes.map((n) => [n.id, n.parentId]));
	const within = (at: string | undefined) => {
		for (let p = at; p; p = parentOf.get(p)) if (p === id) return true;
		return false;
	};
	return new Set(edges.filter((e) => within(e.source) || within(e.target)).map((e) => e.id));
}
