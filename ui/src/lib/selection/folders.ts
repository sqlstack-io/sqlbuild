// A top-level folder of the catalog (as the Overview and cost pages group nodes) as a selection, so
// links to "this folder" open the selection bar on exactly its nodes. Pure.

import type { CatalogNode } from '$lib/api';
import { shortName } from '$lib/build-area/labels';
import { topFolder } from '$lib/lineage/catalog-graph';

const dirOf = (path: string) => (path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '');

/** The project folders a `path:` selector can start at; any other, such as `seeds/`, is an error (S012). */
const PATH_ROOTS = ['models', 'python'];
export const canSelectByPath = (dir: string) => PATH_ROOTS.some((root) => dir.startsWith(`${root}/`));

/**
 * Of `dirs`, those a `path:` selector would over-select: a node sits somewhere under one but doesn't
 * `belong` to it, such as another group's node, or a source declared in a .yml file there. One pass
 * over the nodes, plus one per folder depth.
 */
export function inexactDirs(nodes: readonly CatalogNode[], dirs: ReadonlySet<string>, belongs: (n: CatalogNode, dir: string) => boolean): Set<string> {
	const inexact = new Set<string>();
	for (const n of nodes) {
		const parts = dirOf(n.path).split('/');
		for (let i = 1; i <= parts.length; i++) {
			const dir = parts.slice(0, i).join('/');
			if (dirs.has(dir) && !belongs(n, dir)) inexact.add(dir);
		}
	}
	return inexact;
}

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
		// A bare root such as `models` holds every model; it never stands for one group. Nor can a folder
		// outside the roots a path selector accepts.
		if (!canSelectByPath(dir) || groupAt.has(dir)) continue;
		folderOf.set(g, dir);
		groupAt.set(dir, g);
	}
	// A folder is exact only when no other group's node sits anywhere under it.
	const inexact = inexactDirs(nodes, new Set(groupAt.keys()), (n, dir) => groupAt.get(dir) === topFolder(n));
	const out = new Map<string, string>();
	for (const [g, list] of groups) {
		const dir = folderOf.get(g);
		out.set(g, dir && !inexact.has(dir) ? `path:${dir}` : [...new Set(list.map((n) => shortName(n.key)))].join(' '));
	}
	return out;
}

/** The catalog filtered to a folder's selection. */
export function folderCatalogHref(selection: string): string {
	return `/catalog?${new URLSearchParams({ select: selection })}`;
}
