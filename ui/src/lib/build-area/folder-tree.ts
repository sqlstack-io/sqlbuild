// The catalog's folders as a tree, each folder with where its nodes stand (its subfolders' included),
// a catalog selection for exactly its nodes, and the nodes sitting in it directly. One pass. Pure.

import type { CatalogNode, GroupStatus } from '$lib/api';
import { shortName } from '$lib/build-area/labels';
import { folderOf } from '$lib/lineage/catalog-graph';
import { needsAttention } from './overview-panels';
import { sortFolders } from './overview-panels';

export type FolderNode = GroupStatus & {
	/** From the project's top, such as `payments/providers`; `group` is its last part. */
	path: string;
	depth: number;
	children: FolderNode[];
	/** Nodes in this folder itself, not in a subfolder. */
	members: CatalogNode[];
	/** A catalog selection of every node in it, subfolders included. */
	selection: string;
};

const dirOf = (path: string) => (path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '');

export type FolderSort = 'urgency' | 'group' | 'current' | 'changed' | 'behind' | 'failed';

/** Folders sorted by a column: most urgent first, names A to Z, or counts largest first; `reverse` turns it round. */
export function sortFolderRows<T extends GroupStatus>(rows: readonly T[], by: FolderSort, reverse = false): T[] {
	const urgent = sortFolders(rows) as T[];
	if (by === 'urgency') return reverse ? urgent.reverse() : urgent;
	const rank = new Map(urgent.map((r, i) => [r, i]));
	const sign = reverse ? -1 : 1;
	return [...rows].sort((a, b) => (by === 'group' ? sign * a.group.localeCompare(b.group) : sign * (b[by] - a[by]) || rank.get(a)! - rank.get(b)!));
}

/** The top-level folders, each holding its subfolders, unsorted. Sources are left out, as on the Overview. */
export function folderTree(nodes: readonly CatalogNode[]): FolderNode[] {
	const byPath = new Map<string, FolderNode & { root: string | null; keys: string[] }>();
	const top: FolderNode[] = [];
	for (const n of nodes) {
		if (n.kind === 'source') continue;
		const folder = folderOf(n);
		const dir = dirOf(n.path);
		// The file-system folder the catalog folder starts at, such as `models/`; null when the folder is a declared group.
		const root = dir === folder || dir.endsWith(`/${folder}`) ? dir.slice(0, dir.length - folder.length) : null;
		const parts = folder.split('/');
		const stale = n.stale ?? [];
		const isChanged = stale.some((s) => s.reason !== 'behind');
		const isBehind = stale.some((s) => s.reason === 'behind');
		const isFailed = n.lastOutcome === 'failed';
		const isCurrent = !stale.length && !isFailed && !!n.lastBuild;
		let parent: FolderNode | null = null;
		for (let i = 0; i < parts.length; i++) {
			const path = parts.slice(0, i + 1).join('/');
			let f = byPath.get(path);
			if (!f) {
				f = { group: parts[i], path, depth: i, total: 0, current: 0, changed: 0, behind: 0, failed: 0, children: [], members: [], selection: '', root, keys: [] };
				byPath.set(path, f);
				(parent ? parent.children : top).push(f);
			}
			if (f.root !== root) f.root = null;
			f.total += 1;
			f.current += Number(isCurrent);
			f.changed += Number(isChanged);
			f.behind += Number(isBehind);
			f.failed += Number(isFailed);
			f.keys.push(shortName(n.key));
			if (i === parts.length - 1) f.members.push(n);
			parent = f;
		}
	}
	for (const f of byPath.values()) {
		f.selection = f.root !== null ? `path:${f.root}${f.path}` : [...new Set(f.keys)].join(' ');
		delete (f as { root?: unknown }).root;
		delete (f as { keys?: unknown }).keys;
	}
	return top;
}

/** Every folder path in the tree, for Expand all. */
export function folderPaths(tree: readonly FolderNode[]): string[] {
	return tree.flatMap((f) => [f.path, ...folderPaths(f.children)]);
}

export type TreeRow =
	| { kind: 'folder'; folder: FolderNode; expandable: boolean; expanded: boolean }
	| { kind: 'node'; node: CatalogNode; depth: number }
	| { kind: 'more'; folder: FolderNode; hidden: number };

const nodeNeedsAttention = (n: CatalogNode) => n.lastOutcome === 'failed' || !!n.stale?.length;

/**
 * The rows on screen: folders sorted within each level, an expanded folder's subfolders under it and,
 * with `listNodes`, its own nodes after them, up to `nodeLimit` and then a row for the rest. With
 * `attentionOnly`, fully current folders and nodes are left out.
 */
export function visibleTreeRows(
	tree: readonly FolderNode[],
	options: { expanded: ReadonlySet<string>; sort: FolderSort; reverse: boolean; attentionOnly: boolean; listNodes: boolean; nodeLimit?: number }
): TreeRow[] {
	const { expanded, sort, reverse, attentionOnly, listNodes, nodeLimit = 20 } = options;
	const rows: TreeRow[] = [];
	const walk = (level: readonly FolderNode[]) => {
		for (const folder of sortFolderRows(attentionOnly ? level.filter(needsAttention) : level, sort, reverse)) {
			const members = attentionOnly ? folder.members.filter(nodeNeedsAttention) : folder.members;
			const expandable = folder.children.some((c) => !attentionOnly || needsAttention(c)) || (listNodes && members.length > 0);
			const open = expandable && expanded.has(folder.path);
			rows.push({ kind: 'folder', folder, expandable, expanded: open });
			if (!open) continue;
			walk(folder.children);
			if (!listNodes) continue;
			const shown = [...members].sort((a, b) => Number(nodeNeedsAttention(b)) - Number(nodeNeedsAttention(a)) || shortName(a.key).localeCompare(shortName(b.key)));
			for (const node of shown.slice(0, nodeLimit)) rows.push({ kind: 'node', node, depth: folder.depth + 1 });
			if (shown.length > nodeLimit) rows.push({ kind: 'more', folder, hidden: shown.length - nodeLimit });
		}
	};
	walk(tree);
	return rows;
}
