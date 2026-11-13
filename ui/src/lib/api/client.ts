import type {
	AdhocRunCheck,
	PlanResult,
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
	RunOutputLine,
	RunRowPage,
	RunRowQuery,
	RunStatement,
	Snapshot,
	RunTabCounts,
	Scope,
	SnapshotChange,
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
	overview(scope: Scope): Promise<Overview | null>;
	/** Every node in the current snapshot with its build state; also the lineage graph. */
	listCatalogNodes(scope: Scope): Promise<CatalogNode[]>;
	/** `listing` is a current `listCatalogNodes` result the caller already holds, so it isn't read again. */
	getCatalogNode(scope: Scope, key: string, listing?: CatalogNode[]): Promise<CatalogNodeDetail | null>;
	/** The snapshot's column lineage around one node. */
	getColumnLineage(scope: Scope, key: string): Promise<ColumnLineage | null>;
	/** Every audit and check with its latest result. */
	listChecks(scope: Scope): Promise<Check[]>;
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
	/** One statement's SQL, fetched when its row is expanded. `view.sql` */
	getStatementSql(scope: Scope, runId: string, statementId: string): Promise<StatementSql>;
	listRunOutput(scope: Scope, runId: string): Promise<RunOutputLine[]>;
	/** One failed node's error text, fetched when its row is expanded. `view.evidence` on the node */
	getRunNodeError(scope: Scope, runId: string, nodeKey: string): Promise<RunNodeError>;
	/** Queued runs in claim order, with why each one waits. */
	listQueue(scope: Scope): Promise<QueueEntry[]>;
	timeline(scope: Scope, window: { since: string; until: string }): Promise<Timeline>;
	/** Marks a run handled for everyone who can see it, or withdraws the mark; recorded in the audit log. */
	acknowledgeRun(scope: Scope, runId: string, acknowledged: boolean): Promise<void>;
	/** Writes a `cancel_requested` fact; the scheduler then signals the worker. */
	cancelRun(scope: Scope, runId: string): Promise<Run>;
	/** `nodes` is required for `selected` and ignored otherwise. */
	rerunRun(scope: Scope, runId: string, mode: Exclude<RerunMode, 'selected'>): Promise<Run>;
	/** Reruns exactly `nodes` of the run. */
	rerunSelectedNodes(scope: Scope, runId: string, nodes: string[]): Promise<Run>;

	listSnapshotNodes(scope: Scope): Promise<SnapshotNode[]>;
	/** The server's pre-launch check: resolution, write set and authorization. */
	checkAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<AdhocRunCheck>;
	launchAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<Run>;
	/** Asks the location to run `sqb plan` on a build command against the warehouse, and waits for it. */
	planBuild(scope: Scope, command: string[]): Promise<PlanResult>;

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
	// ---- end OPERATE ----

	listPlatformEvents(query: { activeOnly: boolean }): Promise<PlatformEvent[]>;

	/** The newest version deployed to the location; null before its first deploy. */
	getLatestSnapshot(scope: Scope): Promise<Snapshot | null>;
	/** Null when the location has reported no recent sync. */
	getWarehouseSync(scope: Scope): Promise<WarehouseSync | null>;
	/** Asks the location to read SQLBuild's state back from the warehouse now. `connection.test` */
	requestWarehouseSync(scope: Scope): Promise<void>;

	/** Live "changed" signals (SSE in the server client); consumers refetch. */
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

/** The server refused the read or no longer has what it names, so an earlier answer must not stay on screen. */
export function isRefusal(error: unknown): boolean {
	return error instanceof ApiError && (error.status === 403 || error.status === 404);
}
