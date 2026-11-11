import dagre from '@dagrejs/dagre';
import type { Node, Edge } from '@xyflow/svelte';

// Fallback dimensions used before a node has been measured by Svelte Flow.
const DEFAULT_WIDTH = 236;
const DEFAULT_HEIGHT = 61;

export type LayoutOptions = {
	/** gap between layers (columns) in LR mode */
	rankSep?: number;
	/** gap between nodes within a layer */
	nodeSep?: number;
};

/** Recent layouts by graph shape, so returning to a graph skips dagre, the slow part of showing it. */
const recent = new Map<string, Map<string, { x: number; y: number }>>();
const RECENT_MAX = 8;

/**
 * Layered left-to-right layout via dagre. Uses each node's measured size when
 * available (so taller nodes — e.g. with extra Node fields enabled — get the
 * right spacing and the graph reflows cleanly on toggle).
 */
export function layoutGraph(nodes: Node[], edges: Edge[], opts: LayoutOptions = {}): Node[] {
	const size = (node: Node) => ({
		width: node.measured?.width ?? (node.width as number) ?? DEFAULT_WIDTH,
		height: node.measured?.height ?? (node.height as number) ?? DEFAULT_HEIGHT
	});
	const shape = JSON.stringify([
		opts,
		nodes.map((node) => [node.id, size(node).width, size(node).height]),
		edges.map((edge) => [edge.source, edge.target])
	]);
	let positions = recent.get(shape);
	if (positions) {
		recent.delete(shape);
	} else {
		positions = dagreLayout(nodes, edges, opts, size);
		if (recent.size >= RECENT_MAX) recent.delete(recent.keys().next().value!);
	}
	recent.set(shape, positions);
	return nodes.map((node) => ({ ...node, position: { ...positions.get(node.id)! } }));
}

function dagreLayout(nodes: Node[], edges: Edge[], opts: LayoutOptions, size: (node: Node) => { width: number; height: number }) {
	const g = new dagre.graphlib.Graph();
	g.setGraph({
		rankdir: 'LR',
		ranksep: opts.rankSep ?? 76,
		nodesep: opts.nodeSep ?? 18,
		marginx: 24,
		marginy: 24
	});
	g.setDefaultEdgeLabel(() => ({}));

	for (const node of nodes) g.setNode(node.id, size(node));
	for (const edge of edges) {
		g.setEdge(edge.source, edge.target);
	}

	dagre.layout(g);

	// dagre returns center coordinates; Svelte Flow expects top-left.
	return new Map(
		nodes.map((node) => {
			const pos = g.node(node.id);
			const { width, height } = size(node);
			return [node.id, { x: pos.x - width / 2, y: pos.y - height / 2 }];
		})
	);
}
