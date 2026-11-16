// A job group's member jobs as a graph, drawn as Lineage draws folders: a collapsed member is one
// card, an expanded member a box around its nodes, and edges join whatever is visible at each end.
// Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode, JobGroupMember, RunRef } from '$lib/api';
import { folderTree, scopeGraph, type FolderTree } from './folder-graph';
import { needsBuilding, type NeedsBuilding } from './view-summary';
import { edgeStyle, edgeWidth, visibleEdges, type NodeLink } from './visible-edges';
import { plural } from '$lib/format/plural';

export { needsBuilding };

export const MEMBER_WIDTH = 236;
export const MEMBER_HEIGHT = 82;

/** What the jobs list reports for a member; null where it isn't known. */
export type MemberInfo = {
	nodeCount: number | null;
	lastRun: RunRef | null;
	description?: string;
	/** The member's nodes, by catalog key. */
	nodes?: string[] | null;
};

export type MemberCardData = {
	jobKey: string;
	nodeCount: number | null;
	lastRun: RunRef | null;
	/** How many members this one waits for. */
	waitsFor: number;
	needs: NeedsBuilding;
	/** Whether the member can open in place: its nodes are known. */
	expandable: boolean;
	expanded: boolean;
	ontoggle?: (jobKey: string) => void;
};

/** What is selected on a group's graph: a member job, or one of its nodes drawn inside it. */
export type MemberSelection = { member: string; node?: string };

/** One node-level reason a member waits for another: `from` in the upstream member feeds `to` in the waiting one. */
export type { NodeLink };
export { edgeWidth };

export type MemberEdgeData = {
	/** How many node-level links the edge stands for; 0 for a wait with no lineage between drawn nodes. */
	count: number;
	/** The members it joins, upstream first. */
	from: string;
	to: string;
	/** The tooltip. */
	title: string;
};



export const memberNodeId = (jobKey: string) => `member:${jobKey}`;
/** A node, folder card or folder box drawn inside an expanded member. Members may share nodes, so ids carry the member. */
export const innerId = (jobKey: string, id: string) => `in:${jobKey}:${id}`;
export const pairKey = (upstream: string, member: string) => `${upstream}>${member}`;

/** Each member's waits within the group, without repeats or waits on itself. */
function waitsWithin(members: readonly JobGroupMember[]): Map<string, string[]> {
	const known = new Set(members.map((m) => m.jobKey));
	return new Map(members.map((m) => [m.jobKey, [...new Set(m.upstream.map((u) => u.jobKey))].filter((k) => known.has(k) && k !== m.jobKey)]));
}

/**
 * Why each member waits for another, node by node: a link from a node of the upstream member to a
 * node of the waiting member it feeds, directly or through nodes no member selects (such as a
 * source between a loader job and the models reading it). Lineage through another member's node
 * is that member's link, not this pair's. Keyed by `pairKey`; a pair the server reports with no
 * link here waits for a reason the lineage doesn't show, such as an ordering.
 */
export function memberLinks(members: readonly JobGroupMember[], info: ReadonlyMap<string, MemberInfo>, catalog: readonly CatalogNode[]): Map<string, NodeLink[]> {
	const byKey = new Map(catalog.map((n) => [n.key, n]));
	const nodesOf = new Map(members.map((m) => [m.jobKey, new Set(info.get(m.jobKey)?.nodes ?? [])]));
	const inAny = new Set([...nodesOf.values()].flatMap((s) => [...s]));
	// The member nodes upstream of a node no member selects, through other such nodes.
	const reach = new Map<string, string[]>();
	const reachOf = (key: string): string[] => {
		const known = reach.get(key);
		if (known) return known;
		reach.set(key, []); // A cycle ends here rather than looping.
		const found = new Set<string>();
		for (const up of byKey.get(key)?.upstream ?? []) for (const k of inAny.has(up) ? [up] : reachOf(up)) found.add(k);
		const list = [...found];
		reach.set(key, list);
		return list;
	};
	const links = new Map<string, NodeLink[]>();
	for (const [jobKey, waits] of waitsWithin(members)) {
		const own = nodesOf.get(jobKey)!;
		for (const upstream of waits) {
			const theirs = nodesOf.get(upstream)!;
			const found: NodeLink[] = [];
			const seen = new Set<string>();
			for (const to of own) {
				for (const up of byKey.get(to)?.upstream ?? []) {
					if (own.has(up)) continue;
					for (const from of inAny.has(up) ? [up] : reachOf(up)) {
						if (!theirs.has(from) || own.has(from) || seen.has(`${from}>${to}`)) continue;
						seen.add(`${from}>${to}`);
						found.push({ from, to });
					}
				}
			}
			links.set(pairKey(upstream, jobKey), found);
		}
	}
	return links;
}

export type MemberGraphOptions = {
	catalog?: readonly CatalogNode[];
	/** From `memberLinks`; without it every wait is drawn between member cards. */
	links?: ReadonlyMap<string, NodeLink[]>;
	expanded?: ReadonlySet<string>;
	/** Open folders inside each expanded member drawn by folder. */
	openFolders?: ReadonlyMap<string, ReadonlySet<string>>;
	ontoggle?: (jobKey: string) => void;
	oncloseFolder?: (jobKey: string, folder: string) => void;
};


/**
 * Cards for the members, with each expanded member a box around its nodes (by folder when it has
 * many, folders opening as on Lineage), parents before children as Svelte Flow needs. Edges are drawn
 * at the level you can see: one per pair of collapsed members however many node links join them,
 * ending at the nodes inside an expanded member, and node to node between two expanded members.
 * Positions are left for `layoutFolderGraph`.
 */
export function memberGraph(members: readonly JobGroupMember[], info: ReadonlyMap<string, MemberInfo>, opts: MemberGraphOptions = {}): { nodes: Node[]; edges: Edge[] } {
	const { catalog = [], links = new Map(), expanded = new Set(), openFolders = new Map(), ontoggle, oncloseFolder } = opts;
	const byKey = new Map(catalog.map((n) => [n.key, n]));
	const waits = waitsWithin(members);
	const drawn: Node[] = [];
	const internal: Edge[] = [];
	/** For each expanded member, what stands for one of its nodes. */
	const standIns = new Map<string, (key: string) => string>();

	for (const m of members) {
		const jobKey = m.jobKey;
		const keys = (info.get(jobKey)?.nodes ?? []).filter((k) => byKey.has(k));
		const open = expanded.has(jobKey) && keys.length > 0;
		const data: MemberCardData = {
			jobKey,
			nodeCount: info.get(jobKey)?.nodeCount ?? null,
			lastRun: info.get(jobKey)?.lastRun ?? null,
			waitsFor: waits.get(jobKey)!.length,
			needs: needsBuilding(info.get(jobKey)?.nodes ?? [], byKey),
			expandable: keys.length > 0,
			expanded: open,
			ontoggle
		};
		const id = memberNodeId(jobKey);
		if (!open) {
			drawn.push({ id, type: 'member', position: { x: 0, y: 0 }, width: MEMBER_WIDTH, height: MEMBER_HEIGHT, data });
			continue;
		}
		drawn.push({ id, type: 'memberBox', position: { x: 0, y: 0 }, width: MEMBER_WIDTH, height: MEMBER_HEIGHT, data });
		const nodes = keys.map((k) => byKey.get(k)!);
		const inside = (inner: string) => innerId(jobKey, inner);
		// The member is a scope of its own, drawn by the same rule as Lineage: by folder when large.
		const tree: FolderTree = folderTree(nodes);
		const sub = scopeGraph(nodes, { open: openFolders.get(jobKey) ?? new Set<string>(), tree, onclose: oncloseFolder && ((folder) => oncloseFolder(jobKey, folder)) });
		for (const n of sub.nodes) drawn.push({ ...n, id: inside(n.id), parentId: n.parentId ? inside(n.parentId) : id, extent: 'parent', data: { ...n.data, member: jobKey } });
		for (const e of sub.edges) internal.push({ ...e, id: inside(e.id), source: inside(e.source), target: inside(e.target), style: e.style ?? edgeStyle(1) });
		standIns.set(jobKey, (key) => inside(sub.standIn(key)));
	}

	const at = (jobKey: string, key: string) => standIns.get(jobKey)?.(key) ?? memberNodeId(jobKey);
	const crossing: Edge[] = [];
	for (const m of members) {
		for (const upstream of waits.get(m.jobKey)!) {
			const found = links.get(pairKey(upstream, m.jobKey)) ?? [];
			if (!found.length) {
				const [source, target] = [memberNodeId(upstream), memberNodeId(m.jobKey)];
				const data: MemberEdgeData = { count: 0, from: upstream, to: m.jobKey, title: '' };
				crossing.push({ id: `wait:${source}-${target}`, source, target, type: 'count', style: edgeStyle(1, 'neutral', true), data: { ...data, title: edgeTitle(data) } });
				continue;
			}
			// The same edges as Lineage's folders: one per visible pair, ends standing in for hidden nodes.
			// Each end is keyed with its member, as members may share nodes.
			const pairLinks = found.map((l: NodeLink) => ({ from: `${upstream}\u0000${l.from}`, to: `${m.jobKey}\u0000${l.to}` }));
			const split = (k: string) => k.split('\u0000') as [string, string];
			const title = (count: number) => edgeTitle({ count, from: upstream, to: m.jobKey, title: '' });
			for (const e of visibleEdges(pairLinks, (k) => at(...split(k)), { title })) crossing.push({ ...e, data: { ...e.data, from: upstream, to: m.jobKey } satisfies MemberEdgeData });
		}
	}
	return { nodes: drawn, edges: [...crossing, ...internal] };
}

/** "orders waits for customers: 12 node links", for an edge's tooltip. */
export function edgeTitle(data: MemberEdgeData): string {
	if (!data.count) return `${data.to} waits for ${data.from}, not through lineage between their nodes`;
	return `${data.to} waits for ${data.from}: ${plural(data.count, 'node link')}`;
}

/** A member's waits with their node-level reasons, capped at `max` reasons each; `more` counts the rest. */
export function waitReasons(
	jobKey: string,
	members: readonly JobGroupMember[],
	links: ReadonlyMap<string, NodeLink[]>,
	max = 5
): { upstream: string; reasons: NodeLink[]; more: number }[] {
	return (waitsWithin(members).get(jobKey) ?? []).map((upstream) => {
		const all = links.get(pairKey(upstream, jobKey)) ?? [];
		return { upstream, reasons: all.slice(0, max), more: Math.max(0, all.length - max) };
	});
}
