// A top-level folder of the catalog (as the Overview and cost pages group nodes) as a selection, so
// links to "this folder" open the selection bar on exactly its nodes. Pure.

import type { CatalogNode } from '$lib/api';
import { shortName } from '$lib/build-area/labels';
import { topFolder } from '$lib/lineage/catalog-graph';

const dirOf = (path: string) => (path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '');

/** The longest folder every one of `dirs` is in, or `''`. */
function commonDir(dirs: string[]): string {
	let common = dirs[0]?.split('/') ?? [];
	for (const d of dirs) {
		const parts = d.split('/');
		let i = 0;
		while (i < common.length && common[i] === parts[i]) i++;
		common = common.slice(0, i);
	}
	return common.join('/');
}

/**
 * Each top-level folder's selection: `path:<folder>` when that folder holds exactly the group's
 * nodes, otherwise the nodes by name (a group the catalog makes up, such as seeds, or one sharing
 * its files' folder with another group). One pass over the nodes, plus one per folder depth.
 */
export function folderSelections(nodes: readonly CatalogNode[]): Map<string, string> {
	const groups = new Map<string, CatalogNode[]>();
	for (const n of nodes) {
		const g = topFolder(n);
		const list = groups.get(g);
		if (list) list.push(n);
		else groups.set(g, [n]);
	}
	const folderOf = new Map<string, string>();
	const groupAt = new Map<string, string>();
	for (const [g, list] of groups) {
		const dir = commonDir(list.map((n) => dirOf(n.path)));
		// A bare root such as `models` holds every model; it never stands for one group.
		if (!dir.includes('/') || groupAt.has(dir)) continue;
		folderOf.set(g, dir);
		groupAt.set(dir, g);
	}
	// A folder is exact only when no other group's node sits anywhere under it.
	const inexact = new Set<string>();
	for (const n of nodes) {
		const own = topFolder(n);
		const parts = dirOf(n.path).split('/');
		for (let i = 1; i <= parts.length; i++) {
			const g = groupAt.get(parts.slice(0, i).join('/'));
			if (g !== undefined && g !== own) inexact.add(g);
		}
	}
	const out = new Map<string, string>();
	for (const [g, list] of groups) {
		const dir = folderOf.get(g);
		out.set(g, dir && !inexact.has(g) ? `path:${dir}` : [...new Set(list.map((n) => shortName(n.key)))].join(' '));
	}
	return out;
}

/** The catalog filtered to a folder's selection. */
export function folderCatalogHref(selection: string): string {
	return `/catalog?${new URLSearchParams({ select: selection })}`;
}
