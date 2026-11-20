// The mock project's node graph and a small selector evaluator. The server
// resolves selectors against the real snapshot; this stands in for it.

import type { NodeKind, SnapshotNode } from '../types';
import type { SelectionProblem, SelectionVocabulary, VocabularyEntry } from '../types/operate';

function node(
	key: string,
	kind: NodeKind,
	group: string,
	upstream: string[],
	options: { tags?: string[]; cursor?: SnapshotNode['cursor']; path?: string } = {}
): SnapshotNode {
	const path =
		options.path ??
		(kind === 'source'
			? 'models/sources.yml'
			: kind === 'seed'
				? `seeds/${key}.csv`
				: kind === 'task' || kind === 'loader'
					? `tasks/${key}.py`
					: `models/${group}/${key}.sql`);
	return { key, kind, group, tags: options.tags ?? [], path, cursor: options.cursor ?? null, upstream };
}

export const SNAPSHOT_NODES: SnapshotNode[] = [
	node('raw__customers', 'source', 'sources', []),
	node('raw__payments', 'source', 'sources', []),
	node('raw__orders', 'loader', 'sources', [], { tags: ['ingest'], cursor: 'timestamp' }),
	node('country_codes', 'seed', 'staging', []),
	node('stg_customers', 'view', 'staging', ['raw__customers']),
	node('stg_orders', 'view', 'staging', ['raw__orders']),
	node('stg_payments', 'view', 'staging', ['raw__payments']),
	node('fact_orders', 'table', 'marts', ['stg_orders', 'stg_payments', 'stg_customers'], { tags: ['finance', 'core'] }),
	node('dim_customers', 'table', 'marts', ['stg_customers', 'fact_orders', 'country_codes'], { tags: ['core'] }),
	node('daily_revenue', 'table', 'marts', ['fact_orders'], { tags: ['finance', 'revenue'] }),
	node('customer_status', 'incremental', 'marts', ['dim_customers'], { cursor: 'timestamp' }),
	node('hourly_order_activity', 'incremental', 'activity', ['fact_orders'], { cursor: 'timestamp' }),
	node('hourly_activity_ctx', 'incremental', 'activity', ['hourly_order_activity', 'daily_revenue'], { cursor: 'timestamp' }),
	node('daily_activity_rollup', 'incremental', 'activity', ['hourly_activity_ctx'], { cursor: 'timestamp' }),
	node('revenue_report', 'table', 'reporting', ['daily_revenue'], { tags: ['finance'] }),
	node('activity_report', 'table', 'reporting', ['hourly_activity_ctx']),
	node('export_revenue_csv', 'task', 'exports', ['revenue_report'], { tags: ['export'] }),
	// Prototype (CHI-1591): finance, inventory and fulfillment folders, so a job run has about 30 nodes.
	node('raw__cost_prices', 'source', 'sources', []),
	node('raw__inventory', 'source', 'sources', []),
	node('raw__products', 'source', 'sources', []),
	node('raw__shipments', 'source', 'sources', []),
	node('stg_cost_prices', 'view', 'finance', ['raw__cost_prices']),
	node('fct_order_margin', 'table', 'finance', ['fact_orders', 'stg_cost_prices'], { tags: ['finance'] }),
	node('agg_margin_daily', 'incremental', 'finance', ['fct_order_margin'], { cursor: 'timestamp', tags: ['finance'] }),
	node('rpt_margin_weekly', 'table', 'finance', ['agg_margin_daily'], { tags: ['finance'] }),
	node('rpt_margin_by_category', 'table', 'finance', ['agg_margin_daily', 'dim_products'], { tags: ['finance'] }),
	node('stg_inventory', 'view', 'inventory', ['raw__inventory']),
	node('stg_products', 'view', 'inventory', ['raw__products']),
	node('dim_products', 'table', 'inventory', ['stg_products']),
	node('fct_stock_levels', 'incremental', 'inventory', ['stg_inventory', 'dim_products'], { cursor: 'timestamp' }),
	node('agg_stock_daily', 'table', 'inventory', ['fct_stock_levels']),
	node('rpt_low_stock', 'table', 'inventory', ['agg_stock_daily', 'dim_products']),
	node('stg_shipments', 'view', 'fulfillment', ['raw__shipments']),
	node('fct_shipments', 'incremental', 'fulfillment', ['stg_shipments', 'fact_orders'], { cursor: 'timestamp' }),
	node('agg_delivery_times', 'table', 'fulfillment', ['fct_shipments']),
	node('rpt_late_deliveries', 'table', 'fulfillment', ['agg_delivery_times', 'dim_customers']),
	// Python nodes (CHI-1591): a loader that prints its paging, and a task that warns.
	node('raw_events', 'loader', 'ingest', [], { cursor: 'timestamp', path: 'models/ingest/raw_events.py' }),
	node('score_customers', 'task', 'ml', ['dim_customers'], { path: 'models/ml/score_customers.py' }),
	// Several checks on one node (CHI-1591): all passing, one warning, two warnings, and a failure with warnings.
	node('stg_sessions', 'view', 'staging', ['raw__customers']),
	node('fct_payment_daily', 'table', 'finance', ['stg_payments']),
	node('fct_order_lines', 'table', 'marts', ['stg_orders', 'stg_payments']),
	node('fct_refunds', 'table', 'finance', ['stg_payments', 'fact_orders']),
	// A Python task that fails with a pandas KeyError traceback after printing a few thousand lines.
	node('score_customers_v2', 'task', 'ml', ['dim_customers'], { path: 'models/ml/score_customers_v2.py' }),
	// A notebook runner that prints about 2 million lines (CHI-1597).
	node('notebook_runner', 'task', 'ml', ['fact_orders'], { path: 'models/ml/notebook_runner.py' })
];

const BY_KEY = new Map(SNAPSHOT_NODES.map((n) => [n.key, n]));
const ORDER = new Map(SNAPSHOT_NODES.map((n, i) => [n.key, i]));
const DOWNSTREAM = new Map<string, string[]>();
for (const n of SNAPSHOT_NODES) {
	for (const up of n.upstream) DOWNSTREAM.set(up, [...(DOWNSTREAM.get(up) ?? []), n.key]);
}

export function snapshotNode(key: string): SnapshotNode | undefined {
	return BY_KEY.get(key);
}

/** Whether runs execute (and therefore write) the node; external sources are only read. */
export function isWritable(key: string): boolean {
	const n = BY_KEY.get(key);
	return !!n && n.kind !== 'source';
}

function closure(start: string, next: (key: string) => string[]): Set<string> {
	const seen = new Set<string>();
	const stack = [...next(start)];
	while (stack.length) {
		const key = stack.pop()!;
		if (seen.has(key)) continue;
		seen.add(key);
		stack.push(...next(key));
	}
	return seen;
}

export function ancestors(key: string): Set<string> {
	return closure(key, (k) => BY_KEY.get(k)?.upstream ?? []);
}

export function descendants(key: string): Set<string> {
	return closure(key, (k) => DOWNSTREAM.get(k) ?? []);
}

/** Lineage edges between the given nodes (direct parents only). */
export function edgesWithin(keys: readonly string[]): { from: string; to: string }[] {
	const set = new Set(keys);
	return keys.flatMap((key) => (BY_KEY.get(key)?.upstream ?? []).filter((up) => set.has(up)).map((up) => ({ from: up, to: key })));
}

export function sortTopologically(keys: Iterable<string>): string[] {
	return [...new Set(keys)].sort((a, b) => (ORDER.get(a) ?? 0) - (ORDER.get(b) ?? 0));
}

function matchTerm(term: string): Set<string> | null {
	const up = term.startsWith('+');
	const down = term.endsWith('+') && term.length > 1;
	const body = term.replace(/^\+/, '').replace(/\+$/, '');
	let base: SnapshotNode[];
	if (body === '*' || body === '') base = SNAPSHOT_NODES;
	else if (body.startsWith('tag:')) base = SNAPSHOT_NODES.filter((n) => n.tags.includes(body.slice(4)));
	else if (body.startsWith('group:')) base = SNAPSHOT_NODES.filter((n) => n.group === body.slice(6));
	else if (body.startsWith('kind:')) base = SNAPSHOT_NODES.filter((n) => n.kind === body.slice(5));
	else if (body.startsWith('path:')) base = SNAPSHOT_NODES.filter((n) => n.path.startsWith(body.slice(5)));
	else {
		const exact = BY_KEY.get(body);
		if (!exact) return null;
		base = [exact];
	}
	const out = new Set(base.map((n) => n.key));
	for (const n of base) {
		if (up) for (const a of ancestors(n.key)) out.add(a);
		if (down) for (const d of descendants(n.key)) out.add(d);
	}
	return out;
}

export type Resolution = { nodes: string[]; unknown: string[] };

/** Union of the `select` terms minus the `exclude` terms; an empty selection means the whole project. */
export function resolveSelection(select: readonly string[], exclude: readonly string[] = []): Resolution {
	const unknown: string[] = [];
	const chosen = new Set<string>();
	const terms = select.length ? select : ['*'];
	for (const term of terms) {
		const hit = matchTerm(term);
		if (!hit) unknown.push(term);
		else for (const k of hit) chosen.add(k);
	}
	for (const term of exclude) {
		const hit = matchTerm(term);
		if (!hit) unknown.push(term);
		else for (const k of hit) chosen.delete(k);
	}
	return { nodes: sortTopologically(chosen), unknown };
}

/** Selected nodes a run would execute and write. */
export function writeSetOf(select: readonly string[], exclude: readonly string[] = []): string[] {
	return resolveSelection(select, exclude).nodes.filter(isWritable);
}

const MODEL_KINDS: ReadonlySet<string> = new Set(['view', 'table', 'incremental']);

/** The kind `kind:` matches: models of every materialization are `model`, as the server names them. */
function selectorKind(n: SnapshotNode): string {
	return MODEL_KINDS.has(n.kind) ? 'model' : n.kind;
}

/** The folder a node's file is in, such as `models/marts`. */
function folderOf(n: SnapshotNode): string {
	return n.path.includes('/') ? n.path.slice(0, n.path.lastIndexOf('/')) : '';
}

/** `job:<key>`: the job's nodes, as `operate.ts` resolves them (set there, which imports this module). */
let jobNodesOf: (key: string) => string[] = () => [];
export function setJobResolver(resolve: (key: string) => string[]) {
	jobNodesOf = resolve;
}

/** One comma part of a selector, with its own `+`; `null` when it matches nothing. */
function matchPart(part: string): Set<string> | null {
	const up = part.startsWith('+');
	const down = part.endsWith('+') && part.length > 1;
	const body = part.replace(/^\+/, '').replace(/\+$/, '');
	const [type, value] = body.includes(':') ? [body.slice(0, body.indexOf(':')), body.slice(body.indexOf(':') + 1)] : ['name', body];
	const jobKeys = type === 'job' ? new Set(jobNodesOf(value)) : null;
	const base = SNAPSHOT_NODES.filter((n) =>
		jobKeys ? jobKeys.has(n.key) :
		type === 'tag' ? n.tags.includes(value) : type === 'kind' ? selectorKind(n) === value : type === 'path' ? folderOf(n) === value || folderOf(n).startsWith(`${value}/`) : n.key === value
	);
	if (!base.length) return null;
	const out = new Set(base.map((n) => n.key));
	for (const n of base) {
		if (up) for (const a of ancestors(n.key)) out.add(a);
		if (down) for (const d of descendants(n.key)) out.add(d);
	}
	return out;
}

/** The mock's stand-in for the server's evaluation: tokens are unioned, comma parts intersected, `+` expands its own part. */
export function evaluateMockSelection(select: string, exclude: string): { keys: string[]; error: SelectionProblem | null } {
	const resolve = (field: 'select' | 'exclude', text: string): Set<string> | SelectionProblem => {
		const chosen = new Set<string>();
		for (const match of text.matchAll(/\S+/g)) {
			const token = match[0];
			let hits = null as Set<string> | null;
			for (const [index, part] of token.split(',').entries()) {
				const found = matchPart(part);
				if (!found) {
					const bare = part.replace(/^\+/, '').replace(/\+$/, '');
					const suggestions = [...new Set(SNAPSHOT_NODES.flatMap((n) => n.tags))].filter((tag) => bare.startsWith('tag:') && tag.startsWith(bare.slice(4, 6))).map((tag) => token.replace(part, part.replace(bare, `tag:${tag}`)));
					const colon = bare.indexOf(':');
					const refused = { part: index, type: colon > 0 ? bare.slice(0, colon) : null, value: colon > 0 ? bare.slice(colon + 1) : bare };
					return { code: 'S007', message: `unknown selector name '${part}'`, help: null, location: { field, token, start: match.index, end: match.index + token.length }, suggestions, refused };
				}
				hits = hits ? new Set([...hits].filter((key) => found.has(key))) : found;
			}
			for (const key of hits ?? []) chosen.add(key);
		}
		return chosen;
	};
	const selected = select.trim() ? resolve('select', select) : new Set(SNAPSHOT_NODES.map((n) => n.key));
	if (!(selected instanceof Set)) return { keys: [], error: selected };
	const excluded = resolve('exclude', exclude);
	if (!(excluded instanceof Set)) return { keys: [], error: excluded };
	const keys = [...selected].filter((key) => !excluded.has(key)).sort();
	if (!keys.length) return { keys, error: { code: null, message: 'the selectors match no nodes in the current publication', help: null, location: null, suggestions: [], refused: null } };
	return { keys, error: null };
}

/** Tags, folders, sources, seeds and kinds of the mock project, with node counts. */
export function mockVocabulary(): Omit<SelectionVocabulary, 'snapshotId' | 'jobs'> {
	const counted = (values: string[]): VocabularyEntry[] => {
		const counts = new Map<string, number>();
		for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1);
		return [...counts].sort(([a], [b]) => a.localeCompare(b)).map(([value, count]) => ({ value, count }));
	};
	const folders = (n: SnapshotNode) => folderOf(n).split('/').filter(Boolean).map((_, i, parts) => parts.slice(0, i + 1).join('/'));
	return {
		tags: counted(SNAPSHOT_NODES.flatMap((n) => n.tags)),
		paths: counted(SNAPSHOT_NODES.filter((n) => n.kind !== 'source' && n.kind !== 'seed').flatMap(folders)),
		sources: counted(SNAPSHOT_NODES.filter((n) => n.kind === 'source').map((n) => n.key)),
		seeds: counted(SNAPSHOT_NODES.filter((n) => n.kind === 'seed').map((n) => n.key)),
		kinds: counted(SNAPSHOT_NODES.map(selectorKind))
	};
}
