// Wire types for the OPERATE area: runs, the queue, the timeline, the run
// panel's server check, jobs and job groups, and triggers and sensors.
//
// Execution detail (events, statements, output) mirrors Recall; the scheduler's
// own run facts share the same event stream with `producer = scheduler`.

import type { Capability, Iso, Principal } from '../types';

// ---- runs ----

export const RUN_STATUSES = [
	'queued',
	'launched',
	'running',
	'unresponsive',
	'success',
	'warn',
	'partial',
	'failed',
	'cancelled',
	'skipped',
	'presumed_failed'
] as const;
export type RunStatus = (typeof RUN_STATUSES)[number];

/** What runs: a job, a job group (parent of one child run per member) or an ad-hoc selection. */
export const RUN_KINDS = ['job', 'group', 'adhoc'] as const;
export type RunKind = (typeof RUN_KINDS)[number];

/** Why the run was requested. Group children inherit their parent's origin. */
export const RUN_ORIGINS = ['manual', 'schedule', 'sensor', 'rerun', 'backfill'] as const;
export type RunOrigin = (typeof RUN_ORIGINS)[number];

export type NodeCounts = {
	success: number;
	warn: number;
	failed: number;
	skipped: number;
	running: number;
	pending: number;
};

/** Compute estimate for a finished run; null where the adapter reports no cost. */
export type RunCost = {
	value: number;
	unit: 'credits';
	/** Where the figure comes from, e.g. busy-compute estimates per statement. */
	source: string;
};

export type Run = {
	id: string;
	kind: RunKind;
	origin: RunOrigin;
	status: RunStatus;
	statusReason: string | null;
	/** Job or group key; null for ad-hoc runs. */
	jobKey: string | null;
	/** Set on the child runs of a group run. */
	parentRunId: string | null;
	rootRunId: string;
	childRunCount: number;
	triggerId: string | null;
	triggerName: string | null;
	tickId: string | null;
	/** The scheduled time of the tick that requested this run. */
	tickScheduledFor: Iso | null;
	/** Sensor run requests are de-duplicated by run key. */
	runKey: string | null;
	rerunOfRunId: string | null;
	/** A group member's attempt once its group retried it, from 2; null for a first attempt and every other run. */
	memberAttempt: number | null;
	/** How a person narrowed this run of a job, as typed; null when it runs the whole job. */
	narrowing: { select: string[]; exclude: string[] } | null;
	/** The cursor window a backfill processes, inclusive; null for every other run. */
	window: { start: string; end: string } | null;
	requestedBy: Principal;
	priority: number;
	/** Full refresh or an explicit cursor window was requested. */
	rewriteHistory: boolean;
	locationId: string;
	target: string;
	snapshotId: string;
	/** Null where the server does not report the commit. */
	gitRevision: string | null;
	queuedAt: Iso;
	launchedAt: Iso | null;
	startedAt: Iso | null;
	endedAt: Iso | null;
	nodeCounts: NodeCounts;
	cost: RunCost | null;
	/** Error-severity audit failures the run let through instead of stopping (`ignore_audit_failures`); a group sums its members'. */
	ignoredAuditFailures: number;
};

export type RunListQuery = {
	jobKey?: string;
	statuses?: RunStatus[];
	kinds?: RunKind[];
	origins?: RunOrigin[];
	/** Children of this group run. */
	parentRunId?: string;
	/** Hide child runs; group parents carry `childRunCount`. */
	topLevelOnly?: boolean;
	triggerId?: string;
	since?: Iso;
	until?: Iso;
	limit?: number;
};

// ---- runs list (cursor-paged rows with a plain status line) ----

/** Status tabs of the runs list. `active` is queued, launched, running or unresponsive; `failed` includes partial and lost runs. */
export const RUN_TABS = ['all', 'active', 'queued', 'failed', 'cancelled'] as const;
export type RunTab = (typeof RUN_TABS)[number];

/** Structured filter parsed from the list's search tokens; every field narrows (AND), values within a field are OR. */
export type RunFilter = {
	kinds?: RunKind[];
	origins?: RunOrigin[];
	statuses?: RunStatus[];
	jobKeys?: string[];
	/** Schedule names (trigger type `schedule`). */
	schedules?: string[];
	/** Sensor names (trigger type `sensor`). */
	sensors?: string[];
	/** Runs that execute any of these nodes. */
	nodeKeys?: string[];
	/** Run ID prefixes, with or without the `run-` prefix. */
	ids?: string[];
	/** Principal names that requested the run. */
	requestedBy?: string[];
	/** Free text matched against IDs, jobs, triggers, run keys and principals. */
	text?: string;
};

export type RunRowQuery = {
	tab?: RunTab;
	filter?: RunFilter;
	/** Queued at or after. */
	since?: Iso;
	/** Queued before. */
	until?: Iso;
	/** Include group children as their own rows; otherwise only top-level runs. */
	includeChildren?: boolean;
	/** Only the children of this group run (inline expansion). */
	parentRunId?: string;
	/** Opaque cursor from the previous page. */
	cursor?: string | null;
	limit?: number;
};

export type RunRow = {
	run: Run;
	/** Nodes finished (any outcome) out of the run's planned nodes, across children for groups. */
	progress: { done: number; total: number };
	/** Why a queued run waits, as on the Queue view. */
	waits: QueueWait[];
	/** Last heartbeat, set while the run is unresponsive or once it was marked failed for silence. */
	lastContactAt: Iso | null;
	/** Heartbeat silence after which the scheduler marks a run failed. */
	lostContactLimitSeconds: number;
	/** Cancel requested and not yet finished. */
	cancelRequestedAt: Iso | null;
	/** Ad-hoc runs: the selection as typed. */
	selection: string[];
	/** Nodes a rerun from failure would run; empty when nothing failed. */
	rerunFromFailureCount: number;
};

export type RunRowPage = {
	rows: RunRow[];
	/** Pass back as `cursor` for the next page; null on the last page. */
	nextCursor: string | null;
};

/** Counts for the in-flight tabs, under the same filter as the rows. */
export type RunTabCounts = { active: number; queued: number };

export type BulkRunResult = {
	/** Runs the request applied to. */
	done: string[];
	failed: { runId: string; error: string }[];
};

// ---- run detail ----

export type RunNodeStatus =
	| 'pending'
	| 'running'
	| 'success'
	| 'warn'
	| 'failed'
	| 'skipped'
	| 'cancelled'
	/** The run ended (lost worker, never started) before this node ran. */
	| 'not_run';

/** Graph node kinds: SQL models by materialization, sources and seeds, Python nodes (asset, loader, task) and functions (udf, table_fn). */
export type NodeKind = 'source' | 'loader' | 'seed' | 'view' | 'table' | 'incremental' | 'task' | 'asset' | 'udf' | 'table_fn';

export type SkippedBecauseOf = {
	nodeKey: string;
	/** The run the failed node belonged to; differs from this node's run across jobs. */
	runId: string;
	jobKey: string | null;
};

export type RunNode = {
	nodeKey: string;
	/** Null where the server does not report the node kind. */
	kind: NodeKind | null;
	/** The (child) run that executed this node. */
	runId: string;
	jobKey: string | null;
	status: RunNodeStatus;
	attempt: number;
	startedAt: Iso | null;
	endedAt: Iso | null;
	rowsAffected: number | null;
	errorCode: string | null;
	errorMessage: string | null;
	/** Where in the project the failure points: the model file, and the line when it is known. */
	errorLocation: { file: string; line: number | null } | null;
	/** Full error text (warehouse message, statement, trace); `errorMessage` is its first line. */
	errorDetail: string | null;
	skippedBecauseOf: SkippedBecauseOf | null;
	/** Node failure stops the whole group (`critical = true`). */
	critical: boolean;
};

/** A failed node's error, read on request because it can quote warehouse data. `view.evidence` on the node */
export type RunNodeError = {
	nodeKey: string;
	errorCode: string | null;
	/** Redacted and cut to 2,000 characters by the server; null when none was recorded. */
	message: string | null;
	/** The node's file, relative to the project root. */
	file: string | null;
};

export type RunEdge = { from: string; to: string };

export const RECALL_EVENT_TYPES = [
	'invocation_started',
	'invocation_completed',
	'invocation_failed',
	'resource_attempt_started',
	'resource_attempt_completed',
	'resource_attempt_failed',
	'resource_attempt_skipped',
	'statement_submitted',
	'statement_started',
	'statement_completed',
	'statement_failed',
	'audit_completed'
] as const;
export type RecallEventType = (typeof RECALL_EVENT_TYPES)[number];

export const SCHEDULER_FACTS = [
	'requested',
	'queued',
	'claimed',
	'launched',
	'cancel_requested',
	'presumed_failed',
	'terminated',
	'member_retrying',
	'member_recovered'
] as const;
export type SchedulerFact = (typeof SCHEDULER_FACTS)[number];

export type RunEventLevel = 'debug' | 'info' | 'warn' | 'error';

export type RunEvent = {
	id: string;
	/** Storage order; equal to commit order, used for paging. */
	storageId: number;
	runId: string;
	producer: 'sqb' | 'scheduler';
	type: RecallEventType | SchedulerFact;
	occurredAt: Iso;
	level: RunEventLevel;
	invocationId: string | null;
	nodeKey: string | null;
	statementId: string | null;
	attempt: number | null;
	message: string;
};

export type RunEventPage = {
	events: RunEvent[];
	/** Pass back as `after` for the next page; null when there are no more events yet. */
	nextCursor: number | null;
};

/** How complete a run's estimated query cost is; `none` until any cost was collected and synced. */
export type RunCostStatus = 'complete' | 'partial' | 'pending' | 'collection_failed' | 'unavailable' | 'none';

/** One latest cost observation: a collection's totals (`resourceType` "_total") or one resource on one warehouse. */
export type RunCostRow = {
	/** The build's own run ID, which groups the rows of one collection. */
	buildRunId: string;
	resourceType: string;
	resourceName: string;
	/** The resource's catalog node key; null on run rows. */
	nodeKey: string | null;
	warehouseName: string;
	warehouseSize: string;
	queryCount: number;
	seconds: number;
	bytesScanned: number;
	credits: number;
	/** Null without a configured credit price; the UI never guesses one. */
	usd: number | null;
	status: Exclude<RunCostStatus, 'none'>;
	statementsExpected: number;
	statementsObserved: number;
};

export type RunCostResource = { nodeKey: string; credits: number; usd: number | null; seconds: number; queryCount: number; bytesScanned: number; warehouses: string[] };
export type RunCostWarehouse = { name: string; size: string; credits: number; usd: number | null; seconds: number; queryCount: number };

/** A run's estimated query cost, with its resources and warehouses most expensive first. */
export type RunCostBreakdown = {
	status: RunCostStatus;
	credits: number;
	usd: number | null;
	seconds: number;
	queryCount: number;
	statementsExpected: number;
	statementsObserved: number;
	resources: RunCostResource[];
	warehouses: RunCostWarehouse[];
};

/** One group of a cost summary: a job, a warehouse size or a node; `key` is null for builds no server run started. */
export type CostGroup = { key: string | null; credits: number; usd: number | null; seconds: number; runs: number };

/** A target's estimated query cost over recent days, each breakdown most expensive first. */
export type CostSummary = {
	since: string;
	credits: number;
	/** Null when any run in the window had no configured credit price. */
	usd: number | null;
	runs: number;
	/** Runs whose cost is still pending or partial, which a later build re-reads. */
	incompleteRuns: number;
	byJob: CostGroup[];
	byWarehouseSize: CostGroup[];
	/** Every node that ran, keyed by node key. */
	byNode: CostGroup[];
};

/** One node's estimated query cost in one build, across every warehouse it used. */
export type NodeBuildCost = {
	buildRunId: string;
	/** The server run that started the build; null for builds from the command line. */
	runId: string | null;
	startedAt: string;
	/** The collection status of the whole build. */
	status: Exclude<RunCostStatus, 'none'>;
	credits: number;
	usd: number | null;
	seconds: number;
	queryCount: number;
};

export type RunStatement = {
	id: string;
	runId: string;
	invocationId: string;
	/** 1-based order within the run. */
	sequence: number;
	nodeKey: string;
	kind: string;
	status: 'running' | 'completed' | 'failed' | 'cancelled';
	startedAt: Iso;
	endedAt: Iso | null;
	rowsAffected: number | null;
	/** Rows processed so far while running, when the warehouse reports progress. */
	rowsSoFar: number | null;
	/** The warehouse's own ID for the query, for its query history; null when the adapter has none. */
	queryId: string | null;
	/** False when SQL capture was off or the text was dropped by retention. */
	sqlRecorded: boolean;
	error: string | null;
};

/** One error-severity audit failure a run, or one of its group's member runs, let through (`ignore_audit_failures`). */
export type IgnoredAuditFailure = {
	/** The run that evaluated the audit: the run itself or one of its members. */
	runId: string;
	jobKey: string | null;
	at: Iso;
	/** The check's ID on the Checks page; null when the result named none. */
	checkId: string | null;
	audit: string;
	node: string | null;
	column: string | null;
	severity: string;
	violationCount: number | null;
};

/** One statement's SQL, loaded on demand. Null when it was not recorded. */
export type StatementSql = { statementId: string; sql: string | null };

export type RunOutputLine = {
	invocationId: string;
	stream: 'stdout' | 'stderr';
	at: Iso;
	text: string;
	/** The error the invocation recorded when it failed, rather than a line it printed. */
	error: boolean;
};

export type RunInvocation = {
	invocationId: string;
	runId: string;
	attempt: number;
	command: string;
	startedAt: Iso;
	endedAt: Iso | null;
	exitCode: number | null;
};

export type LivenessState = 'healthy' | 'unresponsive' | 'presumed_failed' | 'never_started' | 'exited';

export type RunLiveness = {
	state: LivenessState;
	workerInstance: string | null;
	containerId: string | null;
	lastHeartbeatAt: Iso | null;
	/** Heartbeat silence after which a run is unresponsive, and then presumed failed. */
	unresponsiveAfterSeconds: number;
	presumedFailedAfterSeconds: number;
	/** No start report within this long after launch means the run never started. */
	startDeadlineSeconds: number;
	exitCode: number | null;
};

/** cancel_requested → SIGTERM (statements cancelled, exit 143) → SIGKILL after the grace period. */
export type CancelProgress = {
	requestedAt: Iso;
	requestedBy: Principal;
	sigtermAt: Iso | null;
	graceSeconds: number;
	sigkillAt: Iso | null;
	exitCode: number | null;
	terminatedAt: Iso | null;
};

export type RunProvenance = {
	snapshotId: string;
	snapshotGeneration: number;
	/** Null when the project was published outside a git checkout. */
	gitRevision: string | null;
	gitDirty: boolean;
	/** Null when the location publishes no deployment image. */
	imageId: string | null;
	sqbVersion: string;
	/** The frozen selection the run was authorized for. */
	selection: string[];
	exclude: string[];
	/** Null where the server does not report the command the worker ran. */
	command: string | null;
};

export type RunDetail = {
	run: Run;
	parent: Run | null;
	children: Run[];
	nodes: RunNode[];
	edges: RunEdge[];
	invocations: RunInvocation[];
	liveness: RunLiveness | null;
	cancel: CancelProgress | null;
	/** Null where the server does not report how the run was built. */
	provenance: RunProvenance | null;
	/** The frozen selection the run was authorized for. */
	selection: string[];
	/** Why the run is still queued; empty once claimed. */
	waits: QueueWait[];
	/** Nodes a rerun from failure would run: the failed ones plus exactly those skipped. */
	rerunFromFailureNodes: string[];
	/** The run this one reruns. */
	rerunOf: Run | null;
	/** Later runs that rerun this one, newest first. */
	reruns: Run[];
};

// ---- queue ----

/** Why the scheduler can't claim a queued run yet; a run in another location or target is never named (`runId` null). */
export type QueueWait =
	| { kind: 'write_claim'; runId: string | null; nodeKey: string; otherNodes: number }
	/** Running runs of the same location and target hold the slots of a pool this run takes. */
	| { kind: 'pool_full'; pool: string; holders: string[] }
	/** An earlier queued run takes the same pool, so it goes first. */
	| { kind: 'earlier_run'; runId: string | null; pool: string }
	| { kind: 'job_running'; runId: string }
	/** `attempt` is set while the group retries that job: its attempt number, from 2. */
	| { kind: 'upstream_job'; runId: string; jobKey: string; attempt: number | null }
	/** A child run waits for its group run to start. */
	| { kind: 'group_parent'; runId: string }
	/** A retry of a failed run waits out the job's `[retry]` delay. */
	| { kind: 'retry_delay'; attempt: number; dueAt: Iso };

export type QueueEntry = {
	run: Run;
	position: number;
	waits: QueueWait[];
};

// ---- timeline ----

export type TimelineBar = {
	/** Null for projected (future) ticks. */
	runId: string | null;
	jobKey: string;
	kind: RunKind;
	origin: RunOrigin;
	status: RunStatus | 'projected';
	/** The schedule or calendar a projected tick belongs to. */
	triggerName: string | null;
	start: Iso;
	/** For running runs, the time the window was read; for projected ticks, start plus typical duration. */
	end: Iso;
};

export type TimelineTickMarker = {
	tickId: string;
	triggerId: string;
	triggerName: string;
	jobKey: string;
	scheduledFor: Iso;
	outcome: 'covered' | 'skipped';
	coveredByRunId: string | null;
	reason: string | null;
};

export type Timeline = {
	now: Iso;
	jobKeys: string[];
	bars: TimelineBar[];
	markers: TimelineTickMarker[];
};

// ---- snapshot nodes (selector autocomplete) ----

export type SnapshotNode = {
	key: string;
	kind: NodeKind;
	group: string;
	tags: string[];
	path: string;
	/** Has a cursor: incremental models and loaders accept cursor windows. */
	cursor: 'timestamp' | 'integer' | null;
	upstream: string[];
};

// ---- selections (the selection bar) ----

/** A selection as typed: each text holds space-separated selectors, as `sqb build -s … --exclude …` takes them. */
export type SelectionRequest = {
	select: string;
	exclude: string;
	/** Sorted keys to skip. */
	offset?: number;
	/** Page size; the server defaults to 1,000 and allows up to 10,000. */
	limit?: number;
};

/** The token a refused selection comes from: its field and its range there, in JavaScript string indexes. */
export type SelectionLocation = { field: 'select' | 'exclude'; token: string; start: number; end: number };

/** Why `sqb` refuses a selection; `code` is null when the selection as a whole is refused. */
export type SelectionProblem = {
	code: string | null;
	message: string;
	/** `sqb`'s own help line, such as "did you mean 'orders'?". */
	help: string | null;
	location: SelectionLocation | null;
	/** Whole-token replacements, closest first. */
	suggestions: string[];
	/**
	 * The comma part of the located token `sqb` refuses: its index among the token's parts, its selector
	 * type (`null` for a bare name or name pattern) and its value without type prefix or `+` markers.
	 * Null when the selection is refused as a whole or the part is a path between two nodes.
	 */
	refused: { part: number; type: string | null; value: string } | null;
};

/** What `sqb build --select … --exclude …` selects in the current publication: one page of keys and the total. */
export type SelectionEvaluation = {
	snapshotId: string | null;
	count: number;
	keys: string[];
	nextOffset: number | null;
	error: SelectionProblem | null;
};

/** One selector value and how many nodes it selects. */
export type VocabularyEntry = { value: string; count: number };

/** The values each selector type takes in the current publication, for autocomplete. */
export type SelectionVocabulary = {
	snapshotId: string | null;
	tags: VocabularyEntry[];
	/** Folders such as `models/staging`, counting the nodes below each at any depth. */
	paths: VocabularyEntry[];
	jobs: VocabularyEntry[];
	sources: VocabularyEntry[];
	seeds: VocabularyEntry[];
	kinds: VocabularyEntry[];
};

// ---- ad-hoc runs (the plan page) ----

export type LaunchVerb = 'build' | 'test' | 'seed' | 'audit' | 'freshness' | 'load';

/** A command as argument tokens after `sqb`, never shell text; the server resolves its selectors. */
export type AdhocRunRequest = {
	command: string[];
	/** Above 0 the run jumps the queue and needs `run.prioritise`. */
	priority: number;
};

/** One permission the run needs and whether the person holds it on every resolved node. */
export type AdhocPermission = { action: Capability; allowed: boolean; reason: string | null };

/** The server's dry run: what launching would run and whether this person may launch it. */
export type AdhocRunCheck = {
	verb: LaunchVerb | null;
	/** The exact nodes the worker would run. */
	selection: string[];
	/** Nodes the run would write and reserve; empty for read-only verbs. */
	writeSet: string[];
	/** Accepted flags as the worker receives them, by setting name. */
	settings: Record<string, unknown>;
	permissions: AdhocPermission[];
	/** Why the command cannot run regardless of permissions, such as a refused flag or a selection that matches nothing. */
	error: string | null;
	launchable: boolean;
};

/** One model in `sqb plan --json`: what the build would do to it and why. */
export type PlanModel = {
	name: string;
	materialization_type: string;
	action: string;
	reason: string;
	incremental_strategy?: string;
	cursor_column?: string;
	cursor_bounds?: { start: string | null; end: string | null } | null;
	backfill: { action: string; duration: string | null };
	qualified_name?: string;
	changed_functions?: string[];
	/** The rebuilt upstream that makes this kept dynamic model recompute in full. */
	reinitialised_by?: string;
	/** Whether the warehouse lists this kept dynamic model as suspended. */
	paused?: boolean;
	cursor_type?: string;
	cursor?: { requested_bounds: { start: string | null; end: string | null }; resolved_bounds: { start: string | null; end: string | null } | null } | null;
	/** Unified diff lines of the query since the model's last build, as `sqb plan` prints them. */
	query_diff?: string[];
	/** Unified diff lines of the model's config since its last build. */
	config_diff?: string[];
	/** A config change of at most two plain keys in words, such as `full_refresh true → false`. */
	config_change?: string;
	schema_findings?: { kind: string; column: string; expected_type: string | null; actual_type: string | null }[];
	/** The query changed as well as the name, on a renamed model. */
	query_changed?: boolean;
	/** The model a renamed model was renamed from. */
	origin_model?: string;
	/** What stays at a renamed model's old name. */
	old_name?: PlanOldName | null;
	/** A paged row's diff counts in place of its diff text, which `planDiff` reads on demand. */
	diff_stats?: { query?: PlanDiffStats; config?: PlanDiffStats };
};

/** A diff's changed lines; `whitespace_only` when they read the same with all whitespace taken out (a case change is a change). */
export type PlanDiffStats = { added: number; removed: number; whitespace_only: boolean };

/** One model's diff text, read on demand. */
export type PlanDiff = { model: string; query_diff?: string[]; config_diff?: string[] };

export type PlanWarning = { severity: string; message: string; model_name?: string; code?: string };

/** One warning template and how many warnings fill it in; `subject` counts the named things it opens with, such as scenarios, when they repeat. */
export type PlanWarningGroup = { template: string; count: number; subject: { noun: string; count: number } | null };

/** What a build does at a moved model's old name: keep a view there (`view`) reading the new one, until `expires_at`. */
export type PlanOldName = { action: string; view?: string | null; reads?: string | null; expires_at?: string | null };

/**
 * A model migration: `model` takes its data from `origin_model` (a rename, or a move into a model that
 * exists). `decision` says what this run does; `transfer` how a pending rename moves the data.
 */
export type PlanMigration = {
	model: string;
	origin_model?: string;
	decision: string;
	completed_at: string | null;
	/** Whether this is a renamed model's own migration, which its Renamed row says, not a migration step. */
	is_rename: boolean;
	compatibility?: string;
	transfer?: string | null;
	old_name?: PlanOldName | null;
};

/** A column of `model` renamed from `origin_column` to `destination_column`. */
export type PlanColumnMigration = { model: string; origin_column: string; destination_column: string; decision: string; completed_at?: string | null };

/** A model's table type changing from `actual_type` to `desired_type`. */
export type PlanTableTypeConversion = { model: string; actual_type?: string | null; desired_type?: string | null; downgrade?: boolean; irreversible_warning?: string | null };

/** A time-travel retention change on a relation (`scope` `table` or `schema`), for the models it holds. */
export type PlanRetention = {
	scope?: string;
	name?: string | null;
	models?: string[];
	desired_days?: number | null;
	actual_days?: number | null;
	direction?: string;
	phase?: string;
	irreversible_warning?: string | null;
};

/**
 * `sqb plan --json` as the location printed it; sections the page doesn't show are kept as they are. A
 * server summary has its sections counted and its migration steps with them; only the default mock
 * serves a whole plan, without either.
 */
export type PlanDocument = PlanFields & (WholePlan | PlanSummaryCounts);

/** The default mock's whole plan: every row, nothing counted. */
type WholePlan = { sections?: undefined; migration_steps?: undefined };

/** A server summary's counts. */
type PlanSummaryCounts = {
	/** The nonempty sections in plan order, each with the notes its rows share most. */
	sections: PlanSectionCount[];
	/** The migration steps: migrations other than renamed models' own, column migrations, table type and retention changes. */
	migration_steps: number;
};

/** The fields of every plan document. */
type PlanFields = {
	selected_count: number;
	models: PlanModel[];
	seeds: { name: string; reason: string; qualified_name?: string }[];
	source_loads: { name: string; kind: string; write_strategy?: string; cursor_column?: string; is_reload: boolean }[];
	functions: { name: string; language: string; reason?: string }[];
	python_nodes: { name: string; kind: string; phase: string; identity_status: string }[];
	warnings: PlanWarning[];
	migrations: PlanMigration[];
	column_migrations: PlanColumnMigration[];
	table_type_conversions: PlanTableTypeConversion[];
	retention: PlanRetention[];
	/** What a location's preview planned from instead of the warehouse; `null` for a live plan. */
	synced_copy: PlanSyncedCopy | null;
	/** What `--changed` runs, holds back and skips; `null` without `--changed`. */
	changed: PlanChanged | null;
	/** How long the location took to plan, step by step; absent for plans recorded without timings. */
	timings?: PlanTimings | null;
	/** Set when audit failures won't stop the build this plan previews; absent otherwise. */
	ignore_audit_failures?: boolean;
	/** A server summary's sections, by size; their rows load a page at a time. */
	pages?: Partial<Record<PlanPageSection, number>>;
	/** A server summary's warnings by message template, largest first; the warnings section pages them in this order. */
	warning_groups?: PlanWarningGroup[];
	/** Rows of each paged section loaded so far, which is the offset of its next page. */
	loaded?: Partial<Record<PlanPageSection, number>>;
	/** Providers missing secrets, with the selected nodes they stop; absent when every node can run. */
	cant_run?: PlanCantRun[];
};

/** A provider whose missing secrets stop part of a plan: the nodes that use it and those that read them. */
export type PlanCantRun = {
	reason: 'missing_secret';
	provider: string;
	/** The provider file, relative to the project. */
	path: string;
	/** The unset environment variables, by name; values are never sent. */
	variables: string[];
	/** Node keys that run the provider, such as `source:raw_payments`. */
	nodes: string[];
	/** Selected node keys downstream of those, which can't run either. */
	readers: string[];
	/** The env file the location loads, such as `.env`; null when it loads none. */
	env_file: string | null;
};

/** Every plan section a server summary counts and pages, in plan order. */
export type PlanPageSection =
	| 'full_refresh'
	| 'query_changed'
	| 'function_changed'
	| 'config_changed'
	| 'schema_changed'
	| 'first_run'
	| 'renamed'
	| 'routine_models'
	| 'migrations'
	| 'column_migrations'
	| 'table_type_conversions'
	| 'retention'
	| 'loader_reloads'
	| 'loader_loads'
	| 'source_reloads'
	| 'source_loads'
	| 'changed_functions'
	| 'functions'
	| 'first_run_seeds'
	| 'changed_seeds'
	| 'unchanged_seeds'
	| 'python_nodes'
	| 'held_back'
	| 'unchanged'
	| 'warnings';

/** A note rows of a section carry, such as `run_reason` "no completed record", and how many rows carry it. */
export type PlanSectionNote = { kind: string; value: string | null; count: number };

/** A section's size and its most frequent notes, so the page can say a note applies to all rows, or N of M. */
export type PlanSectionCount = { section: PlanPageSection; total: number; notes: PlanSectionNote[] };

/**
 * One page of a plan section's rows matching a name filter: `total` matches of the section's `section_total`
 * rows, and the `--changed` runs, warnings and cursor window starts of the page's models.
 */
export type PlanPage = {
	total: number;
	section_total: number;
	rows: unknown[];
	runs: PlanChanged['runs'];
	warnings: PlanWarning[];
	cursor_window_starts: Record<string, string>;
};

/** `sqb plan --changed`'s decision. Nodes are keys such as `model:orders`. */
export type PlanChanged = {
	counts: ('definition' | 'source_data')[];
	reach: 'job' | 'all';
	/** Each node that runs, with why; `downstream_of` names the changed upstream it runs for. */
	runs: { node: string; reason: string; downstream_of: string | null }[];
	/** Nodes left for a job's own run (`refreshed_by`), or that read such a node (`reads`). */
	held_back: ({ node: string; refreshed_by: string[] } | { node: string; reads: string })[];
	/** Selected nodes that run only when something they read changed; nothing did. */
	unchanged: string[];
};

/** How long one plan took: steps in pipeline order with `Other` last, and what it read. */
export type PlanTimings = {
	total_seconds: number;
	steps: { name: string; seconds: number }[];
	/** `synced_state` for a location preview over the synced warehouse state; `live` when it read the warehouse. */
	planned_from: string;
	/** When the plan the location kept, and answered with again, was made; absent for a new plan. */
	reused_from?: string | null;
};

/** The server's synced copy of warehouse metadata a location's plan preview planned from. */
export type PlanSyncedCopy = {
	/** When every table the preview reads last synced; `null` until all of them have. */
	synced_at: string | null;
	/** Each incremental model's window start; its end is the latest data when the run starts. */
	cursor_window_starts: Record<string, string>;
	/** What the preview could not know, which the run plans live. */
	unknown_until_run: string[];
};

/** The location's plan of a build command, or why it could not plan it. */
export type PlanResult = { plan: PlanDocument; error: null; requestId?: string } | { plan: null; error: string };

/** Cursor window for a launch, typed from the selected nodes' cursor type. */
export type CursorWindow = {
	type: 'timestamp' | 'integer';
	start: string | null;
	end: string | null;
};

export type JobLaunchOptions = {
	window?: CursorWindow;
	/** Runs only the job's nodes these `sqb build` selectors also select; the run stays a run of the job. */
	select?: string[];
	/** Leaves the job's nodes these selectors select out of the run. */
	exclude?: string[];
};

/** `selected` reruns exactly the nodes passed with the request. */
export type RerunMode = 'all' | 'from_failure' | 'selected';

// ---- jobs and job groups ----

/** What a schedule tick or sensor request does while a run of the job is active; people's runs always queue. */
export type OverlapPolicy = 'queue' | 'skip';
/** A job's `[retry]`: a failed or partial run reruns from failure up to `retries` times (`attempts` is one more, counting the first run), `retryDelaySeconds` (`delay_seconds`) after the last ended. */
export type OnFailure = { retries: number; retryDelaySeconds: number };
export type JobKind = 'job' | 'group';

export type RunRef = Pick<Run, 'id' | 'status' | 'startedAt' | 'endedAt'>;

export type Job = {
	key: string;
	kind: JobKind;
	description: string;
	/** Selector for jobs; null for groups. */
	select: string | null;
	/** Member jobs for groups; null when the server doesn't say. */
	members: string[] | null;
	nodeCount: number;
	/** The nodes the job selects, by catalog key; a group's are its members' together. */
	nodes: string[];
	/** Null where the server doesn't report it (also `onFailure`, `priority`, `sourceFile`). */
	overlap: OverlapPolicy | null;
	onFailure: OnFailure | null;
	/** Each pool the job takes: its slots, the pool's limit and the other jobs and groups taking it, as published. */
	pools: PoolUse[];
	/** The job's own `concurrency` setting, models building at once in a run; null uses the project's or target's. */
	concurrency: number | null;
	/** What a run uses on the target and which layer set it; null for a snapshot published before it was recorded. */
	resolvedConcurrency: ResolvedConcurrency | null;
	/** The job's own `ignore_audit_failures` setting: failing audits don't stop its runs; null uses the target's, then the project's. */
	ignoreAuditFailures: boolean | null;
	priority: number | null;
	owner: string | null;
	sourceFile: string | null;
	triggerIds: string[];
	lastRun: RunRef | null;
};

/** A pool a job takes: `amount` of its `limit` slots, shared with the other jobs and groups taking it. */
export type PoolUse = {
	pool: string;
	/** The slots the job asks for. */
	amount: number;
	/** The project's limit on the target. */
	limit: number;
	/** This server's `pool_caps` entry; null when it sets none. */
	cap: number | null;
	/** The limit on this server: `limit` lowered to `cap`. */
	serverLimit: number;
	/** `amount` is above `serverLimit`, so on this server the job takes the whole pool. */
	clamped: boolean;
	sharedWith: string[];
};

/** The most of a job's nodes a run of it builds at once. */
export type ResolvedConcurrency = { limit: number };

/** One reason a member waits: `fromNode` of `upstreamJob` feeds its `toNode`, directly or through nodes no member selects. */
export type WaitReason = { upstreamJob: string; fromNode: string; toNode: string };

export type PoolHolders = {
	pool: string;
	limit: number;
	/** Active runs holding its slots. */
	holders: string[];
};

export type JobGroupMember = {
	jobKey: string;
	/** Jobs this member waits for, from lineage; null where the cause isn't reported. */
	upstream: { jobKey: string; source: 'lineage' | null }[];
	/** Position in the derived order; members in the same stage run side by side. */
	stage: number;
	/** Node-level reasons for its waits, as the snapshot records them; every member's is empty in a snapshot published before they were recorded. */
	reasons: WaitReason[];
};

export type JobDetail = Job & {
	/** Null where the server doesn't report it, as for `declaration`, `poolHolders` and `targets`. */
	declaration: { sourceFile: string; line: number; code: string } | null;
	exclude: string[];
	/** Settings are CLI flags validated against the command table. */
	settings: Record<string, string | boolean> | null;
	resolvedNodes: string[];
	poolHolders: PoolHolders[] | null;
	/** Null when the job applies to every target. */
	targets: string[] | null;
	groupMembers: JobGroupMember[] | null;
	criticalNodes: string[];
	/** Groups this job belongs to. */
	memberOf: string[] | null;
	/** Nodes with a cursor, so a cursor window applies. */
	cursorType: 'timestamp' | 'integer' | null;
};

// ---- triggers and sensors ----

export type TriggerState = 'running' | 'paused';
export type TriggerType = 'schedule' | 'sensor';
export type TriggerKind =
	| 'cron_schedule'
	| 'calendar_schedule'
	/** Built-in: a job or group finished, with optional node conditions. */
	| 'job_sensor'
	/** Built-in: a node or source changed in any run (rebuilt, failed, new data, stale). */
	| 'node_sensor'
	/** Built-in condition on platform events, with a code action. */
	| 'platform_sensor'
	/** Custom `@sensor` evaluated in the code location. */
	| 'custom_sensor';

export const TICK_OUTCOMES = ['requested', 'skipped', 'covered', 'failed'] as const;
export type TickOutcome = (typeof TICK_OUTCOMES)[number];

export type Tick = {
	id: string;
	scheduledFor: Iso;
	evaluatedAt: Iso | null;
	outcome: TickOutcome;
	runId: string | null;
	coveredByRunId: string | null;
	/** The run a `skipped` tick yielded to, because the job's overlap is `skip`. */
	activeRunId: string | null;
	reason: string | null;
	/** Evaluated after the grace period (scheduler down or behind). */
	lateBySeconds: number;
	/** Recorded during catch-up after downtime. */
	catchUp: boolean;
};

export type Trigger = {
	/** Deterministic from (location, target, kind, name). */
	id: string;
	name: string;
	type: TriggerType;
	kind: TriggerKind;
	description: string;
	/** Job or group the trigger launches; null for sensors that only run code. */
	jobKey: string | null;
	/** Human-readable cadence or condition. */
	cadence: string;
	cron: string | null;
	timezone: string | null;
	/** State declared in code. */
	defaultState: TriggerState;
	/** Effective runtime state, after any override. */
	state: TriggerState;
	nextDueAt: Iso | null;
	lastTick: Tick | null;
	lastRun: RunRef | null;
	sourceFile: string | null;
	/** Code changed since the last evaluation (sensors). */
	definitionChanged: boolean;
	/** The watchdog's warning, e.g. an overdue tick or a low calendar horizon. */
	warning: string | null;
};

export type TriggerStateEventKind = 'enabled' | 'paused' | 'reset_to_default' | 'cursor_set' | 'cursor_reset';

export type TriggerStateEvent = {
	id: string;
	at: Iso;
	kind: TriggerStateEventKind;
	actor: Principal;
	reason: string | null;
	value: string | null;
};

export type SensorEvaluationOutcome = 'requested' | 'completed' | 'failed' | 'timed_out' | 'abandoned';

export type SensorEvaluation = {
	id: string;
	/** The latest phase recorded; `requested` alone means in flight. */
	outcome: SensorEvaluationOutcome;
	requestedAt: Iso;
	finishedAt: Iso | null;
	cursorBefore: string | null;
	cursorAfter: string | null;
	runRequests: number;
	skipReason: string | null;
	/** Active runs that matching requests yielded to, because the job's overlap is `skip`. */
	skippedFor: string[];
	error: string | null;
	definitionFingerprint: string;
};

export type SensorRunRequest = {
	runKey: string;
	runId: string;
	evaluationId: string | null;
	requestedAt: Iso;
	runStatus: RunStatus;
};

export type CatchUpPolicy = { mode: 'latest' | 'all'; max: number | null };

export type TriggerDetail = Trigger & {
	/** Null where the server doesn't report it, as for `upcoming`, `evaluations`, `runRequests` and `stateHistory`. */
	declaration: { sourceFile: string; line: number; code: string } | null;
	targets: string[] | null;
	catchUp: CatchUpPolicy | null;
	/** Upcoming due times (schedules). */
	upcoming: Iso[] | null;
	/** Calendar schedules: times are computed ahead and stored up to `storedUntil`. */
	calendar: {
		/** Null where the server doesn't report it. */
		horizon: string | null;
		/** The last stored time; null when none is stored. */
		storedUntil: Iso | null;
		lastRefreshedAt: Iso | null;
		/** When the location next works out the calendar; absent where the server doesn't report it. */
		nextRefreshAt?: Iso | null;
		lowHorizonWarningBelow: string | null;
		low: boolean;
	} | null;
	sensor: {
		/** Built-in condition or the custom function, as declared. */
		condition: string;
		nodeConditions: string[];
		minimumIntervalSeconds: number;
		timeoutSeconds: number | null;
		cursor: string | null;
		cursorUpdatedAt: Iso | null;
		/** False when the server doesn't report the cursor; resetting it still works. */
		cursorKnown: boolean;
		currentFingerprint: string | null;
		lastEvaluatedFingerprint: string | null;
	} | null;
	ticks: Tick[];
	evaluations: SensorEvaluation[] | null;
	runRequests: SensorRunRequest[] | null;
	stateHistory: TriggerStateEvent[] | null;
};
