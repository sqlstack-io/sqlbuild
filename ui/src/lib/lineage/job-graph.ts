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

export type JobBox = { jobKey: string; nodes: string[]; lastRun?: RunRef | null };

const edgeStyle = 'stroke: var(--sb-edge); stroke-width: 1.5;';
const afterStyle = 'stroke: var(--sb-edge); stroke-width: 1.5; stroke-dasharray: 5 4;';

export function boxId(jobKey: string): string {
	return `job:${jobKey}`;
}

/**
 * Cards for `nodes` and the lineage between them. With `boxes`, each node sits in the first box
 * that builds it, and a declared `after` between two members with no lineage between them is a
 * dashed edge between their boxes.
 */
export function jobGraph(nodes: string[], catalog: CatalogNode[], boxes: JobBox[] = [], members: JobGroupMember[] = []): { nodes: Node[]; edges: Edge[] } {
	const byKey = new Map(catalog.map((n) => [n.key, n]));
	const inJob = new Set(nodes);
	const owner = new Map<string, string>();
	for (const box of boxes) for (const n of box.nodes) if (inJob.has(n) && !owner.has(n)) owner.set(n, box.jobKey);

	const boxNodes: Node[] = boxes.map((b) => ({
		id: boxId(b.jobKey),
		type: 'job',
		position: { x: 0, y: 0 },
		width: NODE_WIDTH + 2 * PAD,
		height: EMPTY_HEIGHT,
		selectable: false,
		data: { jobKey: b.jobKey, lastRun: b.lastRun ?? null, nodeCount: nodes.filter((n) => owner.get(n) === b.jobKey).length }
	}));
	const cards: Node[] = nodes.map((key) => {
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

	const lineage: Edge[] = nodes.flatMap((key) =>
		(byKey.get(key)?.upstream ?? []).filter((up) => inJob.has(up)).map((up) => ({ id: `${up}-${key}`, source: up, target: key, type: 'bezier', style: edgeStyle }))
	);
	const linked = new Set(lineage.map((e) => `${owner.get(e.source)}>${owner.get(e.target)}`));
	const after: Edge[] = members.flatMap((m) =>
		m.upstream
			.filter((u) => !linked.has(`${u.jobKey}>${m.jobKey}`) && boxes.some((b) => b.jobKey === u.jobKey))
			.map((u) => ({ id: `after:${u.jobKey}-${m.jobKey}`, source: boxId(u.jobKey), target: boxId(m.jobKey), type: 'bezier', style: afterStyle }))
	);
	// Svelte Flow needs a parent before its children.
	return { nodes: [...boxNodes, ...cards], edges: [...lineage, ...after] };
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
			else g.setNode(n.id, { width: NODE_WIDTH + 2 * PAD, height: EMPTY_HEIGHT });
		} else g.setNode(n.id, size(n));
	}
	for (const n of nodes) if (n.parentId) g.setParent(n.id, n.parentId);

	// Dagre can't route edges to a box with children, so an `after` edge orders the boxes through their first cards.
	const anchor = (id: string) => children.get(id)?.[0]?.id ?? id;
	for (const e of edges) g.setEdge(anchor(e.source), anchor(e.target));

	dagre.layout(g);

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
			boxes.set(n.id, { x: p.x - (NODE_WIDTH + 2 * PAD) / 2, y: p.y - EMPTY_HEIGHT / 2, width: NODE_WIDTH + 2 * PAD, height: EMPTY_HEIGHT });
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
