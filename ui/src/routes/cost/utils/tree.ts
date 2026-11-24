// The Cost page's tree: a selection's job-by-model facts indexed once into jobs and folders, so a
// level's children, a row's totals and the rows of a tree opened in place are lookups, not scans.
// Pure; no side effects.

import type { CostFact, CostSelection } from '$lib/api';

/** The job key of builds no server run started, which have no job to select. */
export const MANUAL = '';
export const MANUAL_LABEL = 'By hand or CLI';

export type Totals = { credits: number; previous: number; seconds: number; runs: number; queries: number; models: number };

export type RowKind = 'job' | 'folder' | 'model';

/** One row of a level: a job, a folder or a model, with its totals. */
export type DrillRow = Totals & {
	kind: RowKind;
	/** The job (`MANUAL` for builds by hand), folder path or model's node key. */
	key: string;
	label: string;
	/** Whether the row opens onto another level. */
	deeper: boolean;
	/** A muted fact after the name, such as a model's folder in a flat list. */
	sub?: string;
	/** The job the row is in, if any; `MANUAL` for builds by hand. */
	job: string | null;
	/** The folder the row is, or is in. */
	folder: string | null;
};

/** A model's name as people read it: its node key without the kind. */
export const nameOf = (nodeKey: string) => nodeKey.slice(nodeKey.indexOf(':') + 1);

type Acc = { credits: number; previous: number; seconds: number; queries: number; runs: Map<string, number>; models: Set<string> };
const acc = (): Acc => ({ credits: 0, previous: 0, seconds: 0, queries: 0, runs: new Map(), models: new Set() });

function add(a: Acc, f: CostFact, job: string) {
	a.credits += f.credits;
	a.previous += f.previousCredits;
	a.seconds += f.seconds;
	a.queries += f.queries;
	// A job's builds count once however many of its models a group holds.
	a.runs.set(job, Math.max(a.runs.get(job) ?? 0, f.runs));
	a.models.add(f.nodeKey);
}

function totalsOf(a: Acc): Totals {
	let runs = 0;
	for (const n of a.runs.values()) runs += n;
	return { credits: a.credits, previous: a.previous, seconds: a.seconds, queries: a.queries, runs, models: a.models.size };
}

type FolderNode = { path: string; acc: Acc; folders: Map<string, FolderNode>; models: Map<string, { acc: Acc; folder: string }>; rows?: DrillRow[] };
const folderNode = (path: string): FolderNode => ({ path, acc: acc(), folders: new Map(), models: new Map() });

type JobNode = { key: string; acc: Acc; tree: FolderNode; flat?: DrillRow[] };

/** Where the table stands: the job and folder it lists, and whether a job's models show without folders. */
export type Level = { by: 'job' | 'folder'; job: string | null; folder: string | null; flat: boolean };

export type TreeRow = { row: DrillRow; depth: number; id: string; open: boolean; parent: number; peak: number } | { more: number; depth: number; id: string };

export type CostIndex = {
	/** The totals of everything in the read, or of a job, folder or model in it. */
	totals(job: string | null, folder: string | null, model?: string | null): Totals;
	children(level: Level): DrillRow[];
	rows(level: Level, open: ReadonlySet<string>, limit?: number): TreeRow[];
	/** Every row id that opens, to a depth, for Expand all. */
	ids(level: Level, depth: number): string[];
	/** The jobs that spent a model's cost, most expensive first. */
	jobsOf(nodeKey: string): DrillRow[];
	/** A model's folder, if the read has it. */
	folderOf(nodeKey: string): string | undefined;
};

const segments = (folder: string) => (folder ? folder.split('/') : []);
const inside = (folder: string, base: string | null) => !base || folder === base || folder.startsWith(`${base}/`);
const byCost = (a: DrillRow, b: DrillRow) => b.credits - a.credits;

/** Indexes a read's facts once: by job, then by folder down to models, and by folder alone. */
export function indexCost(read: Pick<CostSelection, 'facts'>, job: string | null = null): CostIndex {
	const all = folderNode('');
	const jobs = new Map<string, JobNode>();
	const models = new Map<string, Map<string, Acc>>();
	const folders = new Map<string, string>();
	const everything = acc();
	for (const f of read.facts) {
		const jobKey = f.job ?? MANUAL;
		// A drilled job narrows the read to its own builds.
		if (job !== null && jobKey !== job) continue;
		add(everything, f, jobKey);
		folders.set(f.nodeKey, f.folder);
		let j = jobs.get(jobKey);
		if (!j) jobs.set(jobKey, (j = { key: jobKey, acc: acc(), tree: folderNode('') }));
		add(j.acc, f, jobKey);
		for (const tree of [all, j.tree]) {
			let node = tree;
			add(node.acc, f, jobKey);
			let path = '';
			for (const part of segments(f.folder)) {
				path = path ? `${path}/${part}` : part;
				let next = node.folders.get(part);
				if (!next) node.folders.set(part, (next = folderNode(path)));
				node = next;
				add(node.acc, f, jobKey);
			}
			const m = node.models.get(f.nodeKey) ?? node.models.set(f.nodeKey, { acc: acc(), folder: f.folder }).get(f.nodeKey)!;
			add(m.acc, f, jobKey);
		}
		const per = models.get(f.nodeKey) ?? models.set(f.nodeKey, new Map()).get(f.nodeKey)!;
		add(per.get(jobKey) ?? per.set(jobKey, acc()).get(jobKey)!, f, jobKey);
	}

	const find = (tree: FolderNode, folder: string | null): FolderNode | undefined => {
		let node: FolderNode | undefined = tree;
		for (const part of segments(folder ?? '')) node = node?.folders.get(part);
		return node;
	};
	const treeOf = (job: string | null) => (job === null ? all : jobs.get(job)?.tree);
	const jobRow = (j: JobNode): DrillRow => ({ kind: 'job', key: j.key, label: j.key === MANUAL ? MANUAL_LABEL : j.key, deeper: true, job: j.key, folder: null, ...totalsOf(j.acc) });
	const folderRows = (node: FolderNode, job: string | null): DrillRow[] =>
		(node.rows ??= [
			...[...node.folders.values()].map((n): DrillRow => ({ kind: 'folder', key: n.path, label: n.path.split('/').at(-1)!, deeper: true, job, folder: n.path, ...totalsOf(n.acc) })),
			...[...node.models].map(([key, m]): DrillRow => ({ kind: 'model', key, label: nameOf(key), deeper: false, job, folder: m.folder, ...totalsOf(m.acc) }))
		].sort(byCost));
	const flatRows = (j: JobNode): DrillRow[] => {
		if (j.flat) return j.flat;
		const out: DrillRow[] = [];
		const walk = (node: FolderNode) => {
			for (const [key, m] of node.models) out.push({ kind: 'model', key, label: nameOf(key), deeper: false, sub: m.folder || undefined, job: j.key, folder: m.folder, ...totalsOf(m.acc) });
			for (const n of node.folders.values()) walk(n);
		};
		walk(j.tree);
		return (j.flat = out.sort(byCost));
	};
	let jobRows: DrillRow[] | undefined;
	const children = (level: Level): DrillRow[] => {
		if (level.by === 'job' && level.job === null) return (jobRows ??= [...jobs.values()].map(jobRow).sort(byCost));
		if (level.flat && level.job !== null) {
			const j = jobs.get(level.job);
			return j ? flatRows(j).filter((r) => inside(r.folder ?? '', level.folder)) : [];
		}
		const tree = treeOf(level.by === 'job' ? level.job : null);
		const node = tree && find(tree, level.folder);
		return node ? folderRows(node, level.by === 'job' ? level.job : null) : [];
	};
	/** The level a row opens onto. */
	const below = (level: Level, row: DrillRow): Level => (row.kind === 'job' ? { by: 'job', job: row.key, folder: null, flat: false } : { ...level, folder: row.key, flat: false });

	return {
		totals(job, folder, model = null) {
			if (model) {
				const per = models.get(model);
				const a = acc();
				for (const [j, m] of per ?? []) {
					if (job !== null && j !== job) continue;
					a.credits += m.credits;
					a.previous += m.previous;
					a.seconds += m.seconds;
					a.queries += m.queries;
					for (const [k, n] of m.runs) a.runs.set(k, n);
					a.models.add(model);
				}
				return totalsOf(a);
			}
			if (job === null && folder === null) return totalsOf(everything);
			const tree = treeOf(job);
			const node = tree && find(tree, folder);
			return node ? totalsOf(node.acc) : totalsOf(acc());
		},
		children,
		rows(level, open, limit = 25) {
			const out: TreeRow[] = [];
			const walk = (at: Level, parent: number, depth: number, path: string) => {
				const kids = children(at);
				const peak = kids[0]?.credits ?? 0;
				let shown = 0;
				let hidden = 0;
				for (const row of kids) {
					if (row.kind === 'model' && ++shown > limit) {
						hidden += 1;
						continue;
					}
					const id = `${path}/${row.kind}:${row.key}`;
					const isOpen = row.deeper && open.has(id);
					out.push({ row, depth, id, open: isOpen, parent, peak });
					if (isOpen) walk(below(at, row), row.credits, depth + 1, id);
				}
				if (hidden) out.push({ more: hidden, depth, id: `${path}/more` });
			};
			walk(level, this.totals(level.by === 'job' ? level.job : null, level.folder).credits, 0, '');
			return out;
		},
		ids(level, depth) {
			const ids: string[] = [];
			const walk = (at: Level, d: number, path: string) => {
				if (d >= depth) return;
				for (const row of children(at)) {
					if (!row.deeper) continue;
					const id = `${path}/${row.kind}:${row.key}`;
					ids.push(id);
					walk(below(at, row), d + 1, id);
				}
			};
			walk(level, 0, '');
			return ids;
		},
		jobsOf(nodeKey) {
			const per = models.get(nodeKey);
			if (!per) return [];
			return [...per].map(([j, a]): DrillRow => ({ kind: 'job', key: j, label: j === MANUAL ? MANUAL_LABEL : j, deeper: false, job: j, folder: null, ...totalsOf(a) })).sort(byCost);
		},
		folderOf: (nodeKey) => folders.get(nodeKey)
	};
}

/** The tree row ids from the top down to a model's folder, for opening its ancestors. */
export function folderIds(folder: string, from: string | null): string[] {
	const parts = segments(folder);
	const start = segments(from ?? '').length;
	const ids: string[] = [];
	let path = '';
	for (let i = start; i < parts.length; i++) ids.push((path += `/folder:${parts.slice(0, i + 1).join('/')}`));
	return ids;
}
