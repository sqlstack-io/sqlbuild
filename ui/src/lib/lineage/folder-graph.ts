// The lineage drawn by folder: a closed folder is one card, an open folder a box around what it
// holds, and boxes nest. Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { CatalogNode } from '$lib/api';
import { cardData, folderOf, groupCard, type GroupCard } from './catalog-graph';
import { layoutLayer, type LayoutItem } from './job-graph';

/**
 * The most cards and boxes the browser lays out at once. Beyond it the page asks the person to open
 * a folder instead of laying out thousands of nodes.
 */
export const MAX_DRAWN = 600;

/** Whether a graph has more cards and boxes than the browser lays out. */
export const tooManyToDraw = (graph: { nodes: unknown[] }) => graph.nodes.length > MAX_DRAWN;

const NODE_WIDTH = 236;
const NODE_HEIGHT = 61;
const PAD = 14;
const HEADER = 30;
/** Wide enough for a header's chevron, name and counts. */
const MIN_BOX_WIDTH = 300;
const CARD_GAP = 24;
const MARGIN = 24;

/** A folder after merging: a folder whose only content is one subfolder is drawn as one with it. */
export type Folder = {
	/** The full path of the innermost merged folder, such as `orders/fulfilment/staging`. */
	id: string;
	parent: string | null;
	/** The path below the parent folder, such as `fulfilment/staging` inside `orders`. */
	label: string;
	folders: string[];
	/** Nodes directly in the folder, in listing order. */
	nodes: string[];
};

export type FolderTree = {
	folders: Map<string, Folder>;
	roots: string[];
	/** Each node's folders, outermost first. */
	chains: Map<string, string[]>;
};

type Trie = { direct: string[]; children: Map<string, Trie> };

/** The folders the nodes live in, with every folder whose only content is one subfolder merged into it. */
export function folderTree(nodes: CatalogNode[]): FolderTree {
	const root: Trie = { direct: [], children: new Map() };
	for (const node of nodes) {
		let at = root;
		for (const part of folderOf(node).split('/')) {
			if (!at.children.has(part)) at.children.set(part, { direct: [], children: new Map() });
			at = at.children.get(part)!;
		}
		at.direct.push(node.key);
	}
	const folders = new Map<string, Folder>();
	const chains = new Map<string, string[]>();
	const visit = (name: string, trie: Trie, parent: Folder | null, chain: string[]): string => {
		let path = parent ? `${parent.id}/${name}` : name;
		while (!trie.direct.length && trie.children.size === 1) {
			const [[part, only]] = trie.children;
			path = `${path}/${part}`;
			trie = only;
		}
		const folder: Folder = { id: path, parent: parent?.id ?? null, label: parent ? path.slice(parent.id.length + 1) : path, folders: [], nodes: trie.direct };
		folders.set(path, folder);
		const own = [...chain, path];
		for (const key of trie.direct) chains.set(key, own);
		for (const [part, child] of trie.children) folder.folders.push(visit(part, child, folder, own));
		return path;
	};
	const roots = [...root.children].map(([name, trie]) => visit(name, trie, null, []));
	return { folders, roots, chains };
}

export const folderCardId = (folder: string) => `group:${folder}`;
export const folderBoxId = (folder: string) => `folder:${folder}`;

/** `open` with every folder around the node open, so the node itself is drawn. */
export function withFoldersOpen(tree: FolderTree, key: string, open: ReadonlySet<string>): Set<string> {
	return new Set([...open, ...(tree.chains.get(key) ?? [])]);
}

/** `open` without the folder and every folder inside it. */
export function withFolderClosed(open: ReadonlySet<string>, folder: string): Set<string> {
	return new Set([...open].filter((f) => f !== folder && !f.startsWith(`${folder}/`)));
}

/** What stands for a node in the folder graph: the node when every folder around it is open, otherwise the outermost closed folder's card. */
export function standInOf(tree: FolderTree, open: ReadonlySet<string>): (key: string) => string {
	return (key) => {
		const closed = (tree.chains.get(key) ?? []).find((f) => !open.has(f));
		return closed === undefined ? key : folderCardId(closed);
	};
}

/** Every node in the folder and the folders inside it. */
function members(tree: FolderTree, folder: string, byKey: Map<string, CatalogNode>): CatalogNode[] {
	const f = tree.folders.get(folder)!;
	return [...f.nodes.map((k) => byKey.get(k)!), ...f.folders.flatMap((sub) => members(tree, sub, byKey))];
}

export type FolderBoxData = { folder: string; label: string; nodeCount: number; stale: number; failed: number; ontoggle?: (folder: string) => void };

/**
 * The lineage with each closed folder as one card and each open folder as a box around its
 * subfolders and nodes, parents before children as Svelte Flow needs. An edge joins whatever stands
 * for each end, once per pair: the node when every folder around it is open, otherwise the
 * outermost closed folder. Boxes take no edges; an edge inside a closed folder disappears.
 */
export function folderGraph(
	nodes: CatalogNode[],
	open: ReadonlySet<string>,
	{ tree = folderTree(nodes), onclose }: { tree?: FolderTree; onclose?: (folder: string) => void } = {}
): { nodes: Node[]; edges: Edge[] } {
	const byKey = new Map(nodes.map((n) => [n.key, n]));
	const drawn: Node[] = [];
	const place = (folder: string, box: string | null) => {
		const inside = box ? { parentId: folderBoxId(box), extent: 'parent' as const } : {};
		const f = tree.folders.get(folder)!;
		const card: GroupCard = groupCard(folder, members(tree, folder, byKey));
		if (!open.has(folder)) {
			drawn.push({ id: folderCardId(folder), type: 'folder', position: { x: 0, y: 0 }, selectable: false, ...inside, data: { ...card, label: f.label } });
			return;
		}
		const data: FolderBoxData = { folder, label: f.label, nodeCount: card.count, stale: card.stale, failed: card.failed, ontoggle: onclose };
		// Sized before layout, which sizes it around its contents, so cards kept inside it have room.
		drawn.push({ id: folderBoxId(folder), type: 'folderBox', position: { x: 0, y: 0 }, width: MIN_BOX_WIDTH, height: NODE_HEIGHT + 2 * PAD + HEADER, selectable: false, ...inside, data });
		for (const sub of f.folders) place(sub, folder);
		for (const key of f.nodes) {
			drawn.push({ id: key, type: 'asset', position: { x: 0, y: 0 }, parentId: folderBoxId(folder), extent: 'parent', data: cardData(byKey.get(key)!) });
		}
	};
	for (const root of tree.roots) place(root, null);

	const standIn = standInOf(tree, open);
	const pairs = new Map<string, Edge>();
	for (const n of nodes) {
		const target = standIn(n.key);
		for (const up of n.upstream) {
			if (!byKey.has(up)) continue;
			const source = standIn(up);
			const id = `${source}-${target}`;
			if (source !== target && !pairs.has(id)) pairs.set(id, { id, source, target, type: 'bezier' });
		}
	}
	return { nodes: drawn, edges: [...pairs.values()] };
}

/**
 * Lays out boxes from the inside out: each box's cards and inner boxes first, which sizes the box,
 * then the box among its siblings. Lineage between two boxes' contents orders the boxes themselves.
 * Any node with children is a box; a card keeps its measured size, or the size it was given.
 * Positions inside a box are relative to it, as Svelte Flow expects.
 */
export function layoutFolderGraph(nodes: Node[], edges: Edge[]): Node[] {
	if (!nodes.length) return [];
	const parentOf = new Map(nodes.map((n) => [n.id, n.parentId ?? null]));
	const children = Map.groupBy(nodes, (n) => n.parentId ?? '');
	const sizes = new Map<string, { width: number; height: number }>();
	const rects = new Map<string, { x: number; y: number }>();
	/** The item directly inside `container` that holds `id`, or null when `container` does not hold it. */
	const lift = (id: string, container: string): string | null => {
		let at: string | null = id;
		while (at !== null && (parentOf.get(at) ?? '') !== container) at = parentOf.get(at) ?? null;
		return at;
	};
	const place = (container: string): { width: number; height: number } => {
		const items: LayoutItem[] = (children.get(container) ?? []).map((n) => {
			// A box is whatever holds other nodes: an open folder, or an expanded member job on a group's page.
			if (n.type === 'folderBox' || children.has(n.id)) return { id: n.id, ...place(n.id) };
			return { id: n.id, width: n.measured?.width ?? n.width ?? NODE_WIDTH, height: n.measured?.height ?? n.height ?? NODE_HEIGHT };
		});
		const seen = new Set<string>();
		const pairs = edges.flatMap((e) => {
			const [s, t] = [lift(e.source, container), lift(e.target, container)];
			if (s === null || t === null || s === t || seen.has(`${s}>${t}`)) return [];
			seen.add(`${s}>${t}`);
			return [[s, t] as const];
		});
		const placed = layoutLayer(items, pairs, CARD_GAP);
		for (const [id, r] of placed) rects.set(id, { x: r.x, y: r.y });
		const width = Math.max(...[...placed.values()].map((r) => r.x + r.width));
		const height = Math.max(...[...placed.values()].map((r) => r.y + r.height));
		const size = container ? { width: Math.max(MIN_BOX_WIDTH, width + 2 * PAD), height: height + 2 * PAD + HEADER } : { width, height };
		if (container) sizes.set(container, size);
		return size;
	};
	place('');
	return nodes.map((n) => {
		const r = rects.get(n.id)!;
		const position = n.parentId ? { x: PAD + r.x, y: PAD + HEADER + r.y } : { x: MARGIN + r.x, y: MARGIN + r.y };
		const size = sizes.get(n.id);
		return size ? { ...n, position, width: size.width, height: size.height } : { ...n, position };
	});
}
