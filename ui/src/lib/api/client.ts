import type {
	DiscoveredSet,
	AdhocRunCheck,
	PlanResult,
	PlanPage,
	PlanDiff,
	PlanPageSection,
	AdhocRunRequest,
	Bootstrap,
	BulkRunResult,
	CatalogNode,
	CatalogNodeDetail,
	Check,
	CheckDetail,
	CheckEvidence,
	CheckSql,
	ColumnLineage,
	Job,
	JobDetail,
	JobLaunchOptions,
	NavCounts,
	Overview,
	PlatformEvent,
	QueueEntry,
	RerunMode,
	Run,
	RunDetail,
	RunEventPage,
	RunListQuery,
	RunNodeError,
	RunOutputFollow,
	RunOutputLine,
	NodeOutputIndex,
	NodeOutputPage,
	NodeOutputSearch,
	CheckSample,
	RunRowPage,
	RunRowQuery,
	RunStatement,
	IgnoredAuditFailure,
	RunCostRow,
	CostSummary,
	CostSelection,
	CostSelectionQuery,
	CostSeriesGroup,
	AutomaticWithdrawal,
	ConditionShape,
	NodeBuildCost,
	NodeStates,
	Snapshot,
	RunTabCounts,
	RepublishResult, Scope,
	SnapshotChange,
	SelectionEvaluation,
	SelectionRequest,
	SelectionVocabulary,
	SnapshotNode,
	StatementSql,
	Timeline,
	Trigger,
	TriggerDetail,
	TriggerState,
	WarehouseSync
} from './types';
import type { ChangeSubscription } from './signals';
import type { PlatformApi } from './platform-client';

/**
 * Everything the UI reads from or asks of the server.
 *
 * Pages depend on this interface only. The mock implementation stands in until
 * the server's OpenAPI client exists; that client is wrapped to implement the
 * same interface.
 */
export interface ApiClient extends PlatformApi {
	/** Whether the server has an endpoint for `method`; the shell hides what it cannot load. */
	supports(method: keyof ApiClient): boolean;

	bootstrap(): Promise<Bootstrap>;

	navCounts(scope: Scope): Promise<NavCounts>;

	// ---- BUILD: overview, catalog and lineage, checks, changes (types/build.ts) ----
	/**
	 * Health, attention items, metrics, build status by group and recent changes; null when the
	 * person may view only part of the target, because its listings need view on every node.
	 */
	overview(scope: Scope, listing?: CatalogNode[]): Promise<Overview | null>;
	/**
	 * Every node in the current snapshot with its build state; also the lineage graph. Pages read it
	 * through the shared catalog store (`$lib/state/catalog.svelte`), which reads it once per snapshot.
	 */
	listCatalogNodes(scope: Scope): Promise<CatalogNode[]>;
	/** The current snapshot's discovered sets and the nodes their items became; none before a publication. */
	readDiscovered(scope: Scope): Promise<DiscoveredSet[]>;
	/** Every node's build state at the current status version, for applying `node_status` changes to a listing. */
	listNodeStates(scope: Scope): Promise<NodeStates>;
	/** `listing` and `checks` are current `listCatalogNodes` and `listChecks` results the caller already holds, so they aren't read again. */
	getCatalogNode(scope: Scope, key: string, listing?: CatalogNode[], checks?: Check[]): Promise<CatalogNodeDetail | null>;
	/** The snapshot's column lineage around one node; `listing` as for `getCatalogNode`. */
	getColumnLineage(scope: Scope, key: string, listing?: CatalogNode[]): Promise<ColumnLineage | null>;
	/**
	 * Every audit and check with its latest result; a repeat the server answers with 304 returns the held list.
	 * A read already in flight is joined only if it began after `notBefore` (`performance.now()`), such as a change signal.
	 */
	listChecks(scope: Scope, notBefore?: number): Promise<Check[]>;
	getCheck(scope: Scope, id: string): Promise<CheckDetail | null>;
	/** The latest evaluation's SQL and error texts. `view.sql`; every read is audited. */
	getCheckSql(scope: Scope, checkId: string): Promise<CheckSql>;
	/** Sample rows stored with a result. `view.evidence`; every read is audited. */
	getCheckEvidence(scope: Scope, checkId: string, evaluationId: string): Promise<CheckEvidence>;
	/** The change feed: one entry per published snapshot, newest first. */
	listSnapshotChanges(scope: Scope, query?: { limit?: number }): Promise<SnapshotChange[]>;
	// ---- end BUILD ----

	// ---- OPERATE: runs, queue, plan page, jobs and triggers (types/operate.ts) ----
	listRuns(scope: Scope, query?: RunListQuery): Promise<Run[]>;
	/** One page of the runs list, newest first, with the facts each row's status line needs. */
	listRunRows(scope: Scope, query?: RunRowQuery): Promise<RunRowPage>;
	/** Active and queued counts for the status tabs; fetched apart from the rows. */
	runTabCounts(scope: Scope, query?: RunRowQuery): Promise<RunTabCounts>;
	/** Requests a cancel of every active run matching the query ("Cancel all"). `run.cancel` */
	cancelMatchingRuns(scope: Scope, query: RunRowQuery): Promise<BulkRunResult>;
	getRun(scope: Scope, runId: string): Promise<Run | null>;
	/** Nodes, children, liveness, cancel progress and provenance for one run. */
	getRunDetail(scope: Scope, runId: string): Promise<RunDetail | null>;
	/** Recall events and scheduler facts in storage order, paged by `after`. */
	listRunEvents(scope: Scope, runId: string, query?: { after?: number; limit?: number }): Promise<RunEventPage>;
	listRunStatements(scope: Scope, runId: string): Promise<RunStatement[]>;
	/** The audit failures a run and its group's current member runs let through, run by run. */
	listRunIgnoredAudits(scope: Scope, runId: string): Promise<IgnoredAuditFailure[]>;
	/** The latest estimated query cost observations of one run; empty until a collection synced. */
	listRunCost(scope: Scope, runId: string): Promise<RunCostRow[]>;
	/** Estimated query cost of the runs that started in the last `days` days, by job, warehouse size and node. */
	getCostSummary(scope: Scope, days: number): Promise<CostSummary>;
	/** A selection's estimated query cost over the last `query.days` whole UTC days, job by node. */
	readCostSelection(scope: Scope, query: CostSelectionQuery): Promise<CostSelection>;
	/** Daily credits of up to 200 groups of a selection, in the groups' order. */
	readCostSeries(scope: Scope, query: CostSelectionQuery, groups: CostSeriesGroup[]): Promise<number[][]>;
	/** One node's estimated query cost per build over the last `days` days, newest first. */
	listNodeCosts(scope: Scope, nodeKey: string, days: number): Promise<NodeBuildCost[]>;
	/** One statement's SQL, fetched when its row is expanded. `view.sql` */
	getStatementSql(scope: Scope, runId: string, statementId: string): Promise<StatementSql>;
	listRunOutput(scope: Scope, runId: string): Promise<RunOutputLine[]>;
	/** The run's output from the output store since line `after` (the newest when null), with its `ctx.log` lines; null when the server keeps no output store. `view.evidence` */
	followRunOutput?(scope: Scope, runId: string, after: number | null): Promise<RunOutputFollow | null>;
	/** A node's output in the output store: how much there is and its error and warning lines, read by range. `view.evidence` */
	getNodeOutputIndex?(scope: Scope, runId: string, nodeKey: string): Promise<NodeOutputIndex | null>;
	getNodeOutputPage?(scope: Scope, runId: string, nodeKey: string, at: { from: number; rows: number; direction: 'forward' | 'backward'; collapse: boolean; stream?: 'all' | 'stdout' | 'stderr'; around?: number }): Promise<NodeOutputPage>;
	searchNodeOutput?(scope: Scope, runId: string, nodeKey: string, query: string, from: number, direction: 'forward' | 'backward', signal?: AbortSignal): Promise<NodeOutputSearch>;
	/** Where the server streams a node's whole output as a file. */
	nodeOutputDownloadUrl?(scope: Scope, runId: string, nodeKey: string): string;
	/** A failing check's sample rows, by the check's statement. */
	getCheckSample?(scope: Scope, runId: string, statementId: string): Promise<CheckSample | null>;
	/** One failed node's error text, fetched when its row is expanded. `view.evidence` on the node */
	getRunNodeError(scope: Scope, runId: string, nodeKey: string): Promise<RunNodeError>;
	/** Queued runs in claim order, with why each one waits. */
	listQueue(scope: Scope): Promise<QueueEntry[]>;
	timeline(scope: Scope, window: { since: string; until: string }): Promise<Timeline>;
	/** Marks a run handled for everyone who can see it, or withdraws the mark; recorded in the audit log. */
	acknowledgeRun(scope: Scope, runId: string, acknowledged: boolean): Promise<void>;
	/** Acknowledge an ongoing condition as it looks now (`shape`), until it changes, or withdraw that. */
	/** `automatic` withdraws one acknowledgement, which the server applies only while it is still the latest. */
	acknowledgeCondition(scope: Scope, key: string, shape: ConditionShape, acknowledged: boolean, automatic?: AutomaticWithdrawal): Promise<void>;
	/** Dismisses for good the problems of stored events webhooks and sensors skipped as unreadable, by storage ID; they stay in the history. */
	dismissSkippedEvents(scope: Scope, storageIds: number[]): Promise<void>;
	/** Writes a `cancel_requested` fact; the scheduler then signals the worker. */
	cancelRun(scope: Scope, runId: string): Promise<Run>;
	/** `nodes` is required for `selected` and ignored otherwise. */
	rerunRun(scope: Scope, runId: string, mode: Exclude<RerunMode, 'selected'>): Promise<Run>;
	/** Reruns exactly `nodes` of the run. */
	rerunSelectedNodes(scope: Scope, runId: string, nodes: string[]): Promise<Run>;

	listSnapshotNodes(scope: Scope): Promise<SnapshotNode[]>;
	/** Resolves a selection exactly as `sqb build` would; a refused selection is a result with its error, not a throw. */
	evaluateSelection(scope: Scope, request: SelectionRequest): Promise<SelectionEvaluation>;
	/** Tags, folders, jobs, sources, seeds and kinds of the current publication, with node counts. */
	selectionVocabulary(scope: Scope): Promise<SelectionVocabulary>;
	/** The server's pre-launch check: resolution, write set and authorization. */
	checkAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<AdhocRunCheck>;
	launchAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<Run>;
	/**
	 * Asks the location to run `sqb plan` on a build command against the warehouse, and waits for it.
	 * The location may answer with the plan it kept for the same command; `fresh` makes it plan again.
	 */
	planBuild(scope: Scope, command: string[], options?: { fresh?: boolean }): Promise<PlanResult>;
	/** One page of a large section of the plan `requestId` names, which its summary only counts. */
	/** One page of a plan section; `name` finds rows whose name contains it, ignoring case. */
	planRows(scope: Scope, requestId: string, section: PlanPageSection, offset: number, limit?: number, name?: string): Promise<PlanPage>;
	/** One planned model's diff text, read on demand; null without one. */
	planDiff(scope: Scope, requestId: string, model: string): Promise<PlanDiff | null>;

	listJobs(scope: Scope): Promise<Job[]>;
	getJob(scope: Scope, key: string): Promise<JobDetail | null>;
	launchJob(scope: Scope, key: string, options?: JobLaunchOptions): Promise<Run>;
	/** Launches with `--full-refresh`, rebuilding the job's tables from scratch. */
	launchJobFromScratch(scope: Scope, key: string, options?: JobLaunchOptions): Promise<Run>;

	listTriggers(scope: Scope): Promise<Trigger[]>;
	getTrigger(scope: Scope, id: string): Promise<TriggerDetail | null>;
	/** One audited state fact per trigger. */
	setTriggerStates(scope: Scope, ids: string[], state: TriggerState, reason?: string | null): Promise<Trigger[]>;
	resetTriggerToDefault(scope: Scope, id: string): Promise<Trigger>;
	resetSensorCursor(scope: Scope, id: string, reason?: string | null): Promise<TriggerDetail>;
	/** Mark the event a sensor is stuck on delivered without calling its function; the reason is required. */
	skipSensorEvent(scope: Scope, id: string, eventId: string, reason: string): Promise<TriggerDetail>;
	// ---- end OPERATE ----

	listPlatformEvents(query: { activeOnly: boolean }): Promise<PlatformEvent[]>;

	/** The newest version deployed to the location; null before its first deploy. */
	getLatestSnapshot(scope: Scope): Promise<Snapshot | null>;
	/** Null when the location has reported no recent sync. */
	getWarehouseSync(scope: Scope): Promise<WarehouseSync | null>;
	/** Asks the location to read SQLBuild's state back from the warehouse now. `connection.test` */
	requestWarehouseSync(scope: Scope): Promise<void>;

	/** Asks the location to compile and publish again, and waits for the result. `location.republish` */
	republish(scope: Scope): Promise<RepublishResult>;

	/** Live "changed" signals (SSE in the server client); consumers refetch or apply their payload. */
	changes: ChangeSubscription;
}

export class ApiError extends Error {
	constructor(
		message: string,
		readonly status: number
	) {
		super(message);
		this.name = 'ApiError';
	}
}

/** A server or recording without the read, such as the static demo's without node states. */
export function isUnsupported(error: unknown): boolean {
	return error instanceof ApiError && (error.status === 404 || error.status === 405 || error.status === 501);
}

/** The server refused the read or no longer has what it names, so an earlier answer must not stay on screen. */
export function isRefusal(error: unknown): boolean {
	return error instanceof ApiError && (error.status === 403 || error.status === 404);
}
