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

export type NodeKind = 'source' | 'loader' | 'seed' | 'view' | 'table' | 'incremental' | 'task';

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
	/** Where in the project the failure points, e.g. the model file and line of the failing statement. */
	errorLocation: { file: string; line: number } | null;
	/** Full error text (warehouse message, statement, trace); `errorMessage` is its first line. */
	errorDetail: string | null;
	skippedBecauseOf: SkippedBecauseOf | null;
	/** Node failure stops the whole group (`critical = true`). */
	critical: boolean;
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
	'terminated'
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

/** One statement's SQL, loaded on demand. Null when it was not recorded. */
export type StatementSql = { statementId: string; sql: string | null };

export type RunOutputLine = {
	invocationId: string;
	stream: 'stdout' | 'stderr';
	at: Iso;
	text: string;
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
	gitRevision: string;
	gitDirty: boolean;
	imageId: string;
	sqbVersion: string;
	/** The frozen selection the run was authorized for. */
	selection: string[];
	exclude: string[];
	command: string;
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

export type QueueWait =
	| { kind: 'write_claim'; runId: string; jobKey: string | null; nodeKey: string; otherNodes: number }
	| { kind: 'concurrency_group'; group: string; limit: number; holders: string[] }
	| { kind: 'priority'; aheadRunIds: string[] }
	| { kind: 'upstream_job'; runId: string; jobKey: string }
	/** A child run waits for its group run to be claimed. */
	| { kind: 'group_parent'; runId: string; jobKey: string };

export type OverlapOutcome = {
	policy: OverlapPolicy;
	/** Ticks merged into this waiting run (`coalesce`). */
	coveredTicks: number;
	detail: string;
};

export type QueueEntry = {
	run: Run;
	position: number;
	waits: QueueWait[];
	overlap: OverlapOutcome | null;
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

// ---- ad-hoc runs (the run panel) ----

export type LaunchVerb = 'build' | 'test' | 'seed' | 'audit' | 'freshness';

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
	/** Why the command cannot run regardless of permissions, such as a refused flag or a verb workers can't run yet. */
	error: string | null;
	launchable: boolean;
};

/** Cursor window for a launch, typed from the selected nodes' cursor type. */
export type CursorWindow = {
	type: 'timestamp' | 'integer';
	start: string | null;
	end: string | null;
};

export type JobLaunchOptions = {
	window?: CursorWindow;
};

/** `selected` reruns exactly the nodes passed with the request. */
export type RerunMode = 'all' | 'from_failure' | 'selected';

// ---- jobs and job groups ----

export type OverlapPolicy = 'coalesce' | 'queue' | 'allow';
export type OnFailure = 'skip_dependents' | 'skip_downstream_jobs' | 'stop_group';
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
	/** Null where the server doesn't report it (also `onFailure`, `priority`, `sourceFile`). */
	overlap: OverlapPolicy | null;
	onFailure: OnFailure | null;
	concurrencyGroups: string[];
	priority: number | null;
	owner: string | null;
	sourceFile: string | null;
	triggerIds: string[];
	lastRun: RunRef | null;
};

export type ConcurrencyGroup = {
	key: string;
	limit: number;
	/** Active runs holding a slot. */
	holders: string[];
};

export type JobGroupMember = {
	jobKey: string;
	/** Jobs this member waits for: lineage-derived plus declared `after`. */
	upstream: { jobKey: string; source: 'lineage' | 'after' }[];
	/** Position in the derived order; members in the same stage run side by side. */
	stage: number;
};

export type JobDetail = Job & {
	/** Null where the server doesn't report it, as for `settings`, `concurrency`, `groupMembers` and `memberOf`. */
	declaration: { sourceFile: string; line: number; code: string } | null;
	exclude: string[];
	/** Settings are CLI flags validated against the command table. */
	settings: Record<string, string | boolean> | null;
	resolvedNodes: string[];
	concurrency: ConcurrencyGroup[] | null;
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
	sequence: number;
	/** The latest phase recorded; `requested` alone means in flight. */
	outcome: SensorEvaluationOutcome;
	requestedAt: Iso;
	finishedAt: Iso | null;
	cursorBefore: string | null;
	cursorAfter: string | null;
	runRequests: number;
	skipReason: string | null;
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
