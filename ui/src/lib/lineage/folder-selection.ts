// The lineage's folder filter: which folders the canvas draws, the stubs that stand for lineage
// outside them, and how the selection is kept in the URL. Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode } from '$lib/api';
import { catalogGraph } from './catalog-graph';
import { folderGraph, folderTree, standInOf, type FolderTree } from './folder-graph';

/** The URL parameter holding the selected folders, comma-separated. */
export const FOLDERS_PARAM = 'folders';

/** The URL parameter holding nodes picked elsewhere, e.g. a provider's code, comma-separated keys. */
export const NODES_PARAM = 'nodes';

/** The picked node keys the catalog knows, from the URL parameter, in order and without repeats. */
export function parseNodes(param: string | null, known: ReadonlySet<string>): string[] {
	if (!param) return [];
	return [...new Set(param.split(',').map((k) => k.trim()))].filter((k) => k && known.has(k));
}

/** A link to the Lineage drawing only the given nodes. */
export const lineageNodesHref = (keys: readonly string[]): string => `/lineage?${NODES_PARAM}=${keys.map(encodeURIComponent).join(',')}`;

/** Whether `folder` is `other` or a folder inside it. */
const within = (folder: string, other: string) => folder === other || folder.startsWith(`${other}/`);

/** The selection with `folder` added, dropping folders inside it; adding a folder already inside the selection changes nothing. */
export function withFolderAdded(selected: readonly string[], folder: string): string[] {
	if (selected.some((s) => within(folder, s))) return [...selected];
	return [...selected.filter((s) => !within(s, folder)), folder];
}

/** Ctrl/⌘-click or the checkbox: removes a selected folder, or adds one that is not selected. */
export function withFolderToggled(selected: readonly string[], folder: string): string[] {
	return selected.includes(folder) ? selected.filter((s) => s !== folder) : withFolderAdded(selected, folder);
}

/** The selected folders the tree knows, from the URL parameter; unknown folders are dropped. */
export function parseFolders(param: string | null, tree: FolderTree): string[] {
	if (!param) return [];
	return param
		.split(',')
		.map((f) => f.trim())
		.filter((f) => f && tree.folders.has(f))
		.reduce<string[]>((acc, f) => withFolderAdded(acc, f), []);
}

/** The URL parameter for a selection, or null when nothing is selected. */
export const foldersParam = (selected: readonly string[]): string | null => (selected.length ? selected.join(',') : null);

/** Whether the node lives in one of the selected folders or a folder inside one. */
export function inSelection(tree: FolderTree, key: string, selected: readonly string[]): boolean {
	const chain = tree.chains.get(key) ?? [];
	return selected.some((s) => chain.includes(s));
}

/** The folders to open so the selected folders are drawn as boxes: each selected folder and every folder around it. */
export function foldersToOpen(tree: FolderTree, selected: readonly string[]): string[] {
	const open: string[] = [];
	for (const s of selected) {
		for (let at: string | null = s; at !== null; at = tree.folders.get(at)?.parent ?? null) open.push(at);
	}
	return open;
}

/**
 * The folder a stub names for a node outside the selection: its outermost folder that neither
 * holds a selected folder nor sits inside one, or its own folder when every folder around it holds
 * part of the selection.
 */
export function stubFolder(chain: readonly string[], selected: readonly string[]): string {
	return chain.find((f) => !selected.some((s) => within(s, f) || within(f, s))) ?? chain.at(-1) ?? '';
}

export type Stub = {
	id: string;
	direction: 'upstream' | 'downstream';
	folder: string;
	/** Distinct nodes outside the selection in the folder that feed, or are fed by, the selection directly. */
	count: number;
	/** The selected nodes those nodes join. */
	targets: string[];
};

export const stubId = (direction: Stub['direction'], folder: string) => `stub:${direction}:${folder}`;

/**
 * Direct lineage between the selection and other folders, one stub per folder and direction. The
 * selection is the selected folders, or the picked nodes when `picked` is given.
 */
export function selectionStubs(nodes: readonly CatalogNode[], tree: FolderTree, selected: readonly string[], picked?: ReadonlySet<string>): Stub[] {
	if (!selected.length && !picked?.size) return [];
	const keys = new Set(nodes.map((n) => n.key));
	const inside = new Set(nodes.filter((n) => (picked ? picked.has(n.key) : inSelection(tree, n.key, selected))).map((n) => n.key));
	const stubs = new Map<string, { stub: Stub; members: Set<string>; targets: Set<string> }>();
	const note = (direction: Stub['direction'], outsider: string, target: string) => {
		const folder = stubFolder(tree.chains.get(outsider) ?? [], selected);
		const id = stubId(direction, folder);
		const entry = stubs.get(id) ?? { stub: { id, direction, folder, count: 0, targets: [] }, members: new Set<string>(), targets: new Set<string>() };
		entry.members.add(outsider);
		entry.targets.add(target);
		stubs.set(id, entry);
	};
	for (const n of nodes) {
		for (const up of n.upstream) {
			if (!keys.has(up)) continue;
			if (inside.has(n.key) && !inside.has(up)) note('upstream', up, n.key);
			if (!inside.has(n.key) && inside.has(up)) note('downstream', n.key, up);
		}
	}
	return [...stubs.values()]
		.map(({ stub, members, targets }) => ({ ...stub, count: members.size, targets: [...targets].sort() }))
		.sort((a, b) => a.direction.localeCompare(b.direction) || b.count - a.count || a.folder.localeCompare(b.folder));
}

/** e.g. "+12 upstream in customers". */
export const stubLabel = (stub: Pick<Stub, 'count' | 'direction' | 'folder'>) => `+${stub.count.toLocaleString('en')} ${stub.direction} in ${stub.folder}`;

export type StubData = { folder: string; label: string; direction: Stub['direction']; onadd?: (folder: string) => void; dimmed?: boolean };

export type LineageGraphOptions = {
	grouped: boolean;
	/** Folders drawn open as boxes, when drawing by folder. */
	open: ReadonlySet<string>;
	/** The selected folders; empty draws every folder. */
	selected: readonly string[];
	/** Picked nodes, drawn instead of the selected folders, with stubs for lineage outside them. */
	picked?: ReadonlySet<string>;
	/** Only nodes that are out of date. */
	staleOnly?: boolean;
	tree?: FolderTree;
	onclose?: (folder: string) => void;
	onadd?: (folder: string) => void;
};

/** The nodes the lineage draws: the picked nodes, or those in the selected folders (every node when none), only those out of date with `staleOnly`. */
export function shownNodes(
	data: CatalogNode[],
	{ selected, picked, staleOnly = false, tree = folderTree(data) }: Pick<LineageGraphOptions, 'selected' | 'picked' | 'staleOnly' | 'tree'>
): CatalogNode[] {
	let shown = data;
	if (picked?.size) shown = shown.filter((n) => picked.has(n.key));
	else if (selected.length) shown = shown.filter((n) => inSelection(tree, n.key, selected));
	if (staleOnly) shown = shown.filter((n) => (n.stale?.length ?? 0) > 0);
	return shown;
}

/**
 * The lineage to draw: every node, or the nodes in the selected folders with a stub per folder and
 * direction for lineage outside them; by folder or node by node. `staleOnly` narrows to nodes that
 * are out of date, without stubs.
 */
export function lineageGraph(data: CatalogNode[], { grouped, open, selected, picked, staleOnly = false, tree = folderTree(data), onclose, onadd }: LineageGraphOptions): { nodes: Node[]; edges: Edge[] } {
	const shown = shownNodes(data, { selected, picked, staleOnly, tree });
	const picking = !!picked?.size;
	const narrowed = shown.length !== data.length;
	const shownTree = narrowed ? folderTree(shown) : tree;
	let graph: { nodes: Node[]; edges: Edge[] };
	let standIn: (key: string) => string = (key) => key;
	if (grouped) {
		graph = folderGraph(shown, open, { tree: shownTree, onclose });
		standIn = standInOf(shownTree, open);
	} else {
		graph = catalogGraph(shown);
		if (narrowed) {
			const keys = new Set(shown.map((n) => n.key));
			graph = { nodes: graph.nodes, edges: graph.edges.filter((e) => keys.has(e.source)) };
		}
	}
	if ((!selected.length && !picking) || staleOnly) return graph;
	const stubs = picking ? selectionStubs(data, tree, [], picked) : selectionStubs(data, tree, selected);
	const stubNodes: Node[] = stubs.map((s) => ({
		id: s.id,
		type: 'stub',
		position: { x: 0, y: 0 },
		selectable: false,
		data: { folder: s.folder, label: stubLabel(s), direction: s.direction, onadd } satisfies StubData
	}));
	const seen = new Set<string>();
	const stubEdges: Edge[] = stubs.flatMap((s) =>
		s.targets.flatMap((t) => {
			const end = standIn(t);
			const [source, target] = s.direction === 'upstream' ? [s.id, end] : [end, s.id];
			const id = `${source}-${target}`;
			if (seen.has(id)) return [];
			seen.add(id);
			return [{ id, source, target, type: 'bezier' }];
		})
	);
	return { nodes: [...graph.nodes, ...stubNodes], edges: [...graph.edges, ...stubEdges] };
}
