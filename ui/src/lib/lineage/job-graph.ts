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
/** The most empty height left between rows of boxes or cards once a stacked column frees it. */
const BAND_GAP = 40;

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

/**
 * Dagre stacks every node of a rank in one column, so a job whose nodes hang off a few parents, or
 * have no lineage inside the job, draws as one column too tall to read. This gives each node a rank
 * that spreads such a column, within one box, over several: the edges into a node get the rank gap
 * as their minimum length, and a moved node with no edge in hangs off the column's first node by
 * `holds`, edges that are laid out but not drawn.
 */
function wrapColumns(ids: string[], parentOf: (id: string) => string, pairs: ReadonlyArray<readonly [string, string]>) {
	const preds = new Map<string, string[]>();
	for (const [s, t] of pairs) preds.set(t, [...(preds.get(t) ?? []), s]);
	// Each node's rank before wrapping: the longest path to it.
	const depth = new Map<string, number>();
	const depthOf = (id: string): number => {
		if (depth.has(id)) return depth.get(id)!;
		depth.set(id, 0); // A cycle, which declarations can't form, ends here rather than looping.
		const d = Math.max(-1, ...(preds.get(id) ?? []).map(depthOf)) + 1;
		depth.set(id, d);
		return d;
	};
	const columns = new Map<string, string[]>();
	for (const id of ids) {
		const key = `${depthOf(id)}\u0000${parentOf(id)}`;
		columns.set(key, [...(columns.get(key) ?? []), id]);
	}

	const max = maxColumnCards(ids.length);
	const rank = new Map<string, number>();
	const holds: [string, string][] = [];
	let wrapped = false;
	const first = (id: string) => [...(preds.get(id) ?? [])].sort()[0] ?? '';
	for (const column of [...columns.values()].sort((a, b) => depth.get(a[0])! - depth.get(b[0])!)) {
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
 * Lays the graph out left to right. Cards use their measured size; boxes are sized to wrap their
 * cards, and child positions are relative to their box, as Svelte Flow expects.
 */
export function layoutJobGraph(nodes: Node[], edges: Edge[]): Node[] {
	const g = new dagre.graphlib.Graph({ compound: true });
	g.setGraph({ rankdir: 'LR', ranksep: 64, nodesep: 24 + HEADER, marginx: 24, marginy: 24 });
	g.setDefaultEdgeLabel(() => ({}));

	const size = (n: Node) => ({ width: n.measured?.width ?? NODE_WIDTH, height: n.measured?.height ?? NODE_HEIGHT });
	const children = new Map<string, Node[]>();
	for (const n of nodes) if (n.parentId) children.set(n.parentId, [...(children.get(n.parentId) ?? []), n]);

	for (const n of nodes) {
		if (n.type === 'job') {
			// An empty box is laid out as a plain node; dagre sizes boxes with children itself.
			if (children.has(n.id)) g.setNode(n.id, {});
			else g.setNode(n.id, { width: n.width ?? NODE_WIDTH + 2 * PAD, height: n.height ?? EMPTY_HEIGHT });
		} else g.setNode(n.id, size(n));
	}
	for (const n of nodes) if (n.parentId) g.setParent(n.id, n.parentId);

	// Dagre can't route edges to a box with children, so an `after` edge orders the boxes through their first cards.
	const anchor = (id: string) => children.get(id)?.[0]?.id ?? id;
	const pairs = edges.map((e) => [anchor(e.source), anchor(e.target)] as const);
	const parents = new Map(nodes.map((n) => [n.id, n.parentId ?? '']));
	const laidOut = nodes.filter((n) => !children.has(n.id)).map((n) => n.id);
	const { rank, holds, wrapped } = wrapColumns(laidOut, (id) => parents.get(id)!, pairs);
	for (const [s, t] of pairs) g.setEdge(s, t, { minlen: Math.max(1, rank.get(t)! - rank.get(s)!) });
	for (const [s, t] of holds) if (!g.hasEdge(s, t)) g.setEdge(s, t, { minlen: rank.get(t)! - rank.get(s)! });

	dagre.layout(g);

	// Dagre staggers the columns a tall one wrapped into, lining each up with what it reads. Stacking
	// every column from the top of its box keeps the graph compact; a column keeps dagre's order, and
	// so its edge crossings, and its cards only move up within their box. A graph that didn't wrap
	// keeps dagre's alignment.
	const tops = new Map<string, number>();
	const stacks = new Map<string, string[]>();
	for (const id of wrapped ? laidOut : []) {
		const p = g.node(id);
		const parent = parents.get(id)!;
		tops.set(parent, Math.min(tops.get(parent) ?? Infinity, p.y - p.height / 2));
		const key = `${parent}\u0000${Math.round(p.x)}`;
		stacks.set(key, [...(stacks.get(key) ?? []), id]);
	}
	for (const stack of stacks.values()) {
		let y = tops.get(parents.get(stack[0])!)!;
		for (const id of stack.sort((a, b) => g.node(a).y - g.node(b).y)) {
			const p = g.node(id);
			p.y = y + p.height / 2;
			y += p.height + STACK_GAP;
		}
	}

	const topLeft = (n: Node) => {
		const p = g.node(n.id);
		const { width, height } = size(n);
		return { x: p.x - width / 2, y: p.y - height / 2, width, height };
	};
	const boxes = new Map<string, { x: number; y: number; width: number; height: number }>();
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

	// A box whose cards moved up leaves an empty band above the box below it; close every such band
	// across the whole graph. Everything below a band moves up by the same amount, so nothing overlaps.
	const outer = nodes.filter((n) => wrapped && !n.parentId).map((n) => boxes.get(n.id) ?? topLeft(n));
	const bands: { from: number; lift: number }[] = [];
	let lift = 0;
	let reach = -Infinity;
	for (const r of outer.sort((a, b) => a.y - b.y)) {
		if (r.y - reach > BAND_GAP && reach > -Infinity) lift += r.y - reach - BAND_GAP;
		bands.push({ from: r.y, lift });
		reach = Math.max(reach, r.y + r.height);
	}
	const up = (y: number) => bands.findLast((b) => b.from <= y)?.lift ?? 0;

	return nodes.map((n) => {
		const box = boxes.get(n.id);
		if (box) return { ...n, position: { x: box.x, y: box.y - up(box.y) }, width: box.width, height: box.height };
		const own = topLeft(n);
		const parent = n.parentId ? boxes.get(n.parentId) : undefined;
		return { ...n, position: parent ? { x: own.x - parent.x, y: own.y - parent.y } : { x: own.x, y: own.y - up(own.y) } };
	});
}
