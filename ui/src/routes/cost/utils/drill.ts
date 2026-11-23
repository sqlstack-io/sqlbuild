// The cost explorer's levels: what a scope (a job, a folder, a model, or a job's folder) holds, its
// children ranked by cost, and the rows of a tree opened in place. Pure; no side effects.
import type { CostFact, CostPage, CostRun } from '../types';

export type GroupRoot = 'job' | 'folder';
/** Where the explorer is: under a job and/or a folder, or on one model. `flat` lists a job's models without folders. */
export type Scope = {
	by: GroupRoot;
	job: string | null;
	folder: string | null;
	model: string | null;
	flat: boolean;
	/** Only these models, for a selection the tree can't break down; null for all. */
	models?: ReadonlySet<string> | null;
};
export type Level = 'jobs' | 'folders' | 'models' | 'model';

export type DrillRow = {
	kind: 'job' | 'folder' | 'model';
	/** The job name, folder path or model name. */
	key: string;
	label: string;
	credits: number;
	previous: number;
	seconds: number;
	runs: number;
	queries: number;
	models: number;
	/** Whether the row opens onto another level. */
	deeper: boolean;
	/** A muted fact after the name, such as a model's folder in a flat list. */
	sub?: string;
};

export const ROOT: Scope = { by: 'job', job: null, folder: null, model: null, flat: false };

export function levelOf(s: Scope): Level {
	if (s.model) return 'model';
	if (s.by === 'job' && !s.job) return 'jobs';
	return s.flat ? 'models' : 'folders';
}

const inFolder = (folder: string, base: string | null) => !base || folder === base || folder.startsWith(`${base}/`);

/** The job-and-model pairs a scope covers. */
export function factsIn(cost: CostPage, s: Scope): CostFact[] {
	return cost.facts.filter((f) => (!s.job || f.job === s.job) && inFolder(f.folder, s.folder) && (!s.model || f.model === s.model) && (!s.models || s.models.has(f.model)));
}

/** Runs of a group: each job's runs once, however many of its models the group holds. */
function runsOf(facts: CostFact[]): number {
	const seen = new Map<string, number>();
	for (const f of facts) seen.set(f.job, f.runs);
	return [...seen.values()].reduce((t, n) => t + n, 0);
}

export function total(facts: CostFact[]): Omit<DrillRow, 'kind' | 'key' | 'label' | 'deeper'> {
	let credits = 0;
	let previous = 0;
	let seconds = 0;
	let queries = 0;
	for (const f of facts) {
		credits += f.credits;
		previous += f.previous;
		seconds += f.seconds;
		queries += f.queries;
	}
	return { credits, previous, seconds, queries, runs: runsOf(facts), models: new Set(facts.map((f) => f.model)).size };
}

function grouped(facts: CostFact[], key: (f: CostFact) => string): Map<string, CostFact[]> {
	const out = new Map<string, CostFact[]>();
	for (const f of facts) {
		const k = key(f);
		const list = out.get(k);
		if (list) list.push(f);
		else out.set(k, [f]);
	}
	return out;
}

/** The scope's children, most expensive first: jobs, a folder's subfolders and own models, or a flat model list. */
export function childrenOf(cost: CostPage, s: Scope): DrillRow[] {
	const facts = factsIn(cost, s);
	const level = levelOf(s);
	let rows: DrillRow[] = [];
	if (level === 'jobs') {
		rows = [...grouped(facts, (f) => f.job)].map(([key, fs]) => ({ kind: 'job', key, label: key, deeper: true, ...total(fs) }));
	} else if (level === 'models') {
		rows = [...grouped(facts, (f) => f.model)].map(([key, fs]) => ({ kind: 'model', key, label: key, deeper: false, sub: fs[0].folder, ...total(fs) }));
	} else if (level === 'folders') {
		const base = s.folder;
		const next = (f: CostFact) => {
			if (f.folder === base) return `m:${f.model}`;
			const rest = base ? f.folder.slice(base.length + 1) : f.folder;
			return `f:${base ? `${base}/` : ''}${rest.split('/')[0]}`;
		};
		rows = [...grouped(facts, next)].map(([id, fs]) => {
			const key = id.slice(2);
			return id.startsWith('f:') ? { kind: 'folder', key, label: key.split('/').at(-1)!, deeper: true, ...total(fs) } : { kind: 'model', key, label: key, deeper: false, ...total(fs) };
		});
	}
	return rows.sort((a, b) => b.credits - a.credits);
}

/** The scope one row opens onto. */
export function scopeFor(s: Scope, row: Pick<DrillRow, 'kind' | 'key'>): Scope {
	if (row.kind === 'job') return { ...s, job: row.key, folder: null, model: null, flat: false };
	if (row.kind === 'folder') return { ...s, folder: row.key, model: null, flat: false };
	return { ...s, model: row.key };
}

export type Crumb = { label: string; scope: Scope };

/** All, then the job, each folder on the way down, and the model. */
export function crumbsOf(s: Scope): Crumb[] {
	const base: Scope = { by: s.by, job: null, folder: null, model: null, flat: false };
	const out: Crumb[] = [{ label: s.by === 'job' ? 'All jobs' : 'All folders', scope: base }];
	let at = base;
	if (s.job) out.push({ label: s.job, scope: (at = { ...at, job: s.job }) });
	if (s.folder) {
		const parts = s.folder.split('/');
		parts.forEach((p, i) => out.push({ label: p, scope: (at = { ...at, folder: parts.slice(0, i + 1).join('/') }) }));
	}
	if (s.model) out.push({ label: s.model, scope: { ...at, model: s.model, flat: s.flat } });
	return out;
}

/** The scope's cost per day: each job's daily cost times the share of it the scope holds. */
export function dailyOf(cost: CostPage, facts: CostFact[]): number[] {
	const shares = new Map<string, number>();
	for (const f of facts) shares.set(f.job, (shares.get(f.job) ?? 0) + f.share);
	const out = cost.daily.map(() => 0);
	for (const [job, share] of shares) (cost.jobDaily[job] ?? []).forEach((v, i) => (out[i] += v * share));
	return out;
}

/** The runs that spent the scope's cost, each costed at the share of it the scope holds. */
export function runsIn(cost: CostPage, facts: CostFact[], manual: string): CostRun[] {
	const share = new Map<string, number>();
	const models = new Map<string, number>();
	for (const f of facts) {
		share.set(f.job, (share.get(f.job) ?? 0) + f.share);
		models.set(f.job, (models.get(f.job) ?? 0) + 1);
	}
	return cost.runs
		.filter((r) => share.has(r.job ?? manual))
		.map((r) => {
			const s = share.get(r.job ?? manual)!;
			const n = models.get(r.job ?? manual)!;
			return { ...r, credits: r.credits * s, seconds: r.seconds * s, queries: Math.round((r.queries / Math.max(1, r.nodes)) * n), nodes: n };
		})
		.sort((a, b) => b.credits - a.credits);
}

/** The scope's cost by warehouse size, from the size each job runs on. */
export function sizesIn(cost: CostPage, facts: CostFact[]): DrillRow[] {
	return [...grouped(facts, (f) => cost.jobSize[f.job] ?? 'Unknown size')]
		.map(([key, fs]) => ({ kind: 'job' as const, key, label: key, deeper: false, ...total(fs) }))
		.sort((a, b) => b.credits - a.credits);
}

export type TreeRow = { row: DrillRow; depth: number; parent: number; id: string; open: boolean; /** The largest sibling's cost, for bars relative to it. */ peak?: number } | { more: number; depth: number; id: string; scope: Scope };
export const rowId = (r: Pick<DrillRow, 'kind' | 'key'>) => `${r.kind}:${r.key}`;

/**
 * The rows of a tree opened in place, as the Catalog's folder tree draws them: each open row's
 * children under it, the first `limit` models of a folder and then a "more" row.
 * `foldersOnly` leaves models out, for a tree of jobs and folders beside a table.
 */
export function treeRows(cost: CostPage, s: Scope, open: ReadonlySet<string>, opts: { limit?: number; foldersOnly?: boolean } = {}): TreeRow[] {
	const out: TreeRow[] = [];
	const limit = opts.limit ?? 25;
	const walk = (scope: Scope, parentCredits: number, depth: number, path: string) => {
		const kids = childrenOf(cost, scope).filter((r) => !opts.foldersOnly || r.kind !== 'model');
		let models = 0;
		for (const r of kids) {
			if (r.kind === 'model' && ++models > limit) continue;
			const id = `${path}/${rowId(r)}`;
			const deeper = r.deeper && (!opts.foldersOnly || childrenOf(cost, scopeFor(scope, r)).some((c) => c.kind !== 'model'));
			const isOpen = deeper && open.has(id);
			out.push({ row: { ...r, deeper }, depth, parent: parentCredits, id, open: isOpen, peak: kids[0]?.credits ?? 0 });
			if (isOpen) walk(scopeFor(scope, r), r.credits, depth + 1, id);
		}
		if (models > limit) out.push({ more: models - limit, depth, id: `${path}/more`, scope });
	};
	walk(s, total(factsIn(cost, s)).credits, 0, '');
	return out;
}

/** Every openable row id of a tree, for "Expand all", to a depth. */
export function treeIds(cost: CostPage, s: Scope, depth: number, foldersOnly = false): string[] {
	const ids: string[] = [];
	const walk = (scope: Scope, d: number, path: string) => {
		if (d >= depth) return;
		for (const r of childrenOf(cost, scope)) {
			if (!r.deeper || (foldersOnly && r.kind === 'model')) continue;
			const id = `${path}/${rowId(r)}`;
			ids.push(id);
			walk(scopeFor(scope, r), d + 1, id);
		}
	};
	walk(s, 0, '');
	return ids;
}

/** The scope in the URL, beside the page's other parameters. */
export function scopeParams(params: URLSearchParams, s: Scope): URLSearchParams {
	const out = new URLSearchParams(params);
	for (const k of ['by', 'job', 'folder', 'model', 'flat']) out.delete(k);
	if (s.by === 'folder') out.set('by', 'folder');
	if (s.job) out.set('job', s.job);
	if (s.folder) out.set('folder', s.folder);
	if (s.model) out.set('model', s.model);
	if (s.flat) out.set('flat', '1');
	return out;
}

export function scopeOf(params: URLSearchParams): Scope {
	const by: GroupRoot = params.get('by') === 'folder' ? 'folder' : 'job';
	return { by, job: by === 'job' ? params.get('job') : null, folder: params.get('folder'), model: params.get('model'), flat: params.get('flat') === '1' };
}

/** The tree row ids from the root down to a scope's job or folder, for opening its ancestors. */
export function pathIds(s: Scope): string[] {
	const ids: string[] = [];
	let path = '';
	if (s.job) ids.push((path += `/job:${s.job}`));
	if (s.folder) {
		const parts = s.folder.split('/');
		parts.forEach((_, i) => ids.push((path += `/folder:${parts.slice(0, i + 1).join('/')}`)));
	}
	return ids;
}

/** The scope a tree row id stands for. */
export function scopeFromId(by: GroupRoot, id: string): Scope {
	const s: Scope = { by, job: null, folder: null, model: null, flat: false };
	for (const seg of id.split('/').filter(Boolean)) {
		const [kind, ...rest] = seg.split(':');
		const key = rest.join(':');
		if (kind === 'job') s.job = key;
		else if (kind === 'folder') s.folder = key;
		else if (kind === 'model') s.model = key;
	}
	return s;
}
