import type { components } from '$control/schema.d.ts';
import { shortName } from '$lib/build-area/labels';
import { cardStatus } from '$lib/lineage/catalog-graph';
import type { CatalogNode, CatalogNodeDetail, Check, CheckCounts, ColumnLens, ColumnLineage, MicrobatchCoverage, NodeJob, NodeOutcome, NodeRunEntry, StaleReason, WarehouseObject } from './types/build';
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


/** The listed node a key or bare name refers to, preferring a model; migrations name models without their kind. */
export function findNode(nodes: CatalogNode[], ref: string): CatalogNode | null {
	return nodes.find((n) => n.key === ref) ?? nodes.find((n) => n.key === `model:${ref}`) ?? nodes.find((n) => shortName(n.key) === ref) ?? null;
}

/** Jobs and groups whose resolved selection includes the node, with the triggers that start each. */
export function nodeJobsOf(nodeKey: string, jobs: Wire['JobRecord'][], triggers: Wire['TriggerRecord'][]): NodeJob[] {
	return jobs
		.filter((job) => job.selection.includes(nodeKey))
		.map((job) => ({ key: job.job_key, kind: job.kind === 'group' ? 'group' : 'job', triggerIds: triggers.filter((t) => t.job_key === job.job_key).map((t) => t.trigger_id) }));
}

const FINISHED: readonly NodeOutcome[] = ['success', 'warn', 'failed', 'skipped'];

/** Finished builds of the node by scheduler runs, newest first; queued, running and cancelled attempts are left out. */
export function toNodeRuns(wire: Wire['NodeRunRecord'][]): NodeRunEntry[] {
	return wire.flatMap((r) => {
		const outcome = FINISHED.find((o) => o === r.status);
		if (!outcome) return [];
		const started = r.started_at ?? r.queued_at;
		return [
			{
				runId: r.run_id,
				fingerprintRunId: null,
				jobKey: r.job_key ?? null,
				outcome,
				startedAt: started,
				durationMs: r.started_at && r.ended_at ? Date.parse(r.ended_at) - Date.parse(r.started_at) : null,
				rows: r.rows_affected ?? null,
				message: r.skipped_because_of ? `Skipped because ${shortName(r.skipped_because_of)} did not succeed` : (r.error_code ?? null)
			}
		];
	});
}

/**
 * The node's relation as last synced, with its sampled row counts. The sync records neither
 * declared types nor creation times, so columns are not compared and those times stay null.
 * Null until a sync has observed the relation.
 */
export function warehouseOf(node: CatalogNode, status: Wire['NodeStatus'] | null, statistics: Wire['WarehouseStatisticsRecord'][]): WarehouseObject | null {
	if (!node.relation || status?.physical.status !== 'observed') return null;
	const { observed_at, record } = status.physical.observation;
	const stats = statistics.flatMap((r) => (r.row_count === null || r.row_count === undefined ? [] : [{ at: r.observed_at, rows: r.row_count, bytes: r.byte_count ?? null }]));
	const newest = statistics.at(-1);
	return {
		relation: node.relation,
		objectType: record.relation_type.toLowerCase().replace(/^base table$/, 'table'),
		createdAt: null,
		lastAlteredAt: null,
		rows: newest?.row_count ?? null,
		bytes: newest?.byte_count ?? null,
		columns: record.columns.map((c) => ({ name: c.name, declaredType: null, actualType: c.data_type, state: null })),
		stats,
		syncedAt: observed_at
	};
}

/** Recorded coverage of a concurrent microbatch model, with its holes counted. */
export function toMicrobatchCoverage(wire: Wire['MicrobatchCoverage']): MicrobatchCoverage {
	return {
		cursorColumn: wire.cursor_column,
		cursorType: wire.cursor_type,
		batchSize: wire.batch_size,
		earliest: wire.earliest,
		contiguousFrontier: wire.contiguous_frontier ?? null,
		knownGaps: wire.known_gaps.length,
		batches: wire.batches.map((b) => ({ start: b.start, end: b.end, state: b.state, runId: b.run_id ?? null })),
		batchesTruncated: wire.batches_truncated
	};
}

/**
 * A listed node with its status, SQL, run history, jobs, checks and synced warehouse object. The
 * server keeps no cost or migrations per node, so those stay empty or null and the pages hide
 * them. `sql` is null when the person may not see SQL; `runs` and `jobs` are null when their
 * reads failed.
 */
export function nodeDetailOf(
	nodes: CatalogNode[],
	node: CatalogNode,
	status: Wire['NodeStatus'] | null,
	sql: Wire['NodeSql'] | null,
	checks: Check[],
	{ runs = null, jobs = null, statistics = [], microbatch = null }: { runs?: NodeRunEntry[] | null; jobs?: NodeJob[] | null; statistics?: Wire['WarehouseStatisticsRecord'][]; microbatch?: MicrobatchCoverage | null } = {}
): CatalogNodeDetail {
	// Checks name their node without its kind, so the attachment tells a source from a model of the same name.
	const own = checks.filter((c) => c.node === node.key || `${c.attachment}:${c.node}` === node.key);
	const physical = status?.physical.status === 'observed' ? status.physical.observation.record : null;
	return {
		...node,
		stale: staleOf(status),
		checks: checkCountsOf(own),
		downstream: nodes.filter((n) => n.upstream.includes(node.key)).map((n) => n.key),
		recentOutcomes: (runs ?? []).slice(0, 40).map((r) => r.outcome).reverse(),
		lastRows: statistics.at(-1)?.row_count ?? null,
		rowsDelta: null,
		lastDurationMs: runs?.find((r) => r.durationMs !== null)?.durationMs ?? null,
		cost: null,
		columns: (physical?.columns ?? []).map((c) => ({ name: c.name, type: c.data_type, description: '', audits: [] })),
		sql: sql?.raw_sql ? { raw: sql.raw_sql, compiled: sql.compiled_sql ?? sql.raw_sql } : null,
		code: null,
		builds: [],
		jobs,
		runs,
		checkResults: own,
		warehouse: warehouseOf(node, status, statistics),
		microbatch,
		history: null
	};
}

/** A kept name through `direct` or `star` is a passthrough, a new name a rename; every other transform changes the value. */
function lensOf(edge: Wire['ColumnLineageEdge']): ColumnLens {
	if (edge.transform !== 'direct' && edge.transform !== 'star') return 'transform';
	return edge.source_column === edge.target_column ? 'passthrough' : 'rename';
}

/** The compiler's transform kind as a label, such as `Cast`; passthroughs and renames keep the lens's own name. */
function transformLabel(edge: Wire['ColumnLineageEdge']): string | undefined {
	if (lensOf(edge) !== 'transform') return undefined;
	return edge.transform === 'unknown' ? 'Transformed' : edge.transform.charAt(0).toUpperCase() + edge.transform.slice(1);
}

/**
 * Column lineage around one node, named and coloured from the catalog listing. The read carries no
 * SQL or expressions, so those stay empty; null before a snapshot publishes lineage for the node.
 */
export function toColumnLineage(read: Wire['ColumnLineageRead'], nodes: CatalogNode[]): ColumnLineage | null {
	if (!read.edges.length) return null;
	const listed = new Map(nodes.map((n) => [n.key, n]));
	return {
		models: read.nodes.flatMap((n) => {
			const node = listed.get(n.node_key);
			const columns = n.columns.map((c) => ({ name: c.name, type: c.data_type ?? '' }));
			return node ? [{ id: n.node_key, name: shortName(n.node_key), kind: node.kind, status: cardStatus(node), columns }] : [];
		}),
		edges: read.edges.map((e) => [`${e.source_key}.${e.source_column}`, `${e.target_key}.${e.target_column}`, lensOf(e), transformLabel(e)]),
		sql: {},
		expressions: {}
	};
}
