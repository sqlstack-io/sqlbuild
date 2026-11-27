// `?mock&scenario=<name>`: the shop location's warehouse test, history sync, check-ins and the live
// feed in one of the states the sync-controls proposal covers, for clicking through on the real pages.
// The server reports none of "requested", "stalled" or "refused" yet; this client fakes them.

import { ApiError, type ApiClient } from '../client';
import { LocalChangeBus } from '../signals';
import type { ConnectionTest, LocationDetail, LocationSummary, NavCounts, Overview, PlatformEvent, RepublishResult, Scope, WarehouseSync } from '../types';
import type { PlatformEventRecord, WarehouseConnection } from '../types/platform';
import { LOCATION_SHOP } from './fixtures';

export const SYNC_SCENARIOS = ['healthy', 'synced-untested', 'requested', 'running', 'stalled', 'stopped', 'deploy-failed-fallback', 'failing', 'not-checking-in', 'deploy-failed', 'several', 'reconnecting', 'offline'] as const;
export type SyncScenario = (typeof SYNC_SCENARIOS)[number];

/** What each scenario shows, for the picker. */
export const SCENARIO_NOTES: Record<SyncScenario, string> = {
	healthy: 'Green: tested, synced, nothing open',
	'synced-untested': 'Amber: synced 2m ago, but the warehouse connection was never tested',
	requested: 'Sync now was clicked; the location hasn’t taken it yet',
	running: 'A sync is running, with progress and how long it has run',
	stalled: 'Amber: a sync stopped reporting progress 12m ago',
	stopped: 'Amber: failed reads (a permission) stopped the sync; Resume it from the Overview',
	'deploy-failed-fallback': 'Amber: the latest deploy failed to load; the previous version still runs',
	failing: 'Red: the warehouse refuses the location’s IP, so it can’t connect and the sync is blocked',
	'not-checking-in': 'Red: the location stopped checking in; every action fails at once, saying why',
	'deploy-failed': 'Red: the latest deploy failed and runs are held, with no version to fall back on',
	several: 'Red: waffle-shop can’t connect, and other locations have problems of their own',
	reconnecting: 'Live updates dropped and are retrying: Reconnect in the top bar',
	offline: 'Live updates gave up: Reload in the top bar'
};

const IP_BLOCKED = '390422 (08001): IP 203.0.113.7 is not allowed to access Snowflake. Contact your local security administrator.';
const NOT_CHECKING_IN = (what: string) => `The location isn't checking in (last 6m ago), so it can't ${what}. Start it, then try again.`;
const KINDS = ['catalog', 'node_results', 'audit_results', 'freshness', 'fingerprints', 'migrations', 'run_results', 'observations', 'statistics', 'consumption', 'resource_costs', 'microbatches', 'janitor_events', 'old_name_views'];

type State = { sync: WarehouseSync; test: ConnectionTest | null; live: boolean; events: PlatformEvent[]; deploy?: { fallback: boolean } };
const PERMISSION = 'SQL access control error: insufficient privileges to operate on schema ARCHIVE';
const LOAD_ERROR = "ImportError: cannot import name 'daily_revenue' from 'models.finance' (orchestration/jobs.py, line 8)";

const iso = (ms: number) => new Date(ms).toISOString();

function initial(scenario: SyncScenario, now: number): State {
	const synced = (minutes: number): WarehouseSync => ({
		lastSyncedAt: iso(now - minutes * 60_000),
		lastError: null,
		lastReportedAt: iso(now - minutes * 60_000),
		syncing: null,
		completed: { at: iso(now - minutes * 60_000), kinds: 14, rows: 2446, elapsedMs: 23_200 },
		failing: null,
		requested: null
	});
	const tested = (minutes: number, outcome: ConnectionTest['outcome'] = 'ok'): ConnectionTest => ({
		outcome,
		at: iso(now - minutes * 60_000),
		durationMs: outcome === 'ok' ? 820 : 30_000,
		trigger: 'manual',
		by: { kind: 'user', name: 'ada', displayName: 'Ada' },
		message: outcome === 'ok' ? 'Connected as TRANSFORMER to WAFFLE_SHOP; warehouse TRANSFORMING_WH available' : IP_BLOCKED
	});
	const event = (kind: PlatformEvent['kind'], severity: PlatformEvent['severity'], message: string, extra: Partial<PlatformEvent> = {}): PlatformEvent => ({
		id: `scenario-${kind}`,
		kind,
		severity,
		occurredAt: iso(now - 12 * 60_000),
		active: true,
		locationId: LOCATION_SHOP,
		subject: null,
		message,
		...extra
	});
	const deployFailed = (fallback: boolean): State => ({
		sync: synced(3),
		test: tested(1),
		live: true,
		deploy: { fallback },
		events: [
			event('location_failing', fallback ? 'warning' : 'critical', `waffle-shop · prod: latest code failed to load: ${LOAD_ERROR}. ${fallback ? 'Serving generation 43.' : 'Runs are held until a deploy loads.'}`, {
				id: 'scenario-deploy',
				state: fallback ? 'falling_back' : 'held',
				occurredAt: iso(now - 5 * 60_000)
			})
		]
	});
	const failing = (halted: boolean): State => ({
		sync: {
			...synced(26 * 60),
			lastError: `catalog: failed while reading: ${IP_BLOCKED}`,
			lastReportedAt: iso(now - 20_000),
			completed: null,
			failing: { since: iso(now - 25 * 3_600_000), reason: IP_BLOCKED, halted, unreachable: true, retrySeconds: halted ? null : 60, retryAt: halted ? null : iso(now + 40_000) }
		},
		test: tested(3, 'failed'),
		live: true,
		events: [
			event('warehouse_sync_failed', halted ? 'critical' : 'warning', halted ? `Run history sync stopped after failed reads in a row: ${IP_BLOCKED}` : `Run history can't reach the warehouse: ${IP_BLOCKED}`, {
				sync: { halted, unreachable: true, retrySeconds: halted ? null : 60, retryAt: halted ? null : iso(now + 40_000), lastSuccessAt: iso(now - 26 * 3_600_000) }
			}),
			event('connection_test_failed', 'critical', `The warehouse connection test failed: ${IP_BLOCKED}`, { id: 'scenario-connection-test', occurredAt: iso(now - 3 * 60_000) })
		]
	});
	switch (scenario) {
		case 'synced-untested':
			return { sync: synced(2), test: null, live: true, events: [] };
		case 'requested':
			return { sync: { ...synced(40), requested: { at: iso(now - 4_000), by: 'ada' } }, test: tested(1), live: true, events: [] };
		case 'running':
			return { sync: { ...synced(40), lastReportedAt: iso(now - 3_000), syncing: { position: 7, kinds: 14, table: 'run_results', rows: 1240, startedAt: iso(now - 2 * 60_000) } }, test: tested(1), live: true, events: [] };
		case 'stalled':
			return {
				sync: { ...synced(55), lastReportedAt: iso(now - 12 * 60_000), syncing: { position: 7, kinds: 14, table: 'run_results', rows: 1240, startedAt: iso(now - 14 * 60_000) } },
				test: tested(1),
				live: true,
				events: [event('warehouse_sync_stalled', 'warning', 'The run history sync stalled at 7/14 (run results): no progress for 12m. The location may have stopped; check its output, then Sync now.')]
			};
		case 'failing':
		case 'several':
			return failing(false);
		case 'stopped': {
			const stopped = failing(true);
			const reason = `audit_results: failed while reading: ${PERMISSION}`;
			return {
				...stopped,
				sync: { ...stopped.sync, lastError: reason, failing: stopped.sync.failing && { ...stopped.sync.failing, reason: PERMISSION, unreachable: false } },
				test: tested(2),
				events: stopped.events
					.filter((e) => e.kind !== 'connection_test_failed')
					.map((e) => ({ ...e, severity: 'warning' as const, message: `Run history sync stopped after failed reads in a row: ${PERMISSION}`, sync: e.sync && { ...e.sync, unreachable: false } }))
			};
		}
		case 'healthy':
			return { sync: synced(2), test: tested(1), live: true, events: [] };
		case 'deploy-failed-fallback':
			return deployFailed(true);
		case 'deploy-failed':
			return deployFailed(false);
		case 'not-checking-in':
			return { sync: synced(9), test: tested(2), live: false, events: [event('location_broken', 'critical', 'shop · prod stopped checking in, so its scheduled runs aren’t starting.', { occurredAt: iso(now - 6 * 60_000) })] };
		case 'reconnecting':
		case 'offline':
			return { sync: synced(4), test: tested(1), live: true, events: [] };
	}
}

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** The client with the shop location in `scenario`, changing as the person clicks Sync now, Test connection or Republish. */
export function syncScenario(client: ApiClient, scenario: SyncScenario): ApiClient {
	const bus = client.changes instanceof LocalChangeBus ? client.changes : null;
	const state = initial(scenario, Date.now());
	const shop = (scope: Scope | string) => (typeof scope === 'string' ? scope : scope.locationId) === LOCATION_SHOP;
	const announce = () => {
		bus?.emit(`warehouse_sync:${LOCATION_SHOP}`);
		bus?.emit(`location_health:${LOCATION_SHOP}`);
	};
	const lastSeen = () => iso(Date.now() - 6 * 60_000);

	if (bus && (scenario === 'reconnecting' || scenario === 'offline')) {
		// After the shell reads its first page, the feed drops: a moment of live, then the scenario.
		setTimeout(() => bus.setState(scenario), 1500);
	}

	/** A requested sync: taken after a moment, then running kind by kind, then done, or failing again while the IP is refused. */
	async function runSync(): Promise<void> {
		const sync = state.sync;
		sync.requested = { at: iso(Date.now()), by: 'ada' };
		announce();
		await delay(2500);
		sync.requested = null;
		if (sync.failing) {
			Object.assign(sync, { lastReportedAt: iso(Date.now()), failing: { ...sync.failing, halted: false, retrySeconds: 60, retryAt: iso(Date.now() + 60_000) } });
			return announce();
		}
		const startedAt = iso(Date.now());
		for (let position = 1; position <= 14; position++) {
			Object.assign(sync, { lastReportedAt: iso(Date.now()), syncing: { position, kinds: 14, table: KINDS[position - 1], rows: position * 175, startedAt } });
			announce();
			await delay(700);
		}
		const now = Date.now();
		Object.assign(sync, { syncing: null, lastSyncedAt: iso(now), lastReportedAt: iso(now), lastError: null, completed: { at: iso(now), kinds: 14, rows: 2446, elapsedMs: now - Date.parse(startedAt) } });
		state.events = state.events.filter((e) => e.kind !== 'warehouse_sync_stalled');
		bus?.emit('platform_events');
		announce();
	}

	// The scenario alone decides the problems, so each tier reads on its own; `several` keeps the other locations' too.
	const others = scenario === 'several';
	const keep = (e: PlatformEvent) => e.locationId !== LOCATION_SHOP && (others || e.locationId === null);
	const withEvents = <T extends PlatformEvent>(events: T[], record: (e: PlatformEvent) => T): T[] => [...state.events.map(record), ...events.filter((e) => !e.active || keep(e))];
	/** Another location as live and connected, its own problems aside. */
	const calmOther = (l: LocationSummary): LocationSummary => (others ? l : {
		...l,
		health: l.health === 'broken' || l.health === 'degraded' ? 'healthy' : l.health,
		liveInstances: Math.max(1, l.liveInstances),
		lastSeenAt: l.liveInstances ? l.lastSeenAt : iso(Date.now() - 20_000),
		connectionTest: l.connectionTest?.outcome === 'failed' ? { ...l.connectionTest, outcome: 'ok', message: 'Connected' } : l.connectionTest,
		activeProblems: 0
	});
	/** The failed deploy the scenario names, and whether a previous version runs instead. */
	const deployOf = (base: Pick<LocationDetail, 'failedDeploy' | 'brokenDeployPolicy'>) =>
		state.deploy
			? { failedDeploy: { gitRevision: '9e1d7c2', imageId: 'sha256:77c0a1d2e4f9', attemptedAt: iso(Date.now() - 5 * 60_000), error: LOAD_ERROR }, brokenDeployPolicy: state.deploy.fallback ? ('fallback' as const) : ('queue' as const) }
			: { failedDeploy: null, brokenDeployPolicy: base.brokenDeployPolicy };
	const overrides: Partial<Record<keyof ApiClient, unknown>> = {
		async getWarehouseSync(scope: Scope): Promise<WarehouseSync | null> {
			if (!shop(scope)) return client.getWarehouseSync(scope);
			await delay(60);
			// A running sync keeps reporting progress until it is done; a stalled one doesn't.
			if (scenario === 'running' && state.sync.syncing && !state.sync.requested) state.sync.lastReportedAt = iso(Date.now() - 3_000);
			return structuredClone(state.sync);
		},
		async requestWarehouseSync(scope: Scope): Promise<void> {
			if (!shop(scope)) return client.requestWarehouseSync(scope);
			await delay(250);
			if (!state.live) throw new ApiError(NOT_CHECKING_IN('sync'), 409);
			if (state.sync.requested || (state.sync.syncing && scenario !== 'stalled')) return;
			if (scenario === 'requested') return;
			void runSync();
		},
		async testConnection(scope: Scope, connection: string): Promise<ConnectionTest> {
			if (!shop(scope)) return client.testConnection(scope, connection);
			if (!state.live) {
				await delay(250);
				throw new ApiError(NOT_CHECKING_IN('test its connection'), 409);
			}
			const started = Date.now();
			await delay(1100);
			const failed = state.sync.failing !== null && state.sync.failing !== undefined;
			state.test = { outcome: failed ? 'failed' : 'ok', at: iso(Date.now()), durationMs: Date.now() - started, trigger: 'manual', by: { kind: 'user', name: 'ada', displayName: 'Ada' }, message: failed ? IP_BLOCKED : 'Connected as TRANSFORMER to WAFFLE_SHOP; warehouse TRANSFORMING_WH available' };
			announce();
			return structuredClone(state.test);
		},
		async republish(scope: Scope): Promise<RepublishResult> {
			if (!shop(scope)) return client.republish(scope);
			if (!state.live) {
				await delay(250);
				throw new ApiError(NOT_CHECKING_IN('publish'), 409);
			}
			await delay(1800);
			return client.republish(scope);
		},
		async getLocation(id: string): Promise<LocationDetail | null> {
			const detail = await client.getLocation(id);
			if (!detail || !shop(id)) return detail;
			const instances = state.live ? detail.instances : detail.instances.map((i) => ({ ...i, state: 'stale' as const, lastSeenAt: lastSeen() }));
			return { ...detail, health: state.live ? (state.deploy?.fallback ? 'falling_back' : detail.health) : 'broken', instances, connectionTest: state.test, ...deployOf(detail) };
		},
		async listLocations(): Promise<LocationSummary[]> {
			const locations = await client.listLocations();
			return locations.map((l) =>
				shop(l.id)
					? { ...l, connectionTest: state.test, activeProblems: state.events.length, ...deployOf(l), ...(state.live ? {} : { health: 'broken' as const, liveInstances: 0, lastSeenAt: lastSeen() }) }
					: calmOther(l)
			);
		},
		async listWarehouseConnections(): Promise<WarehouseConnection[]> {
			const connections = await client.listWarehouseConnections();
			const failing = state.sync.failing;
			return connections.map((c) =>
				shop(c.id)
					? {
							...c,
							lastTest: state.test,
							halted: !!failing?.halted,
							tables: failing
								? c.tables.map((t) =>
										failing.unreachable || t.table === 'audit_results'
											? { ...t, at: state.sync.lastReportedAt ?? t.at, recordsSent: 0, failure: 'Failed while reading the warehouse', error: failing.reason, lastSyncedAt: state.sync.lastSyncedAt }
											: t
									)
								: c.tables
						}
					: others
						? c
						: { ...c, lastTest: c.lastTest?.outcome === 'failed' ? { ...c.lastTest, outcome: 'ok' as const, message: 'Connected' } : c.lastTest, tables: c.tables.map((t) => ({ ...t, failure: null, error: null })) }
			);
		},
		async listPlatformEvents(query: { activeOnly: boolean }): Promise<PlatformEvent[]> {
			return withEvents(await client.listPlatformEvents(query), (e) => e);
		},
		async listPlatformEventHistory(...args: Parameters<ApiClient['listPlatformEventHistory']>): Promise<PlatformEventRecord[]> {
			const events = await client.listPlatformEventHistory(...args);
			const query = args[0] ?? {};
			if (query.locationId !== undefined && query.locationId !== LOCATION_SHOP) return events;
			return withEvents(events, (e) => ({ ...e, resolvedAt: null, links: [{ kind: 'location', id: LOCATION_SHOP }], reactions: [], payload: {} }) as PlatformEventRecord);
		},
		async navCounts(...args: Parameters<ApiClient['navCounts']>): Promise<NavCounts> {
			const counts = await client.navCounts(...args);
			const open = withEvents(await client.listPlatformEvents({ activeOnly: true }), (e) => e).filter((e) => e.active && e.severity !== 'info');
			return { ...counts, activePlatformEvents: open.length };
		},
		async overview(...args: Parameters<ApiClient['overview']>): Promise<Overview | null> {
			const overview = await client.overview(...args);
			if (!overview || !shop(args[0])) return overview;
			const attention = overview.attention.filter((item) => item.kind !== 'platform_event' || keep(item.event));
			return { ...overview, attention: [...state.events.map((event) => ({ kind: 'platform_event' as const, event })), ...attention] };
		}
	};
	if (bus && scenario === 'reconnecting') {
		// Reconnect follows the feed again: connecting, then live.
		const changes: ApiClient['changes'] = {
			subscribe: (handler) => bus.subscribe(handler),
			onConnectionState: (handler) => bus.onConnectionState(handler),
			follow: () => {
				bus.setState('connecting');
				setTimeout(() => bus.setState('live'), 1200);
			}
		};
		overrides.changes = changes;
	}
	return new Proxy(client, {
		get(target, prop, receiver) {
			if (typeof prop === 'string' && prop in overrides) return overrides[prop as keyof ApiClient];
			const value = Reflect.get(target, prop, receiver);
			return typeof value === 'function' ? value.bind(target) : value;
		}
	});
}
