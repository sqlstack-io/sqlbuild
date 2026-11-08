import type { components } from '$control/schema.d.ts';
import type { CatalogNode, CatalogNodeDetail, Check, CheckCounts, StaleReason } from './types/build';
import type { NodeKind } from './types/operate';

type Wire = components['schemas'];

/** The catalog shows models, seeds and sources; audits, functions, macros and hooks are not catalog nodes. */
function kindOf(wire: Wire['CatalogNodeRecord']): NodeKind | null {
	if (wire.kind === 'source' || wire.kind === 'seed') return wire.kind;
	if (wire.kind !== 'model') return null;
	if (wire.materialization === 'view') return 'view';
	return wire.materialization?.startsWith('incremental') ? 'incremental' : 'table';
}

/**
 * One node of the current snapshot. The server reports declared metadata and the newest
 * fingerprint, not run outcomes, staleness, check counts, owners or groups, so those stay null
 * or empty and the pages hide them. The fingerprint's run is a `sqb` run, not a scheduler run.
 */
export function toCatalogNode(wire: Wire['CatalogNodeRecord']): CatalogNode | null {
	const kind = kindOf(wire);
	if (!kind) return null;
	return {
		key: wire.node_key,
		kind,
		materialization: kind === 'source' || kind === 'seed' ? null : (wire.materialization ?? null),
		group: null,
		tags: wire.tags,
		owner: [],
		description: wire.description ?? '',
		path: wire.path ?? '',
		upstream: wire.dependencies,
		relation: wire.relation ? { database: wire.relation.database ?? null, schema: wire.relation.schema, name: wire.relation.name } : null,
		columnCount: wire.column_count ?? 0,
		lastBuild: wire.last_build
			? { fingerprintRunId: wire.last_build.run_id, runId: null, jobKey: null, builtAt: wire.last_build.observed_at, fingerprint: wire.last_build.version_hash }
			: null,
		lastOutcome: null,
		stale: null,
		sourceFreshness: null,
		checks: null
	};
}

/** Catalog nodes in listing order, keeping only dependencies that are themselves listed. */
export function toCatalogNodes(wire: Wire['CatalogNodeRecord'][]): CatalogNode[] {
	const nodes = wire.flatMap((node) => toCatalogNode(node) ?? []);
	const listed = new Set(nodes.map((node) => node.key));
	return nodes.map((node) => ({ ...node, upstream: node.upstream.filter((key) => listed.has(key)) }));
}

/** Passing, warning and failing by each check's newest result; checks without one count only towards the total. */
export function checkCountsOf(checks: Check[]): CheckCounts {
	const outcome = (c: Check) => c.latest?.outcome;
	return {
		total: checks.length,
		passing: checks.filter((c) => outcome(c) === 'pass').length,
		warning: checks.filter((c) => outcome(c) === 'warn').length,
		failing: checks.filter((c) => ['fail', 'error'].includes(outcome(c) ?? '')).length
	};
}

/** A definition that changed since its last recorded build here; other staleness is not reported by the server. */
function staleOf(status: Wire['NodeStatus'] | null): StaleReason[] {
	if (status?.definition.status !== 'changed' || !status.build) return [];
	return [{ reason: 'changed', change: 'definition', detail: 'The definition changed since it was last built here.', since: status.build.observed_at }];
}

/** A node key without its kind, e.g. `customer_summary` for `model:customer_summary`. */
export const shortName = (key: string) => key.slice(key.indexOf(':') + 1);

/** The listed node a key or bare name refers to, preferring a model; migrations name models without their kind. */
export function findNode(nodes: CatalogNode[], ref: string): CatalogNode | null {
	return nodes.find((n) => n.key === ref) ?? nodes.find((n) => n.key === `model:${ref}`) ?? nodes.find((n) => shortName(n.key) === ref) ?? null;
}

/**
 * A listed node with its status and SQL reads and its checks. The server keeps no
 * per-node run history, row counts, cost, jobs or migrations, so those stay empty or null and the
 * pages hide them. `sql` is null when the person may not see SQL.
 */
export function nodeDetailOf(nodes: CatalogNode[], node: CatalogNode, status: Wire['NodeStatus'] | null, sql: Wire['NodeSql'] | null, checks: Check[]): CatalogNodeDetail {
	// Checks name their node without its kind, so the attachment tells a source from a model of the same name.
	const own = checks.filter((c) => c.node === node.key || `${c.attachment}:${c.node}` === node.key);
	const physical = status?.physical.status === 'observed' ? status.physical.observation.record : null;
	return {
		...node,
		stale: staleOf(status),
		checks: checkCountsOf(own),
		downstream: nodes.filter((n) => n.upstream.includes(node.key)).map((n) => n.key),
		recentOutcomes: [],
		lastRows: null,
		rowsDelta: null,
		lastDurationMs: null,
		cost: null,
		columns: (physical?.columns ?? []).map((c) => ({ name: c.name, type: c.data_type, description: '', audits: [] })),
		sql: sql?.raw_sql ? { raw: sql.raw_sql, compiled: sql.compiled_sql ?? sql.raw_sql } : null,
		code: null,
		builds: [],
		jobs: null,
		runs: null,
		checkResults: own,
		warehouse: null,
		microbatch: null,
		history: null
	};
}
