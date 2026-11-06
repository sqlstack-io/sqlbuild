// The mock project's node graph and a small selector evaluator. The server
// resolves selectors against the real snapshot; this stands in for it.

import type { NodeKind, SnapshotNode } from '../types';

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
	node('export_revenue_csv', 'task', 'exports', ['revenue_report'], { tags: ['export'] })
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
