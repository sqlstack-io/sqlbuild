import type { components } from '$control/schema.d.ts';
import { shortName } from '$lib/build-area/labels';
import { cardStatus } from '$lib/lineage/catalog-graph';
import { withBehind } from './node-states';
import type { CatalogNode, CatalogNodeDetail, Check, CheckCounts, ColumnLens, ColumnLineage, MicrobatchCoverage, NodeJob, NodeOutcome, NodeRunEntry, NodeState, NodeStates, NodeStatusDelta, WarehouseObject } from './types/build';
import type { NodeKind } from './types/operate';

type Wire = components['schemas'];

/** Snapshot kinds that are graph nodes under their own name; models take theirs from the materialization. */
const GRAPH_KINDS: ReadonlySet<string> = new Set<NodeKind>(['source', 'seed', 'asset', 'loader', 'udf', 'table_fn']);

/**
 * Every graph node kind is a catalog node: models, sources, seeds, Python nodes and functions.
 * Audits, SQL tests, scenarios, checks, macros and hooks are not.
 */
export function kindOf(wire: Pick<Wire['CatalogNodeRecord'], 'kind' | 'materialization'>): NodeKind | null {
	if (GRAPH_KINDS.has(wire.kind)) return wire.kind as NodeKind;
	if (wire.kind !== 'model') return null;
	if (wire.materialization === 'view') return 'view';
	return wire.materialization?.startsWith('incremental') ? 'incremental' : 'table';
}

/**
 * One node of the current snapshot. The server reports declared metadata, the newest fingerprint,
 * the newest decisive outcome, whether the definition changed since that fingerprint, and its
 * checks by their newest results. It reports no groups, so pages group by folder instead. The
 * fingerprint's run is a `sqb` run, not a scheduler run. Staleness from upstream builds needs the
 * whole listing, so `toCatalogNodes` adds it.
 */
export function toCatalogNode(wire: Wire['CatalogNodeRecord']): CatalogNode | null {
	const kind = kindOf(wire);
	if (!kind) return null;
	const state = toNodeState(wire);
	return {
		key: wire.node_key,
		kind,
		materialization: kind === 'source' || kind === 'seed' || kind === 'udf' || kind === 'table_fn' ? null : (wire.materialization ?? null),
		group: null,
		tags: wire.tags,
		description: wire.description ?? '',
		path: wire.path ?? '',
		upstream: wire.dependencies,
		relation: wire.relation ? { database: wire.relation.database ?? null, schema: wire.relation.schema, name: wire.relation.name } : null,
		columnCount: wire.column_count ?? 0,
		lastBuild: state.lastBuild,
		lastOutcome: state.lastOutcome,
		stale:
			state.definitionChanged && state.lastBuild
				? [{ reason: 'changed', change: 'definition', detail: 'The definition changed since it was last built here.', since: state.lastBuild.builtAt }]
				: [],
		sourceFreshness: null,
		checks: state.checks,
		// The listing carries no state; the node-states read sets it.
		status: null,
		statusAt: null
	};
}

/** A node's build state, from its listing record or the node-states map, which carry the same fields. */
export function toNodeState(wire: Wire['NodeState']): NodeState {
	const outcome = wire.last_outcome?.status;
	return {
		lastBuild: wire.last_build
			? { fingerprintRunId: wire.last_build.run_id, runId: null, jobKey: null, builtAt: wire.last_build.observed_at, fingerprint: wire.last_build.version_hash }
			: null,
		lastOutcome: outcome === 'success' || outcome === 'warn' || outcome === 'failed' ? outcome : null,
		definitionChanged: wire.definition_changed,
		// A recording from an older server, such as the static demo's, has no counts: unknown, not zero.
		checks: wire.checks ?? null,
		status: 'status' in wire ? (wire.status ?? null) : null,
		statusAt: 'status_at' in wire ? (wire.status_at ?? null) : null
	};
}

export function toNodeStates(wire: Wire['NodeStatesRecord']): NodeStates {
	const build = wire.changed_build;
	return {
		snapshotId: wire.snapshot_id ?? null,
		version: wire.version,
		states: Object.fromEntries(Object.entries(wire.states).map(([key, state]) => [key, toNodeState(state)])),
		changedBuild: build ? { generation: build.generation, changed: build.changed, downstream: build.downstream, failing: build.failing ?? null } : null
	};
}

/**
 * The changed states a `node_status` notice carries, or null for a notice without them, as an
 * older server sends. Malformed states are dropped rather than applied.
 */
export function toNodeStatusDelta(data: unknown): NodeStatusDelta | null {
	if (!data || typeof data !== 'object') return null;
	const { version, previous_version: previousVersion, states, full } = data as { version?: unknown; previous_version?: unknown; states?: unknown; full?: unknown };
	if (typeof version !== 'number' || typeof previousVersion !== 'number') return null;
	if (full === true) return { version, previousVersion, full: true };
	if (!states || typeof states !== 'object') return null;
	return { version, previousVersion, states: Object.fromEntries(Object.entries(states as Record<string, Wire['NodeState']>).map(([key, state]) => [key, toNodeState(state)])) };
}

/** Catalog nodes in listing order, keeping only dependencies that are themselves listed. */
export function toCatalogNodes(wire: Wire['CatalogNodeRecord'][]): CatalogNode[] {
	const nodes = wire.flatMap((node) => toCatalogNode(node) ?? []);
	const keys = new Set(nodes.map((node) => node.key));
	const linked = nodes.map((node) => ({ ...node, upstream: node.upstream.filter((key) => keys.has(key)) }));
	return withBehind(linked);
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
		stale: node.stale ?? [],
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
