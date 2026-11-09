import createFetch from 'openapi-fetch';
import type { components, paths } from '$control/schema.d.ts';
import { ApiError, type ApiClient } from './client';
import { checkDetailOf, toCheck, toCheckEvidence, toCheckSql } from './http-checks';
import { findNode, nodeDetailOf, toCatalogNodes, nodeJobsOf, toNodeRuns, toColumnLineage } from './http-catalog';
import { auditMatches, toAuditEntry } from './http-audit';
import { toSnapshotChanges } from './http-changes';
import { manualConnectionTest, toLocationDetail, toLocationSummary, warehouseSyncOf } from './http-locations';
import { toAccount, toRoleAssignments } from './http-people';
import { toMaintenanceHistory } from './http-maintenance';
import { overviewOf } from './http-overview';
import { toPlatformEvents, toPlatformHealth } from './http-problems';
import { roleDetailOf, toRoleDefinition } from './http-roles';
import { jobDetailOf, runWindowOf, toJob, toTick, toTrigger, triggerDetailOf } from './http-jobs';
import { RUN_WINDOW, toAdhocCheck, runDetailOf, runRowPage, runsMatching, runTabCountsOf, toRunNode, toRuns, toRunEvent, toRunOutput, toRunStatements } from './http-runs';
import type { ChangeSubscription, ChangedChannel, ChangedSignal, ConnectionState } from './signals';
import type { AuthMode, Bootstrap, Capability, Location, PlatformEvent, Scope, WarehouseSync } from './types';
import type { Account, AuditEntry, AuditQuery, ConnectionTest, LocationDetail, LocationSummary, PlatformHealth, MaintenanceHistory, RoleAssignment, RoleAssignmentRequest, RoleDefinition, RoleDetail } from './types/platform';
import type { CatalogNode, CatalogNodeDetail, Check, CheckDetail, CheckEvidence, CheckSql, Overview, SnapshotChange, ColumnLineage } from './types/build';
import type { AdhocRunCheck, AdhocRunRequest, Job, JobDetail, JobLaunchOptions, RerunMode, SnapshotNode, Run, RunDetail, RunListQuery, RunRowPage, RunRowQuery, RunTabCounts, Trigger, TriggerDetail, TriggerState, RunEventPage, RunNodeError, RunOutputLine, RunStatement, StatementSql } from './types/operate';

type Wire = components['schemas'];

/** The client methods the server has endpoints for; the shell hides pages that need others. */
const SUPPORTED: ReadonlySet<string> = new Set<keyof ApiClient>([
	'supports',
	'bootstrap',
	'overview',
	'changes',
	'listRunRows',
	'runTabCounts',
	'listRuns',
	'getRun',
	'getRunDetail',
	'listRunEvents',
	'listRunStatements',
	'getStatementSql',
	'listRunOutput',
	'getRunNodeError',
	'cancelRun',
	'rerunRun',
	'listJobs',
	'getJob',
	'launchJob',
	'launchJobFromScratch',
	'listTriggers',
	'getTrigger',
	'setTriggerStates',
	'resetTriggerToDefault',
	'resetSensorCursor',
	'listChecks',
	'getCheck',
	'getCheckSql',
	'getCheckEvidence',
	'listCatalogNodes',
	'getCatalogNode',
	'getColumnLineage',
	'listSnapshotChanges',
	'listAuditEntries',
	'listAccounts',
	'listRoleAssignments',
	'createRoleAssignment',
	'deleteRoleAssignment',
	'disableAccount',
	'enableAccount',
	'listLocations',
	'getLocation',
	'listPlatformEvents',
	'platformHealth',
	'listRoles',
	'getRole',
	'getWarehouseSync',
	'maintenanceHistory',
	'testConnection',
	'listSnapshotNodes',
	'checkAdhocRun',
	'launchAdhocRun'
]);

/** How often a connection test asks for the location's answer, and the longest it waits. */
const CONNECTION_TEST_POLL_MS = 1000;
const CONNECTION_TEST_WAIT_MS = 60_000;

/** The server's largest audit page, and how many the audit log reads before it stops looking. */
const AUDIT_PAGE = 100;
const AUDIT_PAGES = 10;

/** A UI location is one server location and target; its id carries both. */
export function locationKey(locationId: string, target: string): string {
	return `${locationId}:${target}`;
}

export function parseLocationKey(key: string): { locationId: string; target: string } {
	const split = key.indexOf(':');
	return { locationId: key.slice(0, split), target: key.slice(split + 1) };
}

export function toBootstrap(wire: Wire['ControlBootstrap']): Bootstrap {
	const authMode: AuthMode = wire.login_mode;
	return {
		authMode,
		user: { kind: 'user', name: wire.access.name, displayName: wire.access.name },
		capabilities: wire.access.actions as Capability[],
		locations: wire.access.locations.map(
			(location): Location => ({
				id: locationKey(location.location_id, location.target),
				name: location.name,
				health: null,
				lastSeenAt: null,
				target: { name: location.target, adapter: null, connectionSummary: null },
				warehouseSync: { lastSyncedAt: null, lastError: null },
				capabilities: location.actions as Capability[]
			})
		)
	};
}

/** The person change feed URL of a UI location. */
export function feedUrl(baseUrl: string, scope: Scope): string {
	const { locationId, target } = parseLocationKey(scope.locationId);
	return `${baseUrl}/api/control/locations/${encodeURIComponent(locationId)}/targets/${encodeURIComponent(target)}/changes`;
}

/**
 * Whether the feed would open now: a refusal (401 or 403) stops retrying, anything else retries.
 * It asks the feed itself, because the bootstrap lists a location on partial view while the feed
 * needs view on every node, so a bootstrap check would retry a refused feed forever.
 */
export async function feedAllowed(baseUrl: string, scope: Scope, onUnauthorized: () => void): Promise<boolean> {
	const probe = new AbortController();
	const response = await fetch(feedUrl(baseUrl, scope), {
		credentials: 'same-origin',
		headers: { accept: 'text/event-stream' },
		signal: probe.signal
	});
	probe.abort();
	if (response.status === 401) onUnauthorized();
	return response.status !== 401 && response.status !== 403;
}

/** The change feed of the followed location: one `EventSource`, recreated when the scope changes. */
export class HttpChangeFeed implements ChangeSubscription {
	#handlers = new Set<(signal: ChangedSignal) => void>();
	#stateHandlers = new Set<(state: ConnectionState) => void>();
	#state: ConnectionState = 'connecting';
	#source: EventSource | null = null;
	#retry: ReturnType<typeof setTimeout> | null = null;
	#failures = 0;

	/** `stillAllowed` checks access before a retry, so a lost session or refused feed stops retrying. */
	constructor(
		private readonly baseUrl: string,
		private readonly stillAllowed: (scope: Scope) => Promise<boolean>
	) {}

	subscribe(handler: (signal: ChangedSignal) => void): () => void {
		this.#handlers.add(handler);
		return () => this.#handlers.delete(handler);
	}

	onConnectionState(handler: (state: ConnectionState) => void): () => void {
		this.#stateHandlers.add(handler);
		handler(this.#state);
		return () => this.#stateHandlers.delete(handler);
	}

	follow(scope: Scope | null): void {
		this.#failures = 0;
		this.#connect(scope);
	}

	#connect(scope: Scope | null) {
		this.#source?.close();
		this.#source = null;
		if (this.#retry) clearTimeout(this.#retry);
		this.#retry = null;
		if (!scope) return this.#setState('offline');
		const source = new EventSource(feedUrl(this.baseUrl, scope));
		this.#source = source;
		this.#setState('connecting');
		source.onopen = () => {
			this.#failures = 0;
			this.#setState('live');
		};
		// The browser retries a dropped stream by itself, but gives up on any failed response.
		source.onerror = () => {
			if (source.readyState !== EventSource.CLOSED) return this.#setState('reconnecting');
			this.#failures += 1;
			this.#setState(this.#failures > 3 ? 'offline' : 'reconnecting');
			const delay = Math.min(30_000, 1000 * 2 ** (this.#failures - 1));
			this.#retry = setTimeout(() => {
				void this.stillAllowed(scope)
					.catch(() => true)
					.then((listed) => (listed && this.#source === source ? this.#connect(scope) : this.#setState('offline')));
			}, delay);
		};
		source.addEventListener('changed', (event) => {
			const notice = JSON.parse((event as MessageEvent<string>).data) as Wire['ChangeNotice'];
			this.#emit(channelOf(notice.topic, scope));
		});
	}

	#emit(channel: ChangedChannel) {
		const signal = { channel, at: new Date().toISOString() };
		for (const handler of this.#handlers) handler(signal);
	}

	#setState(state: ConnectionState) {
		this.#state = state;
		for (const handler of this.#stateHandlers) handler(state);
	}
}

/** Server topics name the server location; the UI's channels name the UI location. */
export function channelOf(topic: string, scope: Scope): ChangedChannel {
	if (topic.startsWith('snapshot:')) return `snapshot:${scope.locationId}`;
	if (topic.startsWith('node_status:')) return `node_status:${scope.locationId}`;
	return topic === 'runs' || topic === 'ticks' ? topic : 'all';
}

export type HttpClientOptions = {
	baseUrl?: string;
	/** Called when the session is missing or rejected, before the request fails. */
	onUnauthorized: () => void;
};

/**
 * The server-backed client. Methods without an endpoint throw, and `supports`
 * reports them so the shell never shows a page it cannot load.
 */
export function createHttpClient({ baseUrl = '', onUnauthorized }: HttpClientOptions): ApiClient {
	const http = createFetch<paths>({ baseUrl, credentials: 'same-origin' });
	// Locations whose bootstrap actions include view.sql, which may hold on only some nodes, and the
	// nodes there whose SQL was refused; each refused read is audited, so neither is asked again until reload.
	let sqlLocations = new Set<string>();
	const sqlRefused = new Set<string>();
	// The same for run output, which needs view.evidence on every node the run selected.
	let evidenceLocations = new Set<string>();
	const outputRefused = new Set<string>();
	// A node's error text needs view.evidence on that node alone.
	const errorRefused = new Set<string>();
	const implemented = {
		supports: (method: keyof ApiClient) => SUPPORTED.has(method),
		changes: new HttpChangeFeed(baseUrl, (scope) => feedAllowed(baseUrl, scope, onUnauthorized)),
		async bootstrap(): Promise<Bootstrap> {
			const { data, error, response } = await http.GET('/api/control/bootstrap');
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			const bootstrap = toBootstrap(data);
			sqlLocations = new Set(bootstrap.locations.filter((l) => l.capabilities?.includes('view.sql')).map((l) => l.id));
			evidenceLocations = new Set(bootstrap.locations.filter((l) => l.capabilities?.includes('view.evidence')).map((l) => l.id));
			return bootstrap;
		},
		listRunRows: async (scope: Scope, query: RunRowQuery = {}): Promise<RunRowPage> => runRowPage(await listedRuns(scope), scope.locationId, query),
		runTabCounts: async (scope: Scope, query: RunRowQuery = {}): Promise<RunTabCounts> => runTabCountsOf(await listedRuns(scope), scope.locationId, query),
		getRun: async (scope: Scope, runId: string): Promise<Run | null> => (await findRun(scope, runId))?.run ?? null,
		async getRunDetail(scope: Scope, runId: string): Promise<RunDetail | null> {
			const found = await findRun(scope, runId);
			if (!found) return null;
			const { run, wire, listed } = found;
			const owners = [run, ...listed.filter((r) => r.parentRunId === run.id)];
			const nodes = (await Promise.all(owners.map(async (owner) => (await runNodes(scope, owner.id)).map((n) => toRunNode(n, owner))))).flat();
			return runDetailOf(run, wire, listed, nodes);
		},
		async listRunEvents(scope: Scope, runId: string, query: { after?: number; limit?: number } = {}): Promise<RunEventPage> {
			const limit = query.limit ?? 1000;
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/events', {
				params: { path: runPath(scope, runId), query: { after: query.after, limit } }
			});
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return { events: data.flatMap((e) => toRunEvent(e, runId) ?? []), nextCursor: data.length === limit ? (data.at(-1)?.storage_id ?? null) : null };
		},
		async listRunStatements(scope: Scope, runId: string): Promise<RunStatement[]> {
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/statements', { params: { path: runPath(scope, runId) } });
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toRunStatements(data, runId);
		},
		async getStatementSql(scope: Scope, runId: string, statementId: string): Promise<StatementSql> {
			const refusal = `${scope.locationId}|${runId}`;
			if (!sqlLocations.has(scope.locationId) || sqlRefused.has(refusal)) throw new ApiError('You may not view the SQL of this run', 403);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/statements/sql', {
				params: { path: runPath(scope, runId), query: { statement_id: statementId } }
			});
			if (response.status === 401) onUnauthorized();
			if (response.status === 403) sqlRefused.add(refusal);
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return { statementId: data.statement_id, sql: data.sql ?? null };
		},
		async listRunOutput(scope: Scope, runId: string): Promise<RunOutputLine[]> {
			const refusal = `${scope.locationId}|${runId}`;
			if (!evidenceLocations.has(scope.locationId) || outputRefused.has(refusal)) return [];
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output', { params: { path: runPath(scope, runId) } });
			if (response.status === 401) onUnauthorized();
			if (response.status === 403) outputRefused.add(refusal);
			if (response.status === 403) return [];
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toRunOutput(data);
		},
		async getRunNodeError(scope: Scope, runId: string, nodeKey: string): Promise<RunNodeError> {
			const refusal = `${scope.locationId}|${runId}|${nodeKey}`;
			if (!evidenceLocations.has(scope.locationId) || errorRefused.has(refusal)) throw new ApiError("You may not view this node's error text", 403);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes/{node_key}/error', {
				params: { path: { ...runPath(scope, runId), node_key: nodeKey } }
			});
			if (response.status === 401) onUnauthorized();
			if (response.status === 403) errorRefused.add(refusal);
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return { nodeKey: data.node_key, errorCode: data.error_code ?? null, message: data.message ?? null, file: data.path ?? null };
		},
		async cancelRun(scope: Scope, runId: string): Promise<Run> {
			const body = { run_id: runId, event_id: crypto.randomUUID(), reason: 'Cancelled from the web UI' };
			const { error, response } = await http.POST('/api/control/runs/cancel', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
			inFlight.clear();
			if (response.status === 401) onUnauthorized();
			if (!response.ok) throw new ApiError(errorText(error, response), response.status);
			return requireRun(scope, runId);
		},
		async rerunRun(scope: Scope, runId: string, mode: Exclude<RerunMode, 'selected'>): Promise<Run> {
			const source = await requireRun(scope, runId);
			const body = { run_id: crypto.randomUUID(), source_run_id: runId, from_failure: mode === 'from_failure', priority: source.priority, reason: 'Rerun from the web UI' };
			const { error, response } = await http.POST('/api/control/runs/rerun', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
			inFlight.clear();
			if (response.status === 401) onUnauthorized();
			if (!response.ok) throw new ApiError(errorText(error, response), response.status);
			return requireRun(scope, body.run_id);
		},
		listRuns: async (scope: Scope, query: RunListQuery = {}): Promise<Run[]> => runsMatching(toRuns(await listedRuns(scope), scope.locationId), query),
		async listJobs(scope: Scope): Promise<Job[]> {
			const [jobs, triggers, runs] = await Promise.all([listedJobs(scope), triggersOf(scope), recentRuns(scope)]);
			return jobs.map((job) => toJob(job, triggers, runs));
		},
		async getJob(scope: Scope, key: string): Promise<JobDetail | null> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const [read, triggers, runs] = await Promise.all([
				http.GET('/api/control/locations/{location_id}/targets/{target}/jobs/{job_key}', { params: { path: { location_id: locationId, target, job_key: key } } }),
				triggersOf(scope),
				recentRuns(scope)
			]);
			const { data, error, response } = read;
			if (response.status === 401) onUnauthorized();
			if (response.status === 404) return null;
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return jobDetailOf(toJob(data, triggers, runs), data);
		},
		/** A window makes it a backfill, which the server also checks against `run.rewrite_history`. */
		launchJob: (scope: Scope, key: string, options?: JobLaunchOptions): Promise<Run> => startJob(scope, key, options, false),
		/** The server checks a full refresh against `run.rewrite_history` and refuses it with a window. */
		launchJobFromScratch: (scope: Scope, key: string, options?: JobLaunchOptions): Promise<Run> => startJob(scope, key, options, true),
		async listCatalogNodes(scope: Scope): Promise<CatalogNode[]> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/nodes', { params: { path: { location_id: locationId, target } } });
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toCatalogNodes(data);
		},
		async listSnapshotNodes(scope: Scope): Promise<SnapshotNode[]> {
			const nodes = await implemented.listCatalogNodes(scope);
			return nodes.map((n) => ({ key: n.key, kind: n.kind, group: n.group ?? '', tags: n.tags, path: n.path, cursor: null, upstream: n.upstream }));
		},
		async checkAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<AdhocRunCheck> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const body = { location_id: locationId, target, command: request.command, priority: request.priority };
			const { data, error, response } = await http.POST('/api/control/runs/adhoc/check', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toAdhocCheck(data);
		},
		async launchAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<Run> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const body = { run_id: crypto.randomUUID(), location_id: locationId, target, command: request.command, priority: request.priority, reason: 'Started from the web UI' };
			const { error, response } = await http.POST('/api/control/runs/adhoc', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
			inFlight.clear();
			if (response.status === 401) onUnauthorized();
			if (!response.ok) throw new ApiError(errorText(error, response), response.status);
			return requireRun(scope, body.run_id);
		},
		async listSnapshotChanges(scope: Scope, query: { limit?: number } = {}): Promise<SnapshotChange[]> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const [snapshots, runs] = await Promise.all([
				http.GET('/api/control/locations/{location_id}/targets/{target}/snapshots', { params: { path: { location_id: locationId, target }, query: { limit: query.limit } } }),
				recentRuns(scope)
			]);
			if (snapshots.response.status === 401) onUnauthorized();
			if (!snapshots.data) throw new ApiError(errorText(snapshots.error, snapshots.response), snapshots.response.status);
			return toSnapshotChanges(snapshots.data, runs);
		},
		/** Newest first, reading at most `AUDIT_PAGES` server pages to find `limit` matching entries. */
		async listAuditEntries(query: AuditQuery = {}): Promise<AuditEntry[]> {
			const limit = query.limit ?? 100;
			const found: AuditEntry[] = [];
			let before = Number.MAX_SAFE_INTEGER;
			for (let page = 0; page < AUDIT_PAGES && found.length < limit; page++) {
				const { data, error, response } = await http.GET('/api/control/audit', { params: { query: { before, limit: AUDIT_PAGE } } });
				if (response.status === 401) onUnauthorized();
				if (!data) throw new ApiError(errorText(error, response), response.status);
				found.push(...data.map(toAuditEntry).filter((entry) => auditMatches(entry, query)));
				if (data.length < AUDIT_PAGE) break;
				before = data[data.length - 1].storage_id;
			}
			return found.slice(0, limit);
		},
		async getWarehouseSync(scope: Scope): Promise<WarehouseSync | null> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/health', { params: { path: { location_id: locationId, target } } });
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return warehouseSyncOf(data);
		},
		/** Asks the location to connect, then waits for its answer until the request expires. */
		async testConnection(scope: Scope): Promise<ConnectionTest> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const path = { location_id: locationId, target };
			const requestId = crypto.randomUUID();
			const asked = await http.POST('/api/control/locations/{location_id}/targets/{target}/connection-tests', {
				body: { request_id: requestId, reason: 'Tested from the web UI' },
				params: { path, header: { 'x-sqb-csrf': csrfToken(document.cookie) } }
			});
			if (asked.response.status === 401) onUnauthorized();
			if (!asked.response.ok) throw new ApiError(errorText(asked.error, asked.response), asked.response.status);
			let expiresAt = Date.now() + CONNECTION_TEST_WAIT_MS;
			while (Date.now() < expiresAt) {
				await new Promise((resolve) => setTimeout(resolve, CONNECTION_TEST_POLL_MS));
				const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/connection-tests/{request_id}', {
					params: { path: { ...path, request_id: requestId } }
				});
				if (response.status === 401) onUnauthorized();
				if (!response.ok) throw new ApiError(errorText(error, response), response.status);
				const finished = manualConnectionTest(data ?? null);
				if (finished) return finished;
				if (data) expiresAt = Math.min(expiresAt, Date.parse(data.expires_at));
			}
			throw new ApiError('The location did not answer the connection test in time; check that it is running', 504);
		},
		async maintenanceHistory(scope: Scope): Promise<MaintenanceHistory> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/maintenance', { params: { path: { location_id: locationId, target } } });
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toMaintenanceHistory(data);
		},
		listPlatformEvents: async (): Promise<PlatformEvent[]> => toPlatformEvents(await problems(), Date.now()),
		platformHealth: async (): Promise<PlatformHealth> => toPlatformHealth(await problems()),
		/** Each location the person sees, with health only where they may list the whole target. */
		async listLocations(): Promise<LocationSummary[]> {
			const [locations, events] = await Promise.all([bootstrapLocations(), problems().then((p) => toPlatformEvents(p, Date.now()))]);
			return Promise.all(
				locations.map(async (location) => {
					const [health, snapshot] = await locationHealth(location.id);
					const open = events.filter((e) => e.locationId === location.id).length;
					return toLocationSummary(location, health.data ?? null, snapshot, open, Date.now());
				})
			);
		},
		/** Null for a location the person can't see; a refused health read fails the page, as the target's listings do. */
		async getLocation(locationId: string): Promise<LocationDetail | null> {
			const location = (await bootstrapLocations()).find((l) => l.id === locationId);
			if (!location) return null;
			const [health, snapshot] = await locationHealth(location.id);
			if (!health.data) throw new ApiError(errorText(health.error, health.response), health.response.status);
			return toLocationDetail(location, health.data, snapshot, Date.now());
		},
		listAccounts: async (): Promise<Account[]> => (await people()).map(toAccount),
		listRoles: async (locationId: string): Promise<RoleDefinition[]> => (await roleOverviews(locationId)).map(toRoleDefinition),
		async getRole(locationId: string, name: string): Promise<RoleDetail | null> {
			const [roles, assignments] = await Promise.all([roleOverviews(locationId), implemented.listRoleAssignments()]);
			const role = roles.find((r) => r.key === name);
			return role ? roleDetailOf(role, assignments, locationId) : null;
		},
		async listRoleAssignments(): Promise<RoleAssignment[]> {
			// Group members come from the people read; without it, SSO-derived roles stay unlisted.
			return toRoleAssignments(await roleHoldings(), await people().catch(() => []));
		},
		async createRoleAssignment(request: RoleAssignmentRequest): Promise<RoleAssignment> {
			const person = await personNamed(request.principal);
			const { locationId, target } = request.scope.locationId ? parseLocationKey(request.scope.locationId) : { locationId: null, target: null };
			await changeRole({ principal_id: person.principal_id, role: request.role, location_id: locationId, target, revoke: false });
			// The server answers only whether it changed anything; an assignment that already existed is found too.
			const given = (await implemented.listRoleAssignments()).find(
				(a) => a.source === 'manual' && a.principal.name === request.principal && a.role === request.role && a.scope.locationId === request.scope.locationId
			);
			if (!given) throw new ApiError(`${request.role} was not given to ${request.principal}`, 409);
			return given;
		},
		async deleteRoleAssignment(assignmentId: string): Promise<void> {
			const held = (await roleHoldings()).assignments.find((a) => a.assignment_id === assignmentId);
			if (!held) throw new ApiError('That role assignment no longer exists', 404);
			await changeRole({ principal_id: held.principal_id, role: held.role_key, location_id: held.location_id ?? null, target: held.target ?? null, revoke: true });
		},
		disableAccount: (name: string): Promise<Account> => changePerson(name, 'disable'),
		enableAccount: (name: string): Promise<Account> => changePerson(name, 'enable'),
		async getColumnLineage(scope: Scope, ref: string): Promise<ColumnLineage | null> {
			const nodes = await implemented.listCatalogNodes(scope);
			const node = findNode(nodes, ref);
			if (!node) return null;
			const { locationId, target } = parseLocationKey(scope.locationId);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/column-lineage', {
				params: { path: { location_id: locationId, target }, query: { node_key: node.key } }
			});
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toColumnLineage(data, nodes);
		},
		async getCatalogNode(scope: Scope, ref: string): Promise<CatalogNodeDetail | null> {
			const nodes = await implemented.listCatalogNodes(scope);
			const node = findNode(nodes, ref);
			if (!node) return null;
			const { locationId, target } = parseLocationKey(scope.locationId);
			const path = { location_id: locationId, target };
			const [status, sql, checks, runs, jobs] = await Promise.all([
				http.GET('/api/control/locations/{location_id}/targets/{target}/node-status', { params: { path, query: { node_key: node.key } } }),
				sqlLocations.has(scope.locationId) && !sqlRefused.has(`${scope.locationId}|${node.key}`)
					? http.GET('/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/sql', { params: { path: { ...path, node_key: node.key } } })
					: null,
				checkResults(scope).catch(() => []),
				node.kind === 'source'
					? null
					: http.GET('/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/runs', { params: { path: { ...path, node_key: node.key }, query: { limit: 100 } } }),
				Promise.all([listedJobs(scope), listedTriggers(scope)]).then(([j, t]) => nodeJobsOf(node.key, j, t)).catch(() => null)
			]);
			if (status.response.status === 401 || sql?.response.status === 401 || runs?.response.status === 401) onUnauthorized();
			if (sql?.response.status === 403) sqlRefused.add(`${scope.locationId}|${node.key}`);
			// Status and SQL have their own permissions; a refusal leaves that part out rather than failing the page.
			return nodeDetailOf(nodes, node, status.data ?? null, sql?.data ?? null, checks.map(toCheck), { runs: runs?.data ? toNodeRuns(runs.data) : null, jobs });
		},
		async overview(scope: Scope): Promise<Overview | null> {
			// A week of publications for the models metric; the server returns at most 100.
			const reads = Promise.all([
				recentRuns(scope),
				implemented.listChecks(scope),
				implemented.listCatalogNodes(scope),
				implemented.listSnapshotChanges(scope, { limit: 100 }),
				implemented.listPlatformEvents()
			]);
			// Every listing needs view on all of the target, so a partial viewer gets no counts at all.
			const read = await reads.catch((error: unknown) => {
				if (error instanceof ApiError && error.status === 403) return null;
				throw error;
			});
			if (!read) return null;
			const [runs, checks, nodes, changes, events] = read;
			return overviewOf({ runs, checks, nodes, changes, events, now: Date.now() });
		},
		listChecks: async (scope: Scope): Promise<Check[]> => (await checkResults(scope)).map(toCheck),
		getCheck: async (scope: Scope, id: string): Promise<CheckDetail | null> => checkDetailOf(await checkResults(scope, id)),
		async getCheckSql(scope: Scope, checkId: string): Promise<CheckSql> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/sql', {
				params: { path: { location_id: locationId, target, binding_key: checkId } }
			});
			if (response.status === 401) onUnauthorized();
			if (!response.ok) throw new ApiError(errorText(error, response), response.status);
			if (!data) throw new ApiError('The check has no stored result yet', 404);
			return toCheckSql(data);
		},
		async getCheckEvidence(scope: Scope, checkId: string, evaluationId: string): Promise<CheckEvidence> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/results/{event_id}/evidence', {
				params: { path: { location_id: locationId, target, binding_key: checkId, event_id: evaluationId } }
			});
			if (response.status === 401) onUnauthorized();
			if (!response.ok) throw new ApiError(errorText(error, response), response.status);
			if (!data) throw new ApiError('No evidence was stored with this result', 404);
			return toCheckEvidence(data);
		},
		listTriggers: (scope: Scope): Promise<Trigger[]> => triggersOf(scope),
		async getTrigger(scope: Scope, id: string): Promise<TriggerDetail | null> {
			const [wires, runs] = await Promise.all([listedTriggers(scope), recentRuns(scope)]);
			const wire = wires.find((t) => t.trigger_id === id);
			if (!wire) return null;
			const ticks = (await ticksOf(scope, id, 100)).map(toTick);
			return triggerDetailOf(toTrigger(wire, runs, ticks[0] ?? null), wire, ticks);
		},
		async setTriggerStates(scope: Scope, ids: string[], state: TriggerState, reason?: string | null): Promise<Trigger[]> {
			for (const [done, id] of ids.entries()) {
				try {
					await changeTrigger(id, { kind: state === 'running' ? 'enable' : 'pause' }, reason);
				} catch (error) {
					if (done === 0 || !(error instanceof ApiError)) throw error;
					throw new ApiError(`${error.message} (${done} of ${ids.length} triggers were already changed)`, error.status);
				}
			}
			return (await triggersOf(scope)).filter((t) => ids.includes(t.id));
		},
		async resetTriggerToDefault(scope: Scope, id: string): Promise<Trigger> {
			await changeTrigger(id, { kind: 'reset_default' });
			return requireTrigger(scope, id);
		},
		async resetSensorCursor(scope: Scope, id: string, reason?: string | null): Promise<TriggerDetail> {
			await changeTrigger(id, { kind: 'reset_cursor' }, reason);
			const detail = await implemented.getTrigger(scope, id);
			if (!detail) throw new ApiError('The trigger is no longer published', 404);
			return detail;
		}
	};

	function runPath(scope: Scope, runId: string) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		return { location_id: locationId, target, run_id: runId };
	}

	/**
	 * One request per listing at a time: pages that refresh together on a change notice share it
	 * instead of each asking the server again. A write clears it, so reads after a write see it.
	 */
	const inFlight = new Map<string, Promise<unknown>>();
	function shared<T>(key: string, load: () => Promise<T>): Promise<T> {
		const pending = inFlight.get(key) as Promise<T> | undefined;
		if (pending) return pending;
		const started: Promise<T> = load().finally(() => inFlight.get(key) === started && inFlight.delete(key));
		inFlight.set(key, started);
		return started;
	}

	const listedJobs = (scope: Scope) => shared(`jobs ${scope.locationId}`, () => fetchJobs(scope));

	async function fetchJobs(scope: Scope) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/jobs', { params: { path: { location_id: locationId, target } } });
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	const listedTriggers = (scope: Scope) => shared(`triggers ${scope.locationId}`, () => fetchTriggers(scope));

	/** Published triggers, each with its newest tick. */
	async function fetchTriggers(scope: Scope) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/triggers', { params: { path: { location_id: locationId, target } } });
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function ticksOf(scope: Scope, triggerId: string, limit: number) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks', {
			params: { path: { location_id: locationId, target, trigger_id: triggerId }, query: { limit } }
		});
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function roleOverviews(locationKey: string) {
		const { locationId, target } = parseLocationKey(locationKey);
		const { data, error, response } = await http.GET('/api/admin/locations/{location_id}/targets/{target}/roles', { params: { path: { location_id: locationId, target } } });
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function roleHoldings() {
		const { data, error, response } = await http.GET('/api/admin/roles');
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function changeRole(body: Wire['RoleAssignmentRequest']) {
		const { error, response } = await http.POST('/api/admin/roles', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
		if (response.status === 401) onUnauthorized();
		if (!response.ok) throw new ApiError(errorText(error, response), response.status);
	}

	async function personNamed(name: string) {
		const person = (await people()).find((p) => p.name === name);
		if (!person) throw new ApiError(`Unknown person ${name}`, 404);
		return person;
	}

	/** The server refuses to disable the last enabled installation administrator. */
	async function changePerson(name: string, change: 'disable' | 'enable'): Promise<Account> {
		const person = await personNamed(name);
		const { error, response } = await http.POST('/api/admin/people/{principal_id}', {
			body: change,
			params: { path: { principal_id: person.principal_id }, header: { 'x-sqb-csrf': csrfToken(document.cookie) } }
		});
		if (response.status === 401) onUnauthorized();
		if (!response.ok) throw new ApiError(errorText(error, response), response.status);
		return toAccount(await personNamed(name));
	}

	async function people() {
		const { data, error, response } = await http.GET('/api/admin/people');
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function problems() {
		const { data, error, response } = await http.GET('/api/control/platform/problems');
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function bootstrapLocations() {
		const { data, error, response } = await http.GET('/api/control/bootstrap');
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return toBootstrap(data).locations;
	}

	/** The target's health read, kept whole so a refusal can be shown or left out, and its newest snapshot. */
	async function locationHealth(locationKey: string) {
		const { locationId, target } = parseLocationKey(locationKey);
		const path = { location_id: locationId, target };
		const [health, snapshots] = await Promise.all([
			http.GET('/api/control/locations/{location_id}/targets/{target}/health', { params: { path } }),
			http.GET('/api/control/locations/{location_id}/targets/{target}/snapshots', { params: { path, query: { limit: 1 } } })
		]);
		if (health.response.status === 401 || snapshots.response.status === 401) onUnauthorized();
		return [health, snapshots.data?.[0] ?? null] as const;
	}

	/** The newest result of every check, or one check's results newest first. */
	async function checkResults(scope: Scope, bindingKey?: string) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const { data, error, response } = bindingKey
			? await http.GET('/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}', {
					params: { path: { location_id: locationId, target, binding_key: bindingKey } }
				})
			: await http.GET('/api/control/locations/{location_id}/targets/{target}/checks', { params: { path: { location_id: locationId, target } } });
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	const recentRuns = async (scope: Scope) => toRuns(await listedRuns(scope), scope.locationId);

	/** Triggers with their latest tick and the newest listed run each started. */
	async function triggersOf(scope: Scope): Promise<Trigger[]> {
		const [wires, runs] = await Promise.all([listedTriggers(scope), recentRuns(scope)]);
		return wires.map((t) => toTrigger(t, runs, t.latest_tick ? toTick(t.latest_tick) : null));
	}

	async function requireTrigger(scope: Scope, id: string): Promise<Trigger> {
		const trigger = (await triggersOf(scope)).find((t) => t.id === id);
		if (!trigger) throw new ApiError('The trigger is no longer published', 404);
		return trigger;
	}

	/** Records an operator change; the server takes the actor from the session and needs a reason. */
	async function changeTrigger(triggerId: string, change: Wire['TriggerChange'], reason?: string | null) {
		const body = { trigger_id: triggerId, event_id: crypto.randomUUID(), actor: '', reason: reason?.trim() || 'Changed from the web UI', change };
		const { error, response } = await http.POST('/api/control/triggers', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
		inFlight.clear();
		if (response.status === 401) onUnauthorized();
		if (!response.ok) throw new ApiError(errorText(error, response), response.status);
	}

	const listedRuns = (scope: Scope) => shared(`runs ${scope.locationId}`, () => fetchRuns(scope));

	/** The newest runs of the location, up to the server's cap. */
	async function fetchRuns(scope: Scope) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/runs', {
			params: { path: { location_id: locationId, target }, query: { limit: RUN_WINDOW } }
		});
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function findRun(scope: Scope, runId: string) {
		const wires = await listedRuns(scope);
		const listed = toRuns(wires, scope.locationId);
		const index = listed.findIndex((r) => r.id === runId);
		return index < 0 ? null : { run: listed[index], wire: wires[index], listed };
	}

	/** A window makes it a backfill; the server checks either against `run.rewrite_history` and refuses both together. */
	async function startJob(scope: Scope, key: string, options: JobLaunchOptions | undefined, fullRefresh: boolean): Promise<Run> {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const window = options?.window ? runWindowOf(options.window) : undefined;
		const reason = fullRefresh ? 'Full refresh started from the web UI' : window ? 'Backfill started from the web UI' : 'Started from the web UI';
		const body = { run_id: crypto.randomUUID(), location_id: locationId, target, job_key: key, priority: 0, reason, window, full_refresh: fullRefresh || undefined };
		const { error, response } = await http.POST('/api/control/runs', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
		inFlight.clear();
		if (response.status === 401) onUnauthorized();
		if (!response.ok) throw new ApiError(errorText(error, response), response.status);
		return requireRun(scope, body.run_id);
	}

	async function requireRun(scope: Scope, runId: string): Promise<Run> {
		const found = await findRun(scope, runId);
		if (!found) throw new ApiError('The run is not in the recent runs of this location', 404);
		return found.run;
	}

	async function runNodes(scope: Scope, runId: string) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes', {
			params: { path: { location_id: locationId, target, run_id: runId } }
		});
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	return new Proxy(implemented, {
		get(target, property, receiver) {
			if (property in target) return Reflect.get(target, property, receiver);
			return () => Promise.reject(new ApiError(`${String(property)} is not available on this server yet`, 501));
		}
	}) as unknown as ApiClient;
}

/** The server's message: a JSON error body's text, or a 4xx plain-text body such as a request the route couldn't read; never an HTML page. */
function errorText(error: unknown, response: Response): string {
	// Plain text comes from the server's request rejections; a 5xx one can name internal types, so it shows as the status.
	if (typeof error === 'string' && error.trim() && response.status < 500 && response.headers.get('content-type')?.startsWith('text/plain')) return error.trim();
	const message = (error as { message?: unknown; error?: unknown } | undefined)?.message ?? (error as { error?: unknown } | undefined)?.error;
	return typeof message === 'string' ? message : `${response.status} ${response.statusText}`;
}

/** The CSRF token the server sets in a readable cookie at sign-in; writes send it back. */
export function csrfToken(cookies: string): string {
	for (const cookie of cookies.split(';')) {
		const [name, ...value] = cookie.trim().split('=');
		if (name === '__Host-sqb-csrf') return value.join('=');
	}
	return '';
}

/** Signs in with a password; resolves to whether the server accepted it. */
export async function loginWithPassword(username: string, password: string, baseUrl = ''): Promise<'ok' | 'rejected' | 'throttled' | 'unavailable'> {
	const http = createFetch<paths>({ baseUrl, credentials: 'same-origin' });
	const { response } = await http.POST('/api/auth/password/login', { body: { username, password } });
	if (response.ok) return 'ok';
	if (response.status === 429) return 'throttled';
	return response.status === 401 ? 'rejected' : 'unavailable';
}

export async function logout(baseUrl = ''): Promise<void> {
	const http = createFetch<paths>({ baseUrl, credentials: 'same-origin' });
	await http.POST('/api/auth/logout', { params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
}
