// A job's nodes as a lineage graph; for a group, each member job is a box around the nodes it builds.

import dagre from '@dagrejs/dagre';
import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode, JobGroupMember, RunRef } from '$lib/api';
import { shortName } from '$lib/build-area/labels';
import { cardData } from './catalog-graph';

const NODE_WIDTH = 236;
const NODE_HEIGHT = 61;
/** Room inside a member box: padding around its nodes plus its header. */
const PAD = 14;
const HEADER = 30;
const EMPTY_HEIGHT = 44;
/** A collapsed box shows only its header. */
const COLLAPSED_HEIGHT = HEADER + 2;
/** Wide enough for a header's name, node count and last run on one line. */
const COLLAPSED_WIDTH = 380;
/** Space between cards stacked in one column. */
const STACK_GAP = 20;
/** Space dagre leaves between cards, and between boxes and cards outside them. */
const CARD_GAP = 24;
const BOX_GAP = 24 + HEADER;
const MARGIN = 24;

export type JobBox = { jobKey: string; nodes: string[]; lastRun?: RunRef | null };

const edgeStyle = 'stroke: var(--sb-edge); stroke-width: 1.5;';
const afterStyle = 'stroke: var(--sb-edge); stroke-width: 1.5; stroke-dasharray: 5 4;';

export function boxId(jobKey: string): string {
	return `job:${jobKey}`;
}

/**
 * Cards for `nodes` and the lineage between them. With `boxes`, a node built by several members sits
 * in the most specific box, the one building the fewest nodes (the earliest on a tie), so a small
 * job inside a broad one keeps its nodes; a declared `after` between two members with no lineage
 * between them is a dashed edge between their boxes. A box in `collapsed` shows only its header,
 * and lineage to its cards meets the box instead.
 */
export function jobGraph(
	nodes: string[],
	catalog: CatalogNode[],
	boxes: JobBox[] = [],
	members: JobGroupMember[] = [],
	collapsed: ReadonlySet<string> = new Set()
): { nodes: Node[]; edges: Edge[] } {
	const byKey = new Map(catalog.map((n) => [n.key, n]));
	const inJob = new Set(nodes);
	const built = new Map(boxes.map((b) => [b.jobKey, b.nodes.filter((n) => inJob.has(n))]));
	const owner = new Map<string, string>();
	for (const box of [...boxes].sort((a, b) => built.get(a.jobKey)!.length - built.get(b.jobKey)!.length))
		for (const n of built.get(box.jobKey)!) if (!owner.has(n)) owner.set(n, box.jobKey);
	// What a card is drawn as: itself, or its collapsed box.
	const shownAs = (key: string) => {
		const parent = owner.get(key);
		return parent && collapsed.has(parent) ? boxId(parent) : key;
	};

	const boxNodes: Node[] = boxes.map((b) => ({
		id: boxId(b.jobKey),
		type: 'job',
		position: { x: 0, y: 0 },
		width: collapsed.has(b.jobKey) ? COLLAPSED_WIDTH : NODE_WIDTH + 2 * PAD,
		height: collapsed.has(b.jobKey) ? COLLAPSED_HEIGHT : EMPTY_HEIGHT,
		selectable: false,
		data: {
			jobKey: b.jobKey,
			lastRun: b.lastRun ?? null,
			nodeCount: built.get(b.jobKey)!.length,
			drawnElsewhere: built.get(b.jobKey)!.filter((n) => owner.get(n) !== b.jobKey).length,
			collapsed: collapsed.has(b.jobKey)
		}
	}));
	const cards: Node[] = nodes.filter((key) => shownAs(key) === key).map((key) => {
		const parent = owner.get(key);
		const node = byKey.get(key);
		return {
			id: key,
			type: 'asset',
			position: { x: 0, y: 0 },
			...(parent ? { parentId: boxId(parent), extent: 'parent' as const } : {}),
			data: node ? cardData(node) : { name: key, label: shortName(key), kind: '', status: 'none' }
		};
	});

	const seen = new Set<string>();
	const lineage: Edge[] = nodes.flatMap((key) =>
		(byKey.get(key)?.upstream ?? [])
			.filter((up) => inJob.has(up))
			.flatMap((up) => {
				const [source, target] = [shownAs(up), shownAs(key)];
				const id = `${source}-${target}`;
				if (source === target || seen.has(id)) return [];
				seen.add(id);
				return [{ id, source, target, type: 'bezier', style: edgeStyle }];
			})
	);
	const jobOf = (id: string) => owner.get(id) ?? id.slice('job:'.length);
	const linked = new Set(lineage.map((e) => `${jobOf(e.source)}>${jobOf(e.target)}`));
	const after: Edge[] = members.flatMap((m) =>
		m.upstream
			.filter((u) => !linked.has(`${u.jobKey}>${m.jobKey}`) && boxes.some((b) => b.jobKey === u.jobKey))
			.map((u) => ({ id: `after:${u.jobKey}-${m.jobKey}`, source: boxId(u.jobKey), target: boxId(m.jobKey), type: 'bezier', style: afterStyle }))
	);
	// Svelte Flow needs a parent before its children.
	return { nodes: [...boxNodes, ...cards], edges: [...lineage, ...after] };
}

/** The most cards a column holds before it wraps, growing with the graph so its shape stays near the canvas's. */
export function maxColumnCards(count: number): number {
	return Math.max(8, Math.round(Math.sqrt(count * 3)));
}

type Item = { id: string; width: number; height: number };
type Rect = { x: number; y: number; width: number; height: number };

/**
 * Dagre stacks every node of a rank in one column, so a job whose nodes hang off a few parents, or
 * have no lineage inside the job, draws as one column too tall to read. This gives each node a rank
 * that spreads such a column over several: the edges into a node get the rank gap as their minimum
 * length, and a moved node with no edge in hangs off the column's first node by `holds`, edges that
 * are laid out but not drawn.
 */
function wrapColumns(ids: string[], pairs: ReadonlyArray<readonly [string, string]>) {
	const preds = new Map<string, string[]>();
	for (const [s, t] of pairs) preds.set(t, [...(preds.get(t) ?? []), s]);
	// Each node's rank before wrapping: the longest path to it.
	const depth = new Map<string, number>();
	const depthOf = (id: string): number => {
		if (depth.has(id)) return depth.get(id)!;
		depth.set(id, 0); // A cycle ends here rather than looping; dagre breaks it for the drawing.
		const d = Math.max(-1, ...(preds.get(id) ?? []).map(depthOf)) + 1;
		depth.set(id, d);
		return d;
	};
	const columns = new Map<number, string[]>();
	for (const id of ids) columns.set(depthOf(id), [...(columns.get(depthOf(id)) ?? []), id]);

	const max = maxColumnCards(ids.length);
	const rank = new Map<string, number>();
	const holds: [string, string][] = [];
	let wrapped = false;
	const first = (id: string) => [...(preds.get(id) ?? [])].sort()[0] ?? '';
	for (const [, column] of [...columns].sort(([a], [b]) => a - b)) {
		// Siblings stay together, so a parent's children fill a column before the next parent's.
		column.sort((a, b) => first(a).localeCompare(first(b)) || a.localeCompare(b));
		const base = Math.max(0, ...column.flatMap((id) => (preds.get(id) ?? []).map((p) => (rank.get(p) ?? 0) + 1)));
		const per = Math.ceil(column.length / Math.ceil(column.length / max));
		wrapped ||= per < column.length;
		column.forEach((id, i) => {
			const step = Math.floor(i / per);
			rank.set(id, base + step);
			if (step && !preds.has(id)) holds.push([column[0], id]);
		});
	}
	return { rank, holds, wrapped };
}

/**
 * Lays `items` out left to right from the origin, wrapping tall columns. Dagre staggers the columns
 * a tall one wrapped into, lining each up with what it reads, so a graph that wrapped then stacks
 * every column from the top: a column keeps dagre's order, and so its edge crossings. A graph that
 * didn't wrap keeps dagre's layout.
 */
function layoutLayer(items: Item[], pairs: ReadonlyArray<readonly [string, string]>, nodesep: number): Map<string, Rect> {
	const g = new dagre.graphlib.Graph();
	g.setGraph({ rankdir: 'LR', ranksep: 64, nodesep, marginx: 0, marginy: 0 });
	g.setDefaultEdgeLabel(() => ({}));
	for (const item of items) g.setNode(item.id, { width: item.width, height: item.height });
	const { rank, holds, wrapped } = wrapColumns(items.map((i) => i.id), pairs);
	for (const [s, t] of pairs) g.setEdge(s, t, { minlen: wrapped ? Math.max(1, rank.get(t)! - rank.get(s)!) : 1 });
	for (const [s, t] of holds) if (!g.hasEdge(s, t)) g.setEdge(s, t, { minlen: rank.get(t)! - rank.get(s)! });
	dagre.layout(g);

	const rects = new Map(items.map((i) => {
		const p = g.node(i.id);
		return [i.id, { x: p.x - i.width / 2, y: p.y - i.height / 2, width: i.width, height: i.height }];
	}));
	if (wrapped) {
		const top = Math.min(...[...rects.values()].map((r) => r.y));
		const stacks = Map.groupBy(rects.values(), (r) => Math.round(r.x + r.width / 2));
		for (const stack of stacks.values()) {
			let y = top;
			for (const r of stack.sort((a, b) => a.y - b.y)) {
				r.y = y;
				y += r.height + STACK_GAP;
			}
		}
	}
	const left = Math.min(...[...rects.values()].map((r) => r.x));
	const top = Math.min(...[...rects.values()].map((r) => r.y));
	for (const r of rects.values()) {
		r.x -= left;
		r.y -= top;
	}
	return rects;
}

/**
 * Lays the graph out left to right: first each box's cards, which sizes the box, then the boxes and
 * the cards outside any box, ordered by the lineage and `after` edges between them. Cards use their
 * measured size, and a card's position is relative to its box, as Svelte Flow expects.
 */
export function layoutJobGraph(nodes: Node[], edges: Edge[]): Node[] {
	if (!nodes.length) return [];
	const size = (n: Node) => ({ width: n.measured?.width ?? NODE_WIDTH, height: n.measured?.height ?? NODE_HEIGHT });
	const children = Map.groupBy(nodes.filter((n) => n.parentId), (n) => n.parentId!);
	const pairsAmong = (ids: Set<string>, lift: (id: string) => string) => {
		const seen = new Set<string>();
		return edges.flatMap((e) => {
			const [s, t] = [lift(e.source), lift(e.target)];
			if (s === t || !ids.has(s) || !ids.has(t) || seen.has(`${s}>${t}`)) return [];
			seen.add(`${s}>${t}`);
			return [[s, t] as const];
		});
	};
	const parentOf = new Map(nodes.map((n) => [n.id, n.parentId ?? n.id]));
	const topPairs = pairsAmong(new Set(nodes.filter((n) => !n.parentId).map((n) => n.id)), (id) => parentOf.get(id) ?? id);
	// Lineage that leaves a box and comes back, as when a narrow job builds a node in the middle of a
	// broad job's chain, can't be drawn box by box without an edge running backwards.
	if (loops(topPairs)) return layoutCompound(nodes, edges, size, children);

	const inside = new Map<string, Map<string, Rect>>();
	const outer: Item[] = [];
	for (const n of nodes) {
		if (n.parentId) continue;
		const kids = children.get(n.id);
		if (!kids) {
			outer.push(n.type === 'job' ? { id: n.id, width: n.width ?? NODE_WIDTH + 2 * PAD, height: n.height ?? EMPTY_HEIGHT } : { id: n.id, ...size(n) });
			continue;
		}
		const rects = layoutLayer(kids.map((k) => ({ id: k.id, ...size(k) })), pairsAmong(new Set(kids.map((k) => k.id)), (id) => id), CARD_GAP);
		inside.set(n.id, rects);
		const right = Math.max(...[...rects.values()].map((r) => r.x + r.width));
		const bottom = Math.max(...[...rects.values()].map((r) => r.y + r.height));
		outer.push({ id: n.id, width: right + 2 * PAD, height: bottom + 2 * PAD + HEADER });
	}
	const placed = layoutLayer(outer, topPairs, BOX_GAP);

	return nodes.map((n) => {
		if (n.parentId) {
			const r = inside.get(n.parentId)!.get(n.id)!;
			return { ...n, position: { x: PAD + r.x, y: PAD + HEADER + r.y } };
		}
		const r = placed.get(n.id)!;
		const position = { x: MARGIN + r.x, y: MARGIN + r.y };
		return n.type === 'job' ? { ...n, position, width: r.width, height: r.height } : { ...n, position };
	});
}

/** Whether the edges form a loop. */
function loops(pairs: ReadonlyArray<readonly [string, string]>): boolean {
	const next = Map.groupBy(pairs, ([s]) => s);
	const state = new Map<string, 'open' | 'done'>();
	const visit = (id: string): boolean => {
		if (state.get(id) === 'done') return false;
		if (state.get(id) === 'open') return true;
		state.set(id, 'open');
		const found = (next.get(id) ?? []).some(([, t]) => visit(t));
		state.set(id, 'done');
		return found;
	};
	return [...next.keys()].some(visit);
}

/**
 * Lays the whole graph out at once, boxes and cards together, so lineage between boxes always runs
 * left to right. Columns don't wrap, so a large job draws taller.
 */
function layoutCompound(nodes: Node[], edges: Edge[], size: (n: Node) => { width: number; height: number }, children: Map<string, Node[]>): Node[] {
	const g = new dagre.graphlib.Graph({ compound: true });
	g.setGraph({ rankdir: 'LR', ranksep: 64, nodesep: BOX_GAP, marginx: MARGIN, marginy: MARGIN });
	g.setDefaultEdgeLabel(() => ({}));
	for (const n of nodes) {
		if (n.type === 'job' && children.has(n.id)) g.setNode(n.id, {});
		else if (n.type === 'job') g.setNode(n.id, { width: n.width ?? NODE_WIDTH + 2 * PAD, height: n.height ?? EMPTY_HEIGHT });
		else g.setNode(n.id, size(n));
	}
	for (const n of nodes) if (n.parentId) g.setParent(n.id, n.parentId);
	// Dagre can't route edges to a box with children, so an edge to a box goes to its first card.
	const anchor = (id: string) => children.get(id)?.[0]?.id ?? id;
	for (const e of edges) g.setEdge(anchor(e.source), anchor(e.target));
	dagre.layout(g);

	const topLeft = (n: Node) => {
		const p = g.node(n.id);
		const { width, height } = size(n);
		return { x: p.x - width / 2, y: p.y - height / 2, width, height };
	};
	const boxes = new Map<string, Rect>();
	for (const n of nodes) {
		if (n.type !== 'job') continue;
		const kids = (children.get(n.id) ?? []).map(topLeft);
		if (!kids.length) {
			const p = g.node(n.id);
			const width = n.width ?? NODE_WIDTH + 2 * PAD;
			const height = n.height ?? EMPTY_HEIGHT;
			boxes.set(n.id, { x: p.x - width / 2, y: p.y - height / 2, width, height });
			continue;
		}
		const left = Math.min(...kids.map((k) => k.x)) - PAD;
		const top = Math.min(...kids.map((k) => k.y)) - PAD - HEADER;
		const right = Math.max(...kids.map((k) => k.x + k.width)) + PAD;
		const bottom = Math.max(...kids.map((k) => k.y + k.height)) + PAD;
		boxes.set(n.id, { x: left, y: top, width: right - left, height: bottom - top });
	}
	return nodes.map((n) => {
		const box = boxes.get(n.id);
		if (box) return { ...n, position: { x: box.x, y: box.y }, width: box.width, height: box.height };
		const own = topLeft(n);
		const parent = n.parentId ? boxes.get(n.parentId) : undefined;
		return { ...n, position: parent ? { x: own.x - parent.x, y: own.y - parent.y } : { x: own.x, y: own.y } };
	});
}
