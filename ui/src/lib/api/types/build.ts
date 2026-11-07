// Wire types for the BUILD area: the catalog and lineage, checks, the change
// feed, and the overview that summarises them.
//
// Definitions (graph, columns, SQL, column lineage) come from the current
// snapshot. Build state comes from synced warehouse state (fingerprints, audit
// results, source freshness, microbatches, migrations, information schema)
// joined to the scheduler's run nodes, so builds made outside the scheduler
// (CLI, CI) show too.

import type { Iso, PlatformEvent, Principal } from '../types';
import type { NodeKind, QueueWait, Run, RunStatus } from './operate';
import type { MigrationEvent, OldNameView, RelationName } from './platform';

// ---- node build state ----

/** The outcome of a node's build attempt. Separate from whether it is stale. */
export type NodeOutcome = 'success' | 'warn' | 'failed' | 'skipped';

/**
 * Why a node is stale. Derived from the snapshot and synced state, never from
 * a plan against the warehouse.
 */
export type StaleReason =
	/** The node's own definition differs from what was last built in this target. */
	| { reason: 'changed'; change: 'definition' | 'compiled'; detail: string; since: Iso }
	/** An upstream definition changed since this node's last build, so the next build would differ. */
	| { reason: 'changed_upstream'; upstream: string[]; since: Iso }
	/** Sources or upstream models have newer data than this node's last build. */
	| { reason: 'behind'; upstream: string[]; detail: string; since: Iso };

/** A build recorded in `_sqlbuild_fingerprints`. */
export type NodeBuild = {
	/** The `sqb` run that wrote the fingerprint. */
	fingerprintRunId: string;
	/** The scheduler run, when the build was one; null for builds outside the scheduler. */
	runId: string | null;
	jobKey: string | null;
	builtAt: Iso;
	fingerprint: string;
};

/** The latest `_sqlbuild_source_freshness` result for a source. */
export type SourceFreshness = {
	status: 'pass' | 'warn' | 'error';
	/** Newest loaded-at value observed in the source. */
	loadedAt: Iso | null;
	checkedAt: Iso;
	warnAfterSeconds: number | null;
	errorAfterSeconds: number | null;
};

export type CheckCounts = { total: number; passing: number; warning: number; failing: number };

export type CatalogNode = {
	key: string;
	kind: NodeKind;
	/** e.g. `table`, `view`, `incremental · merge`, `incremental · microbatch`; null for sources and tasks. */
	materialization: string | null;
	group: string;
	tags: string[];
	/** Declared `owner`, or the project default. */
	owner: string[];
	description: string;
	path: string;
	upstream: string[];
	/** The warehouse object this node builds or reads. */
	relation: RelationName | null;
	columnCount: number;
	lastBuild: NodeBuild | null;
	/** The latest build attempt's outcome, from run nodes and audit results. */
	lastOutcome: NodeOutcome | null;
	stale: StaleReason[];
	/** Sources only. */
	sourceFreshness: SourceFreshness | null;
	checks: CheckCounts;
};

// ---- node detail ----

export type ColumnDef = {
	name: string;
	type: string | null;
	description: string;
	/** Audits attached to the column, by definition name. */
	audits: string[];
};

/** Raw and compiled SQL of a build, as stored with its fingerprint. */
export type BuiltDefinition = { build: NodeBuild; raw: string; compiled: string };

/** One build attempt of the node: a scheduler run node or a build outside the scheduler. */
export type NodeRunEntry = {
	runId: string | null;
	fingerprintRunId: string | null;
	jobKey: string | null;
	outcome: NodeOutcome;
	startedAt: Iso;
	durationMs: number | null;
	rows: number | null;
	message: string | null;
};

export type CostUnit = 'credits';

/** Cost rollup for the node; null where the adapter reports no cost. */
export type NodeCost = { last24h: number; perBuild: number; unit: CostUnit };

export type WarehouseColumnState = 'match' | 'type_differs' | 'missing' | 'undeclared';

/** The synced information-schema object and its sampled statistics. */
export type WarehouseObject = {
	relation: RelationName;
	objectType: 'table' | 'view';
	createdAt: Iso;
	lastAlteredAt: Iso;
	rows: number | null;
	bytes: number | null;
	columns: { name: string; declaredType: string | null; actualType: string | null; state: WarehouseColumnState }[];
	/** Sampled series, oldest first. */
	stats: { at: Iso; rows: number; bytes: number }[];
	syncedAt: Iso;
};

export type MicrobatchState = 'complete' | 'gap' | 'replay_required';

/**
 * Projection of `_sqlbuild_microbatches` for a model's current relation. Coverage is
 * tracked only for concurrent microbatch models; other models have none.
 */
export type MicrobatchCoverage = {
	cursorColumn: string;
	/** Bounds are timestamp text for `timestamp` cursors and integer text for `integer` cursors. */
	cursorType: 'timestamp' | 'integer';
	batchSize: string;
	/** Start of the earliest completed partition. */
	earliest: string;
	/** End of the completed range that starts at `earliest` without a hole. */
	contiguousFrontier: string | null;
	/** Holes between completed partitions. */
	knownGaps: number;
	/** The newest batches, oldest first. */
	batches: { start: string; end: string; state: MicrobatchState; runId: string | null }[];
	/** Older batches were left out of `batches`. */
	batchesTruncated: boolean;
};

/** Renames and moves recorded in `_sqlbuild_migrations`, with any old-name views. */
export type NodeHistory = {
	formerNames: string[];
	migrations: MigrationEvent[];
	oldNameViews: OldNameView[];
};

export type NodeJob = { key: string; kind: 'job' | 'group'; triggerIds: string[] };

export type CatalogNodeDetail = CatalogNode & {
	downstream: string[];
	/** Outcomes of the most recent builds, oldest first, up to 40 (`recent_outcomes`). */
	recentOutcomes: NodeOutcome[];
	lastRows: number | null;
	rowsDelta: number | null;
	lastDurationMs: number | null;
	cost: NodeCost | null;
	columns: ColumnDef[];
	/** Current snapshot SQL; null for sources and Python nodes. */
	sql: { raw: string; compiled: string } | null;
	/** Python node source, stored by content hash for display. */
	code: { file: string; source: string } | null;
	/** As built in this target, newest first. */
	builds: BuiltDefinition[];
	jobs: NodeJob[];
	runs: NodeRunEntry[];
	checkResults: Check[];
	warehouse: WarehouseObject | null;
	microbatch: MicrobatchCoverage | null;
	history: NodeHistory;
};

// ---- column lineage ----

export type ColumnLens = 'raw' | 'passthrough' | 'rename' | 'transform';

export type ColumnLineage = {
	models: { id: string; name: string; kind: string; status: 'ok' | 'fail' | 'warn' | 'stale' | 'src'; columns: { name: string; type: string }[] }[];
	/** `model.column` pairs from upstream to downstream. */
	edges: [string, string, ColumnLens][];
	sql: Record<string, string>;
	/** The defining expression of each column, per model. */
	expressions: Record<string, Record<string, string>>;
};

// ---- checks (`_sqlbuild_audit_results`) ----

export type CheckOutcome = 'pass' | 'warn' | 'fail' | 'error' | 'insufficient' | 'skipped';
export type CheckSeverity = 'warn' | 'error';
export type RunScopePhase = 'delta_and_final' | 'final';
export type EvaluationMode = 'violations' | 'measurement';
export type CheckKind = 'audit' | 'python_check' | 'sql_test';
/** What the check is attached to; `end` checks run at the end of a run. */
export type CheckAttachment = 'model' | 'source' | 'seed' | 'end';

export type CheckThreshold = {
	level: 'warn' | 'error';
	operator: 'below' | 'above' | 'outside';
	value: number | [number, number];
};

export type CheckEvaluation = {
	/** Audit result event ID. */
	id: string;
	runId: string | null;
	fingerprintRunId: string;
	evaluatedAt: Iso;
	outcome: CheckOutcome;
	violationCount: number | null;
	measuredValue: number | null;
	evidenceCount: number | null;
	evidenceTruncated: boolean;
};

/** One audit or check with its latest result. */
export type Check = {
	id: string;
	name: string;
	/** The audit definition, e.g. `not_null`; null for custom checks. */
	definition: string | null;
	/** Null where the server does not report it. */
	kind: CheckKind | null;
	attachment: CheckAttachment;
	node: string | null;
	column: string | null;
	severity: CheckSeverity;
	runScopePhase: RunScopePhase;
	evaluationMode: EvaluationMode;
	thresholds: CheckThreshold[];
	description: string | null;
	latest: CheckEvaluation | null;
	/** Outcomes of recent evaluations, oldest first; null where the listing does not report them. */
	recentOutcomes: CheckOutcome[] | null;
};

export type CheckDetail = Check & {
	history: CheckEvaluation[];
};

/** The latest evaluation's SQL and error texts, which quote SQL. Reading them requires `view.sql` and is audited. */
export type CheckSql = {
	evaluationId: string;
	executedSql: string | null;
	measurementSql: string | null;
	evidenceSql: string | null;
	executionError: string | null;
	evidenceError: string | null;
};

/** Sample rows stored with a result. Reading them requires `view.evidence` and is audited. */
export type CheckEvidence = {
	evaluationId: string;
	/** Every row key, in sorted order; a row without a key holds null there. */
	columns: string[];
	/** JSON cells: scalars, or objects and arrays from structured warehouse columns. */
	rows: unknown[][];
	truncated: boolean;
};

// ---- change feed (snapshot diffs per deploy) ----

export type ChangeOp = 'added' | 'removed' | 'changed' | 'renamed';

export type NodeChange = {
	key: string;
	kind: NodeKind | null;
	op: ChangeOp;
	/** Renames: the previous key. */
	formerKey: string | null;
	/** Only the compiled SQL changed, e.g. through a macro or variable. */
	compiledOnly: boolean;
	detail: string;
	columns: { name: string; op: ChangeOp; detail: string }[];
};

export type OrchestrationChange = {
	kind: 'job' | 'group' | 'trigger' | 'sensor';
	key: string;
	/** Triggers and sensors: the trigger ID, while it still exists. */
	triggerId: string | null;
	op: ChangeOp;
	detail: string;
};

export type RoleChange = {
	role: string;
	op: ChangeOp;
	detail: string;
	/** Nodes the role's grants cover before and after. */
	coverage: { before: number; after: number } | null;
};

/** The environment fingerprint (lock file, Python and `sqb` versions) changed. */
export type EnvironmentChange = { before: string; after: string; detail: string };

/** Average cost per run of a job before and after the deploy, from cost rollups. */
export type CostChange = { jobKey: string; before: number; after: number; unit: CostUnit; runsAfter: number };

export type SnapshotChange = {
	snapshotId: string;
	generation: number;
	previousGeneration: number | null;
	gitRevision: string;
	gitDirty: boolean;
	gitAuthor: string;
	publishedAt: Iso;
	publishedBy: Principal;
	nodes: NodeChange[];
	orchestration: OrchestrationChange[];
	roles: RoleChange[];
	environment: EnvironmentChange | null;
	/** The first runs after the deploy, oldest first. */
	followingRuns: Pick<Run, 'id' | 'jobKey' | 'kind' | 'status' | 'startedAt' | 'queuedAt'>[];
	/** Null when the adapter has no cost data. */
	costChanges: CostChange[] | null;
};

// ---- overview ----

export type AttentionRun = Pick<Run, 'id' | 'jobKey' | 'kind' | 'status' | 'statusReason' | 'startedAt' | 'endedAt'>;

export type AttentionItem =
	| { kind: 'run_problem'; run: AttentionRun }
	| { kind: 'run_unresponsive'; run: AttentionRun; lastHeartbeatAt: Iso | null }
	| { kind: 'stale_models'; changed: string[]; behind: string[] }
	| { kind: 'stale_sources'; sources: ({ key: string } & SourceFreshness)[] }
	| { kind: 'platform_event'; event: PlatformEvent }
	| { kind: 'queue_blocked'; runId: string; jobKey: string | null; queuedAt: Iso; waits: QueueWait[] }
	| { kind: 'calendar_low'; triggerId: string; triggerName: string; detail: string };

export type OverviewMetrics = {
	models: { total: number; addedThisWeek: number; removedThisWeek: number };
	runs24h: { total: number; success: number; warn: number; partial: number; failed: number; active: number };
	checks: CheckCounts;
	/** Distinct stale nodes, and how many are changed or behind (a node can be both). */
	stale: { nodes: number; changed: number; behind: number; sources: number };
	/** Null when the adapter has no cost data. */
	cost: { last24h: number; baseline: number; baselineDays: number; unit: CostUnit } | null;
};

/** Build status per group: current, stale (changed or behind) and failed. */
export type GroupStatus = { group: string; total: number; current: number; changed: number; behind: number; failed: number };

export type Overview = {
	metrics: OverviewMetrics;
	attention: AttentionItem[];
	groups: GroupStatus[];
	recentChanges: SnapshotChange[];
	latestRun: (Pick<Run, 'id' | 'endedAt'> & { status: RunStatus }) | null;
};
