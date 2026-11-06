import type { Edge, Node } from '@xyflow/svelte';

// Whole-graph layered layout for large projects. Stand-in for what `sqb compile`
// would emit: columns are dependency depth, and each column is ordered by a few
// barycentre sweeps so connected models sit near each other. Sizes and gaps
// match the lineage page's default cards and dagre spacing.

export const NODE_W = 236;
export const NODE_H = 61;
export const GAP_X = 76;
export const GAP_Y = 18;
export const COL_STEP = NODE_W + GAP_X;
export const ROW_STEP = NODE_H + GAP_Y;

export type GraphNode = {
	id: string;
	name: string;
	status: string;
	kind: string;
	depth: number;
	x: number;
	y: number;
};

export type GraphLayout = {
	nodes: GraphNode[];
	byId: Map<string, GraphNode>;
	edges: [string, string][];
	upstream: Map<string, string[]>;
	downstream: Map<string, string[]>;
	columns: GraphNode[][];
	colTop: number[]; // world y of each column's first row (columns are top-aligned)
	width: number;
	height: number;
};

export function layoutDag(nodes: Node[], edges: Edge[], sweeps = 6): GraphLayout {
	const byId = new Map<string, GraphNode>();
	for (const n of nodes) {
		const d = n.data as Record<string, unknown>;
		byId.set(n.id, {
			id: n.id,
			name: String(d.name),
			status: d.stale ? 'stale' : String(d.status),
			kind: String(d.kind),
			depth: 0,
			x: 0,
			y: 0
		});
	}
	const upstream = new Map<string, string[]>();
	const downstream = new Map<string, string[]>();
	for (const e of edges) {
		(upstream.get(e.target) ?? upstream.set(e.target, []).get(e.target)!).push(e.source);
		(downstream.get(e.source) ?? downstream.set(e.source, []).get(e.source)!).push(e.target);
	}

	// depth = longest path from a source (Kahn's topological order)
	const indegree = new Map<string, number>();
	for (const id of byId.keys()) indegree.set(id, upstream.get(id)?.length ?? 0);
	const queue = [...byId.keys()].filter((id) => indegree.get(id) === 0);
	for (let i = 0; i < queue.length; i++) {
		const id = queue[i];
		const depth = byId.get(id)!.depth;
		for (const next of downstream.get(id) ?? []) {
			const n = byId.get(next)!;
			n.depth = Math.max(n.depth, depth + 1);
			indegree.set(next, indegree.get(next)! - 1);
			if (indegree.get(next) === 0) queue.push(next);
		}
	}

	const maxDepth = Math.max(0, ...[...byId.values()].map((n) => n.depth));
	const columns: GraphNode[][] = Array.from({ length: maxDepth + 1 }, () => []);
	for (const n of [...byId.values()].sort((a, b) => a.name.localeCompare(b.name))) columns[n.depth].push(n);

	// barycentre sweeps: order each column by the average relative position of its neighbours
	const rel = new Map<string, number>();
	const index = () => {
		for (const col of columns) col.forEach((n, i) => rel.set(n.id, col.length > 1 ? i / (col.length - 1) : 0.5));
	};
	index();
	for (let s = 0; s < sweeps; s++) {
		const down = s % 2 === 0;
		const order = down ? columns.slice(1) : columns.slice(0, -1).reverse();
		for (const col of order) {
			const key = new Map<string, number>();
			for (const n of col) {
				const ns = (down ? upstream : downstream).get(n.id) ?? [];
				key.set(n.id, ns.length ? ns.reduce((sum, id) => sum + rel.get(id)!, 0) / ns.length : rel.get(n.id)!);
			}
			col.sort((a, b) => key.get(a.id)! - key.get(b.id)!);
			col.forEach((n, i) => rel.set(n.id, col.length > 1 ? i / (col.length - 1) : 0.5));
		}
	}

	const tallest = Math.max(...columns.map((c) => c.length));
	const colTop = columns.map(() => 0);
	columns.forEach((col, d) =>
		col.forEach((n, i) => {
			n.x = d * COL_STEP;
			n.y = colTop[d] + i * ROW_STEP;
		})
	);

	return {
		nodes: [...byId.values()],
		byId,
		edges: edges.map((e) => [e.source, e.target]),
		upstream,
		downstream,
		columns,
		colTop,
		width: columns.length * COL_STEP - GAP_X,
		height: tallest * ROW_STEP - GAP_Y
	};
}

/** Every model reachable from the given ids along `next` (inclusive). */
export function closure(ids: Iterable<string>, next: Map<string, string[]>): Set<string> {
	const seen = new Set<string>();
	const stack = [...ids];
	while (stack.length) {
		const id = stack.pop()!;
		if (seen.has(id)) continue;
		seen.add(id);
		for (const n of next.get(id) ?? []) stack.push(n);
	}
	return seen;
}
