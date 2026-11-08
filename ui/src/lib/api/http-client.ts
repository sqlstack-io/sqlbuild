import createFetch from 'openapi-fetch';
import type { components, paths } from '$control/schema.d.ts';
import { ApiError, type ApiClient } from './client';
import { checkDetailOf, toCheck, toCheckEvidence, toCheckSql } from './http-checks';
import { toCatalogNodes } from './http-catalog';
import { auditMatches, toAuditEntry } from './http-audit';
import { toSnapshotChanges } from './http-changes';
import { toLocationSummary, warehouseSyncOf } from './http-locations';
import { toAccount, toRoleAssignments } from './http-people';
import { toPlatformEvents } from './http-problems';
import { roleDetailOf, toRoleDefinition } from './http-roles';
import { jobDetailOf, toJob, toTick, toTrigger, triggerDetailOf } from './http-jobs';
import { RUN_WINDOW, runDetailOf, runRowPage, runsMatching, runTabCountsOf, toRunNode, toRuns } from './http-runs';
import type { ChangeSubscription, ChangedChannel, ChangedSignal, ConnectionState } from './signals';
import type { AuthMode, Bootstrap, Capability, Location, PlatformEvent, Scope, WarehouseSync } from './types';
import type { Account, AuditEntry, AuditQuery, LocationSummary, RoleAssignment, RoleDefinition, RoleDetail } from './types/platform';
import type { CatalogNode, Check, CheckDetail, CheckEvidence, CheckSql, SnapshotChange } from './types/build';
import type { Job, JobDetail, RerunMode, Run, RunDetail, RunListQuery, RunRowPage, RunRowQuery, RunTabCounts, Trigger, TriggerDetail, TriggerState } from './types/operate';

type Wire = components['schemas'];

/** The client methods the server has endpoints for; the shell hides pages that need others. */
const SUPPORTED: ReadonlySet<string> = new Set<keyof ApiClient>([
	'supports',
	'bootstrap',
	'changes',
	'listRunRows',
	'runTabCounts',
	'listRuns',
	'getRun',
	'getRunDetail',
	'cancelRun',
	'rerunRun',
	'listJobs',
	'getJob',
	'launchJob',
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
	'listSnapshotChanges',
	'listAuditEntries',
	'listAccounts',
	'listRoleAssignments',
	'listLocations',
	'listPlatformEvents',
	'listRoles',
	'getRole',
	'getWarehouseSync'
]);

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
	const implemented = {
		supports: (method: keyof ApiClient) => SUPPORTED.has(method),
		changes: new HttpChangeFeed(baseUrl, (scope) => feedAllowed(baseUrl, scope, onUnauthorized)),
		async bootstrap(): Promise<Bootstrap> {
			const { data, error, response } = await http.GET('/api/control/bootstrap');
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toBootstrap(data);
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
		async cancelRun(scope: Scope, runId: string): Promise<Run> {
			const body = { run_id: runId, event_id: crypto.randomUUID(), reason: 'Cancelled from the web UI' };
			const { error, response } = await http.POST('/api/control/runs/cancel', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
			if (response.status === 401) onUnauthorized();
			if (!response.ok) throw new ApiError(errorText(error, response), response.status);
			return requireRun(scope, runId);
		},
		async rerunRun(scope: Scope, runId: string, mode: Exclude<RerunMode, 'selected'>): Promise<Run> {
			const source = await requireRun(scope, runId);
			const body = { run_id: crypto.randomUUID(), source_run_id: runId, from_failure: mode === 'from_failure', priority: source.priority, reason: 'Rerun from the web UI' };
			const { error, response } = await http.POST('/api/control/runs/rerun', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
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
			const [jobs, triggers, runs] = await Promise.all([listedJobs(scope), triggersOf(scope), recentRuns(scope)]);
			const wire = jobs.find((job) => job.job_key === key);
			return wire ? jobDetailOf(toJob(wire, triggers, runs), wire) : null;
		},
		async launchJob(scope: Scope, key: string): Promise<Run> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const body = { run_id: crypto.randomUUID(), location_id: locationId, target, job_key: key, priority: 0, reason: 'Started from the web UI' };
			const { error, response } = await http.POST('/api/control/runs', { body, params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
			if (response.status === 401) onUnauthorized();
			if (!response.ok) throw new ApiError(errorText(error, response), response.status);
			return requireRun(scope, body.run_id);
		},
		async listCatalogNodes(scope: Scope): Promise<CatalogNode[]> {
			const { locationId, target } = parseLocationKey(scope.locationId);
			const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/nodes', { params: { path: { location_id: locationId, target } } });
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toCatalogNodes(data);
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
		async listPlatformEvents(): Promise<PlatformEvent[]> {
			const { data, error, response } = await http.GET('/api/control/platform/problems');
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toPlatformEvents(data, Date.now());
		},
		/** Each location the person sees, with health only where they may list the whole target. */
		async listLocations(): Promise<LocationSummary[]> {
			const { data, error, response } = await http.GET('/api/control/bootstrap');
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return Promise.all(
				toBootstrap(data).locations.map(async (location) => {
					const { locationId, target } = parseLocationKey(location.id);
					const path = { location_id: locationId, target };
					const [health, snapshots] = await Promise.all([
						http.GET('/api/control/locations/{location_id}/targets/{target}/health', { params: { path } }),
						http.GET('/api/control/locations/{location_id}/targets/{target}/snapshots', { params: { path, query: { limit: 1 } } })
					]);
					if (health.response.status === 401 || snapshots.response.status === 401) onUnauthorized();
					return toLocationSummary(location, health.data ?? null, snapshots.data?.[0] ?? null, Date.now());
				})
			);
		},
		listAccounts: async (): Promise<Account[]> => (await people()).map(toAccount),
		listRoles: async (locationId: string): Promise<RoleDefinition[]> => (await roleOverviews(locationId)).map(toRoleDefinition),
		async getRole(locationId: string, name: string): Promise<RoleDetail | null> {
			const [roles, assignments] = await Promise.all([roleOverviews(locationId), implemented.listRoleAssignments()]);
			const role = roles.find((r) => r.key === name);
			return role ? roleDetailOf(role, assignments, locationId) : null;
		},
		async listRoleAssignments(): Promise<RoleAssignment[]> {
			const { data, error, response } = await http.GET('/api/admin/roles');
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			// Group members come from the people read; without it, SSO-derived roles stay unlisted.
			return toRoleAssignments(data, await people().catch(() => []));
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

	async function listedJobs(scope: Scope) {
		const { locationId, target } = parseLocationKey(scope.locationId);
		const { data, error, response } = await http.GET('/api/control/locations/{location_id}/targets/{target}/jobs', { params: { path: { location_id: locationId, target } } });
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
	}

	async function listedTriggers(scope: Scope) {
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

	async function people() {
		const { data, error, response } = await http.GET('/api/admin/people');
		if (response.status === 401) onUnauthorized();
		if (!data) throw new ApiError(errorText(error, response), response.status);
		return data;
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
		const latest = await Promise.all(wires.map(async (t) => (await ticksOf(scope, t.trigger_id, 1)).map(toTick)[0] ?? null));
		return wires.map((t, i) => toTrigger(t, runs, latest[i]));
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
		if (response.status === 401) onUnauthorized();
		if (!response.ok) throw new ApiError(errorText(error, response), response.status);
	}

	/** The newest runs of the location, up to the server's cap. */
	async function listedRuns(scope: Scope) {
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

function errorText(error: unknown, response: Response): string {
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
