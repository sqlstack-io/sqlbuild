// A job group's members by the folder of the file each is declared in, drawn as Lineage draws folders:
// a closed folder is one card, an open one a box around its members and subfolders. Pure.

/** A folder of members after merging: a folder whose only content is one subfolder is drawn as one with it. */
export type MemberFolder = {
	/** The full path, such as `jobs/orders/daily`. */
	id: string;
	parent: string | null;
	/** The path below the parent folder. */
	label: string;
	folders: string[];
	/** Members declared directly in the folder, in group order. */
	members: string[];
};

export type MemberFolderTree = {
	folders: Map<string, MemberFolder>;
	/** What the group draws at its top: folders, and members in no folder. */
	roots: string[];
	loose: string[];
	/** Each member's folders, outermost first; empty for a member drawn at the top. */
	chains: Map<string, string[]>;
};

/** A group of more members than this opens with its folders closed, as Lineage opens a large scope by folder. */
export const MEMBER_FOLDERS_CLOSED_ABOVE = 24;

/** The folder of a declaration such as `jobs/orders/daily.py:12`; empty for a file at the top or no file. */
export function declarationFolder(sourceFile: string | null | undefined): string {
	const path = (sourceFile ?? '').replace(/:\d+$/, '');
	const slash = path.lastIndexOf('/');
	return slash > 0 ? path.slice(0, slash) : '';
}

type Trie = { direct: string[]; children: Map<string, Trie> };

const size = (t: Trie): number => t.direct.length + [...t.children.values()].reduce((n, c) => n + size(c), 0);

/**
 * The members' folders. As on Lineage, a folder whose only content is one subfolder merges into it.
 * Folders every member shares are opened (a group declared under `jobs/` draws what is inside it), and
 * a folder holding a single member draws that member's card in its place, rather than a box or card
 * around one job.
 */
export function memberFolderTree(members: readonly string[], folderOf: (member: string) => string): MemberFolderTree {
	const root: Trie = { direct: [], children: new Map() };
	for (const m of members) {
		let at = root;
		for (const part of folderOf(m).split('/').filter(Boolean)) {
			if (!at.children.has(part)) at.children.set(part, { direct: [], children: new Map() });
			at = at.children.get(part)!;
		}
		at.direct.push(m);
	}
	// Open the folders every member shares.
	let top = root;
	let prefix = '';
	while (!top.direct.length && top.children.size === 1) {
		const [[part, only]] = top.children;
		prefix = prefix ? `${prefix}/${part}` : part;
		top = only;
	}
	const folders = new Map<string, MemberFolder>();
	const chains = new Map<string, string[]>();
	const visit = (name: string, trie: Trie, parent: MemberFolder | null, base: string, chain: string[], into: { folders: string[]; members: string[] }) => {
		// One member: its card in the parent's place, without a folder around it.
		if (size(trie) === 1) {
			const only = (function first(t: Trie): string {
				return t.direct[0] ?? first([...t.children.values()][0]);
			})(trie);
			into.members.push(only);
			chains.set(only, chain);
			return;
		}
		let path = base ? `${base}/${name}` : name;
		while (!trie.direct.length && trie.children.size === 1) {
			const [[part, only]] = trie.children;
			path = `${path}/${part}`;
			trie = only;
		}
		const folder: MemberFolder = { id: path, parent: parent?.id ?? null, label: parent ? path.slice(parent.id.length + 1) : path.slice(prefix ? prefix.length + 1 : 0), folders: [], members: [] };
		folders.set(path, folder);
		into.folders.push(path);
		const own = [...chain, path];
		for (const m of trie.direct) {
			folder.members.push(m);
			chains.set(m, own);
		}
		for (const [part, child] of trie.children) visit(part, child, folder, path, own, folder);
	};
	const level = { folders: [] as string[], members: [] as string[] };
	for (const m of top.direct) {
		level.members.push(m);
		chains.set(m, []);
	}
	for (const [part, child] of top.children) visit(part, child, null, prefix, [], level);
	// Members keep the group's order within each level.
	const order = new Map(members.map((m, i) => [m, i]));
	const byOrder = (a: string, b: string) => order.get(a)! - order.get(b)!;
	for (const f of folders.values()) f.members.sort(byOrder);
	return { folders, roots: level.folders, loose: level.members.sort(byOrder), chains };
}

/** What stands for a member: itself when every folder around it is open, else its outermost closed folder. */
export function memberStandIn(tree: MemberFolderTree, open: ReadonlySet<string>): (member: string) => string | null {
	return (member) => (tree.chains.get(member) ?? []).find((f) => !open.has(f)) ?? null;
}

/** Every member in the folder and the folders inside it. */
export function membersIn(tree: MemberFolderTree, folder: string): string[] {
	const f = tree.folders.get(folder);
	return f ? [...f.members, ...f.folders.flatMap((sub) => membersIn(tree, sub))] : [];
}
