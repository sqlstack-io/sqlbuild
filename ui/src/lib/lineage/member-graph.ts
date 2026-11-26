// A job group's member jobs as a graph, drawn as Lineage draws folders: a collapsed member is one
// card, an expanded member a box around its nodes, and edges join whatever is visible at each end.
// Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode, JobGroupMember, RunRef } from '$lib/api';
import { folderTree, layoutFolderGraph, scopeGraph, type FolderTree } from './folder-graph';
import { needsBuilding, type NeedsBuilding } from './view-summary';
import { edgeStyle, edgesTouching, edgeWidth, visibleEdges, type NodeLink } from './visible-edges';
import { plural } from '$lib/format/plural';
import { membersIn, memberStandIn, type MemberFolderTree } from './member-folders';
import type { FolderBoxData } from './folder-graph';
import type { GroupCard } from './catalog-graph';

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
	/** Where the member is declared, such as `jobs/orders/daily.py:12`; its folder groups it on the graph. */
	sourceFile?: string | null;
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
	/** Every member wait the edge stands for, when a closed folder of members stands at an end. */
	pairs?: { from: string; to: string }[];
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
 * The edges no path through the other edges implies, in their order. Each implied edge is dropped in
 * turn, so what stays still joins everything the input joined, even round a cycle; `droppable` says
 * which edges may go at all. On a graph without cycles this is its transitive reduction.
 */
export function transitiveReduction<E extends { source: string; target: string }>(edges: readonly E[], droppable: (e: E) => boolean = () => true): E[] {
	const kept = new Set(edges.map((_, i) => i));
	const out = Map.groupBy(edges.keys(), (i) => edges[i].source);
	const reaches = (from: string, to: string, skip: number) => {
		const seen = new Set([from]);
		const stack = [from];
		while (stack.length) {
			for (const i of out.get(stack.pop()!) ?? []) {
				if (i === skip || !kept.has(i)) continue;
				const next = edges[i].target;
				if (next === to) return true;
				if (!seen.has(next)) {
					seen.add(next);
					stack.push(next);
				}
			}
		}
		return false;
	};
	for (const [i, e] of edges.entries()) if (e.source !== e.target && droppable(e) && reaches(e.source, e.target, i)) kept.delete(i);
	return edges.filter((_, i) => kept.has(i));
}

/**
 * Each member's waits split in two: `direct`, the waits drawn, and `implied`, the waits that follow
 * from them through other members (A waits for B and B for C, so A's wait for C is implied). The
 * server lists every wait, implied ones too; drawing those turns a large group into a tangle.
 */
export function directWaits(members: readonly JobGroupMember[]): { direct: Map<string, string[]>; implied: Map<string, string[]> } {
	const waits = waitsWithin(members);
	const all = [...waits].flatMap(([member, ups]) => ups.map((up) => ({ source: up, target: member })));
	const kept = new Set(transitiveReduction(all).map((e) => pairKey(e.source, e.target)));
	const direct = new Map<string, string[]>();
	const implied = new Map<string, string[]>();
	for (const [member, ups] of waits) {
		direct.set(member, ups.filter((up) => kept.has(pairKey(up, member))));
		implied.set(member, ups.filter((up) => !kept.has(pairKey(up, member))));
	}
	return { direct, implied };
}

/** Every member each member waits for, directly or through others, and every member waiting for it. */
export function memberReach(members: readonly JobGroupMember[]): { up: Map<string, Set<string>>; down: Map<string, Set<string>> } {
	const waits = waitsWithin(members);
	const after = new Map<string, string[]>();
	for (const [member, ups] of waits) for (const up of ups) after.set(up, [...(after.get(up) ?? []), member]);
	const closure = (start: string, next: (k: string) => string[]) => {
		const seen = new Set<string>();
		const stack = [...next(start)];
		while (stack.length) {
			const k = stack.pop()!;
			if (seen.has(k) || k === start) continue;
			seen.add(k);
			stack.push(...next(k));
		}
		return seen;
	};
	const keys = [...waits.keys()];
	return {
		up: new Map(keys.map((k) => [k, closure(k, (x) => waits.get(x) ?? [])])),
		down: new Map(keys.map((k) => [k, closure(k, (x) => after.get(x) ?? [])]))
	};
}

/**
 * The edges lit when a card is hovered or selected. For a member, its whole upstream and downstream:
 * every drawn edge between members it waits for, directly or not, and between members waiting for
 * it, so the waits the drawing leaves implied still light up; plus what touches its box. For a node
 * inside a member, just the edges that touch it.
 */
export function litEdges(focus: string, edges: readonly Edge[], nodes: readonly Pick<Node, 'id' | 'parentId'>[], reach: ReturnType<typeof memberReach>): Set<string> {
	const lit = edgesTouching(focus, edges, nodes);
	const prefix = memberNodeId('');
	const member = focus.startsWith(prefix) ? focus.slice(prefix.length) : null;
	if (member === null) return lit;
	const up = reach.up.get(member) ?? new Set<string>();
	const down = reach.down.get(member) ?? new Set<string>();
	for (const e of edges) {
		const d = e.data as Partial<MemberEdgeData> | undefined;
		if (!d?.from || !d.to) continue;
		for (const { from, to } of d.pairs ?? [{ from: d.from, to: d.to }]) {
			if ((up.has(from) && (to === member || up.has(to))) || (down.has(to) && (from === member || down.has(from)))) lit.add(e.id);
		}
	}
	return lit;
}

/**
 * Why each member waits for another, node by node, as the server publishes it (`reasons`), keyed
 * by `pairKey` like `memberLinks`; a wait with none is direct with no lineage, such as an ordering.
 * Null when no member has a reason: a snapshot published before reasons were recorded sends every
 * member's list empty, which can't be told from a group whose waits are all orderings, so the page
 * works the reasons out from lineage instead (`memberLinks`), which finds none for the latter too.
 */
export function publishedLinks(members: readonly JobGroupMember[]): Map<string, NodeLink[]> | null {
	if (!members.some((m) => m.reasons.length)) return null;
	const links = new Map<string, NodeLink[]>();
	for (const [jobKey, waits] of waitsWithin(members)) for (const upstream of waits) links.set(pairKey(upstream, jobKey), []);
	for (const m of members) {
		for (const r of m.reasons) {
			const key = pairKey(r.upstreamJob, m.jobKey);
			if (!links.has(key)) continue;
			links.get(key)!.push({ from: r.fromNode, to: r.toNode });
		}
	}
	return links;
}

/**
 * Why each member waits for another, node by node, worked out from lineage, for a server that
 * doesn't publish it (`publishedLinks`): a link from a node of the upstream member to a
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
	/** Members drawn by the folder of their declaration: closed folders as cards, open ones as boxes. */
	folders?: { tree: MemberFolderTree; open: ReadonlySet<string>; onclose?: (folder: string) => void };
};

/** A closed folder of members, and an open one's box. */
export const memberFolderCardId = (folder: string) => `jobfolder:${folder}`;
export const memberFolderBoxId = (folder: string) => `jobfolderbox:${folder}`;


/**
 * Cards for the members, with each expanded member a box around its nodes (by folder when it has
 * many, folders opening as on Lineage), parents before children as Svelte Flow needs. Edges are drawn
 * at the level you can see: one per pair of collapsed members however many node links join them,
 * ending at the nodes inside an expanded member, and node to node between two expanded members.
 * Positions are left for `layoutFolderGraph`.
 */
export function memberGraph(members: readonly JobGroupMember[], info: ReadonlyMap<string, MemberInfo>, opts: MemberGraphOptions = {}): { nodes: Node[]; edges: Edge[] } {
	const { catalog = [], links = new Map(), expanded = new Set(), openFolders = new Map(), ontoggle, oncloseFolder, folders } = opts;
	const byKey = new Map(catalog.map((n) => [n.key, n]));
	const waits = directWaits(members).direct;
	const drawn: Node[] = [];
	const internal: Edge[] = [];
	/** For each expanded member, what stands for one of its nodes. */
	const standIns = new Map<string, (key: string) => string>();

	const drawMember = (m: JobGroupMember, box: string | null) => {
		const jobKey = m.jobKey;
		const placed = box ? { parentId: box, extent: 'parent' as const } : {};
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
			drawn.push({ id, type: 'member', position: { x: 0, y: 0 }, width: MEMBER_WIDTH, height: MEMBER_HEIGHT, ...placed, data });
			return;
		}
		drawn.push({ id, type: 'memberBox', position: { x: 0, y: 0 }, width: MEMBER_WIDTH, height: MEMBER_HEIGHT, ...placed, data });
		const nodes = keys.map((k) => byKey.get(k)!);
		const inside = (inner: string) => innerId(jobKey, inner);
		// The member is a scope of its own, drawn by the same rule as Lineage: by folder when large.
		const tree: FolderTree = folderTree(nodes);
		const sub = scopeGraph(nodes, { open: openFolders.get(jobKey) ?? new Set<string>(), tree, onclose: oncloseFolder && ((folder) => oncloseFolder(jobKey, folder)) });
		for (const n of sub.nodes) drawn.push({ ...n, id: inside(n.id), parentId: n.parentId ? inside(n.parentId) : id, extent: 'parent', data: { ...n.data, member: jobKey } });
		for (const e of sub.edges) internal.push({ ...e, id: inside(e.id), source: inside(e.source), target: inside(e.target), style: e.style ?? edgeStyle(1) });
		standIns.set(jobKey, (key) => inside(sub.standIn(key)));
	};

	// By folder: each closed folder a card, each open one a box around its subfolders and members.
	const byMember = new Map(members.map((m) => [m.jobKey, m]));
	const folderStandIn = folders ? memberStandIn(folders.tree, folders.open) : () => null;
	if (folders) {
		const { tree, open, onclose } = folders;
		const facts = (folder: string) => {
			const inside = membersIn(tree, folder).filter((k) => byMember.has(k));
			const status = (k: string) => info.get(k)?.lastRun?.status;
			const needs = (k: string) => needsBuilding(info.get(k)?.nodes ?? [], byKey);
			return {
				count: inside.length,
				ok: inside.filter((k) => status(k) === 'success').length,
				stale: inside.filter((k) => needs(k).changed + needs(k).behind > 0).length,
				failed: inside.filter((k) => status(k) === 'failed').length
			};
		};
		const place = (folder: string, box: string | null) => {
			const f = tree.folders.get(folder)!;
			const placed = box ? { parentId: box, extent: 'parent' as const } : {};
			const fact = facts(folder);
			if (!fact.count) return;
			if (!open.has(folder)) {
				const data: GroupCard & { memberFolder: string } = { group: folder, label: f.label, ...fact, noun: 'job', empty: 'No runs', memberFolder: folder };
				drawn.push({ id: memberFolderCardId(folder), type: 'folder', position: { x: 0, y: 0 }, selectable: false, ...placed, data });
				return;
			}
			const data: FolderBoxData = { folder, label: f.label, nodeCount: fact.count, stale: fact.stale, failed: fact.failed, noun: 'job', ontoggle: onclose };
			const id = memberFolderBoxId(folder);
			drawn.push({ id, type: 'folderBox', position: { x: 0, y: 0 }, width: MEMBER_WIDTH, height: MEMBER_HEIGHT, selectable: false, ...placed, data });
			for (const sub of f.folders) place(sub, folder);
			for (const k of f.members) if (byMember.has(k)) drawMember(byMember.get(k)!, id);
		};
		for (const folder of tree.roots) place(folder, null);
		const inTree = new Set(tree.chains.keys());
		for (const m of members) if (!inTree.has(m.jobKey) || !(tree.chains.get(m.jobKey) ?? []).length) drawMember(m, null);
	} else {
		for (const m of members) drawMember(m, null);
	}

	/** What stands for a member's node: the member's closed folder, its card, or what is drawn of the node inside it. */
	const memberAt = (jobKey: string) => {
		const folder = folderStandIn(jobKey);
		return folder === null ? memberNodeId(jobKey) : memberFolderCardId(folder);
	};
	const at = (jobKey: string, key: string) => (folderStandIn(jobKey) === null ? (standIns.get(jobKey)?.(key) ?? memberNodeId(jobKey)) : memberAt(jobKey));
	const crossing: Edge[] = [];
	for (const m of members) {
		for (const upstream of waits.get(m.jobKey)!) {
			const found = links.get(pairKey(upstream, m.jobKey)) ?? [];
			if (!found.length) {
				const [source, target] = [memberAt(upstream), memberAt(m.jobKey)];
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
	// Waits whose ends a closed folder stands in for are one edge per pair of what is drawn, as Lineage's
	// folders are; a wait inside one closed folder isn't drawn.
	const merged = new Map<string, Edge>();
	for (const e of crossing) {
		if (e.source === e.target) continue;
		const key = `${e.source}>${e.target}`;
		const d = e.data as MemberEdgeData;
		const had = merged.get(key);
		if (!had) {
			merged.set(key, { ...e, data: { ...d, pairs: [{ from: d.from, to: d.to }] } });
			continue;
		}
		const h = had.data as MemberEdgeData;
		const pairs = h.pairs!.some((p) => p.from === d.from && p.to === d.to) ? h.pairs! : [...h.pairs!, { from: d.from, to: d.to }];
		const count = h.count + d.count;
		const data: MemberEdgeData = { ...h, count, pairs, title: pairs.length > 1 ? `${plural(pairs.length, 'wait')}, ${plural(count, 'node link')}` : h.title };
		merged.set(key, { ...had, id: `${had.source}-${had.target}`, style: edgeStyle(Math.max(1, count), 'neutral', count === 0), data });
	}
	const visible = [...merged.values()];
	// A wait implied by a path of other drawn edges isn't drawn either, now that hidden nodes stand
	// in for their member or folder; lineage between two open members' nodes always is.
	const collapsed = (id: string) => id.startsWith(memberNodeId('')) || id.startsWith(memberFolderCardId(''));
	const between = new Set(visible);
	const reduced = new Set(transitiveReduction([...visible, ...internal], (e) => between.has(e) && (collapsed(e.source) || collapsed(e.target))));
	return { nodes: drawn, edges: [...visible.filter((e) => reduced.has(e)), ...internal] };
}

/** Space between the cards of the block of members that wait for nothing and that nothing waits for. */
const BLOCK_GAP = 24;

/**
 * `layoutFolderGraph`, with the collapsed members no drawn edge touches set apart in a compact block
 * under the graph, rows as wide as the graph (or a square, if the graph is narrow), rather than a
 * long column beside it.
 */
export function layoutMemberGraph(nodes: Node[], edges: Edge[]): Node[] {
	const touched = new Set(edges.flatMap((e) => [e.source, e.target]));
	const alone = new Set(nodes.filter((n) => !n.parentId && n.type === 'member' && !touched.has(n.id)).map((n) => n.id));
	if (alone.size < 2) return layoutFolderGraph(nodes, edges);
	const rest = nodes.filter((n) => !alone.has(n.id));
	const placed = rest.length ? layoutFolderGraph(rest, edges) : [];
	const top = placed.filter((n) => !n.parentId);
	const left = top.length ? Math.min(...top.map((n) => n.position.x)) : BLOCK_GAP;
	const right = top.length ? Math.max(...top.map((n) => n.position.x + (n.width ?? MEMBER_WIDTH))) : left;
	const below = top.length ? Math.max(...top.map((n) => n.position.y + (n.height ?? MEMBER_HEIGHT))) + 2 * BLOCK_GAP : BLOCK_GAP;
	const size = (n: Node) => ({ width: n.measured?.width ?? n.width ?? MEMBER_WIDTH, height: n.measured?.height ?? n.height ?? MEMBER_HEIGHT });
	const cards = nodes.filter((n) => alone.has(n.id)).map(size);
	const cardWidth = Math.max(...cards.map((c) => c.width));
	const cardHeight = Math.max(...cards.map((c) => c.height));
	const columns = Math.min(alone.size, Math.max(Math.ceil(Math.sqrt(alone.size)), Math.floor((right - left + BLOCK_GAP) / (cardWidth + BLOCK_GAP))));
	const at = new Map([...alone].map((id, i) => [id, { x: left + (i % columns) * (cardWidth + BLOCK_GAP), y: below + Math.floor(i / columns) * (cardHeight + BLOCK_GAP) }]));
	const byId = new Map(placed.map((n) => [n.id, n]));
	return nodes.map((n) => (alone.has(n.id) ? { ...n, position: at.get(n.id)! } : byId.get(n.id)!));
}

/** "orders waits for customers: 12 node links", for an edge's tooltip. */
export function edgeTitle(data: MemberEdgeData): string {
	if (!data.count) return `${data.to} waits for ${data.from}, not through lineage between their nodes`;
	return `${data.to} waits for ${data.from}: ${plural(data.count, 'node link')}`;
}

/**
 * A member's waits with their node-level reasons, capped at `max` reasons each; `more` counts the
 * rest. Direct waits come first; an implied wait names `through`, the direct wait it comes after.
 */
export function waitReasons(
	jobKey: string,
	members: readonly JobGroupMember[],
	links: ReadonlyMap<string, NodeLink[]>,
	max = 5
): { upstream: string; reasons: NodeLink[]; more: number; through?: string }[] {
	const { direct, implied } = directWaits(members);
	const { up } = memberReach(members);
	const reasons = (upstream: string) => {
		const all = links.get(pairKey(upstream, jobKey)) ?? [];
		return { upstream, reasons: all.slice(0, max), more: Math.max(0, all.length - max) };
	};
	const mine = direct.get(jobKey) ?? [];
	return [...mine.map(reasons), ...(implied.get(jobKey) ?? []).map((upstream) => ({ ...reasons(upstream), through: mine.find((d) => up.get(d)?.has(upstream)) }))];
}
