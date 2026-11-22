import { describe, expect, it } from 'vitest';
import { connectionDetailsOf, connectionTestOf, firstSyncPending, healthOf, instancesOf, missingSecretsEventOf, missingSecretsOf, poolCapWarningsOf, toLocationDetail, toWarehouseConnection, warehouseSyncOf } from './http-locations';

const now = Date.parse('2026-11-08T12:00:00Z');
const at = (secondsAgo: number) => new Date(now - secondsAgo * 1000).toISOString();
type Health = Parameters<typeof healthOf>[0][number]['report']['health'];
const seen = (secondsAgo: number, health: Health, session = 's-1') => ({
	received_at: at(secondsAgo),
	report: { event_id: `e-${secondsAgo}`, session_id: session, target: 'prod', health }
});

describe('location health', () => {
	it('is broken without a check-in in the last 90 seconds, whatever was reported before', () => {
		expect(healthOf([seen(91, { kind: 'heartbeat' })], now)).toEqual({ health: 'broken', liveInstances: 0, lastSeenAt: at(91) });
	});

	it("reads the server's newest pool-cap warning, job by job", () => {
		const job = { job_key: 'close_of_business', kind: 'group' as const, pool: 'warehouse_heavy', amount: 3, cap: 1, message: 'group close_of_business takes 3 of pool warehouse_heavy; this server caps it at 1, so it runs alone in that pool' };
		const capped = seen(5, { kind: 'pool_capped', generation: 2, jobs: [job] }, '00000000-0000-0000-0000-000000000000');
		expect(poolCapWarningsOf([capped, seen(10, { kind: 'heartbeat' })])).toEqual([{ jobKey: 'close_of_business', kind: 'group', pool: 'warehouse_heavy', amount: 3, cap: 1, message: job.message }]);
		expect(poolCapWarningsOf([seen(10, { kind: 'heartbeat' })])).toEqual([]);
	});

	it("doesn't count the server's pool-cap warning as a location check-in", () => {
		const capped = seen(5, { kind: 'pool_capped', generation: 2, jobs: [] }, '00000000-0000-0000-0000-000000000000');
		expect(healthOf([capped, seen(91, { kind: 'heartbeat' })], now)).toEqual({ health: 'broken', liveInstances: 0, lastSeenAt: at(91) });
	});

	it('counts each live instance once', () => {
		const observations = [seen(10, { kind: 'heartbeat' }), seen(20, { kind: 'connection', generation: 2, healthy: true }), seen(30, { kind: 'heartbeat' }, 's-2')];
		expect(healthOf(observations, now)).toMatchObject({ health: 'healthy', liveInstances: 2, lastSeenAt: at(10) });
	});

	it('falls back after a failed load until a newer generation connects', () => {
		const failed = seen(20, { kind: 'compile_failed', generation: 3 });
		expect(healthOf([seen(5, { kind: 'heartbeat' }), failed, seen(40, { kind: 'connection', generation: 2, healthy: true })], now).health).toBe('falling_back');
		expect(healthOf([seen(5, { kind: 'connection', generation: 4, healthy: true }), failed], now).health).toBe('healthy');
	});

	it('reports missing provider secrets ahead of a failed connection until a newer report finds none', () => {
		const missing = seen(10, { kind: 'provider_secrets', generation: 2, missing: 1, providers: [] });
		const unhealthy = seen(10, { kind: 'connection', generation: 2, healthy: false });
		expect(healthOf([missing, unhealthy], now).health).toBe('missing_secrets');
		expect(healthOf([seen(5, { kind: 'provider_secrets', generation: 3, missing: 0, providers: [] }), missing], now).health).toBe('healthy');
	});

	it('lists the newest check\'s providers missing secrets, the target\'s allowed ones last, and raises only the others', () => {
		const orders = { provider: 'orders_api', path: 'providers/orders_api.py', settings: [{ field: 'api_key', variable: 'ORDERS_API_KEY', line: 18 }], nodes: ['source:raw_orders'], allowed: false };
		const bucket = { provider: 'export_bucket', path: 'providers/storage.py', settings: [{ field: 'secret', variable: 'BUCKET_SECRET' }], nodes: ['asset:export'], allowed: true };
		const older = seen(30, { kind: 'provider_secrets', generation: 1, missing: 2, providers: [orders, { ...orders, provider: 'gone' }] });
		const newest = seen(10, { kind: 'provider_secrets', generation: 2, missing: 1, providers: [bucket, orders] });
		expect(missingSecretsOf([newest, older]).map((p) => [p.provider, p.allowed, p.settings[0].line])).toEqual([
			['orders_api', false, 18],
			['export_bucket', true, null]
		]);
		expect(missingSecretsEventOf('loc-1:prod', [newest])).toMatchObject({
			id: 'secrets:loc-1:prod',
			kind: 'location_missing_secrets',
			severity: 'warning',
			occurredAt: at(10),
			members: ['ORDERS_API_KEY'],
			locationId: 'loc-1:prod',
			message: "orders_api is missing ORDERS_API_KEY in the location's environment, so runs of raw_orders stop before the warehouse."
		});
		const many = { ...orders, nodes: ['source:a', 'model:b', 'model:c', 'model:d', 'asset:e'] };
		expect(missingSecretsEventOf('loc-1:prod', [seen(4, { kind: 'provider_secrets', generation: 4, missing: 1, providers: [many] })])?.message).toBe(
			"orders_api is missing ORDERS_API_KEY in the location's environment, so runs of a, b, c and 2 more stop before the warehouse."
		);
		const allowedOnly = seen(5, { kind: 'provider_secrets', generation: 3, missing: 0, providers: [bucket] });
		expect(missingSecretsEventOf('loc-1:prod', [allowedOnly, newest])).toBeNull();
		expect(healthOf([allowedOnly], now).health).toBe('healthy');
	});

	it('needs attention when the latest connection check failed', () => {
		const observations = [seen(5, { kind: 'connection', generation: 2, healthy: false }), seen(60, { kind: 'connection', generation: 1, healthy: true })];
		expect(healthOf(observations, now).health).toBe('degraded');
		expect(connectionTestOf(observations)).toMatchObject({ outcome: 'failed', at: at(5) });
	});

	it('records the startup check with its duration, and an older location\'s check as after publishing', () => {
		expect(connectionTestOf([seen(5, { kind: 'connection', generation: 1, healthy: true, trigger: 'startup', elapsed_millis: 410 })])).toMatchObject({
			outcome: 'ok',
			trigger: 'startup',
			durationMs: 410,
			message: 'Connected on startup'
		});
		expect(connectionTestOf([seen(5, { kind: 'connection', generation: 2, healthy: false })])).toMatchObject({ trigger: 'publish', durationMs: 0, message: 'Could not connect after publishing' });
	});
});

describe('location processes', () => {
	it('lists each session once by its newest check-in, newest first, and stale after 90 seconds', () => {
		const observations = [seen(200, { kind: 'heartbeat' }, 'old'), seen(30, { kind: 'heartbeat' }), seen(95, { kind: 'heartbeat' }, 'old'), seen(5, { kind: 'connection', generation: 2, healthy: true })];
		expect(instancesOf(observations, now).map((i) => [i.id, i.lastSeenAt, i.state])).toEqual([
			['s-1', at(5), 'live'],
			['old', at(95), 'stale']
		]);
		const capped = seen(1, { kind: 'pool_capped', generation: 2, jobs: [] }, '00000000-0000-0000-0000-000000000000');
		expect(instancesOf([capped, ...observations], now).map((i) => i.id)).toEqual(['s-1', 'old']);
	});
});

describe('toLocationDetail', () => {
	const location = { id: 'loc:prod', name: 'orders', health: null, lastSeenAt: null, target: { name: 'prod', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };

	it('shows a failed reload as its health, without inventing a failed deploy or error text', () => {
		const detail = toLocationDetail(location, [seen(5, { kind: 'heartbeat' }), seen(20, { kind: 'compile_failed', generation: 3 }), seen(40, { kind: 'connection', generation: 2, healthy: true })], null, now);
		expect([detail.health, detail.failedDeploy, detail.lastLoadError, detail.snapshot]).toEqual(['falling_back', null, null, null]);
	});

	it('lists the rule findings that failed the load, as sqb compile prints them', () => {
		const rules = { finding_count: 3, findings: [{ code: 'SQBRMODEL102', message: 'SELECT * is allowed only inside dependency import CTEs', path: 'models/orders.sql', line: 2 }] };
		const detail = toLocationDetail(location, [seen(20, { kind: 'compile_failed', generation: 3, rules })], null, now);
		expect(detail.lastLoadError).toBe('Compile failed: 3 rule findings\nmodels/orders.sql:2 [SQBRMODEL102] SELECT * is allowed only inside dependency import CTEs\n… and 2 more');
	});

	it('is healthy again once a newer generation connects', () => {
		const detail = toLocationDetail(location, [seen(5, { kind: 'connection', generation: 4, healthy: true }), seen(20, { kind: 'compile_failed', generation: 3 })], null, now);
		expect([detail.health, detail.connectionTest?.outcome]).toEqual(['healthy', 'ok']);
	});
});

describe('warehouseSyncOf', () => {
	const sync = (at: string, table: 'catalog' | 'statistics', failure: 'read' | 'delivery' | 'busy' | null, error?: string) => ({
		received_at: at,
		report: { event_id: at, session_id: 's1', target: 'development', health: { kind: 'warehouse_sync' as const, generation: 1, location: { schema: 'main' }, table, failure, records_sent: 0, elapsed_millis: 5, error } }
	});

	it('is null without any sync report', () => {
		expect(warehouseSyncOf([])).toBeNull();
	});

	it('dates the newest clean sync and names only tables whose newest sync failed', () => {
		const result = warehouseSyncOf([
			sync('2026-11-08T12:03:00Z', 'catalog', 'read'),
			sync('2026-11-08T12:02:00Z', 'statistics', null),
			sync('2026-11-08T12:01:00Z', 'catalog', null),
			sync('2026-11-08T12:00:00Z', 'statistics', 'delivery')
		]);
		expect(result).toEqual({ lastSyncedAt: '2026-11-08T12:02:00Z', lastError: 'catalog: failed while reading the warehouse', lastReportedAt: '2026-11-08T12:03:00Z', syncing: null, completed: null });
	});

	it("gives each table its newest sync, the time it last synced cleanly, and where the state tables are read from", () => {
		const location = { id: 'loc:development', name: 'Orders', health: null, lastSeenAt: null, target: { name: 'development', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };
		const connection = toWarehouseConnection(location, [
			sync('2026-11-08T12:03:00Z', 'statistics', 'read'),
			sync('2026-11-08T12:02:00Z', 'catalog', null),
			sync('2026-11-08T12:01:00Z', 'statistics', null),
			sync('2026-11-08T12:00:00Z', 'statistics', null)
		]);
		expect([connection.id, connection.target, connection.stateLocation, connection.lastTest]).toEqual(['loc:development', 'development', { database: null, schema: 'main' }, null]);
		expect(connection.tables.map((t) => [t.table, t.at, t.failure, t.lastSyncedAt])).toEqual([
			['catalog', '2026-11-08T12:02:00Z', null, '2026-11-08T12:02:00Z'],
			['statistics', '2026-11-08T12:03:00Z', 'Failed while reading the warehouse', '2026-11-08T12:01:00Z']
		]);
	});

	it('counts the records mirrored of each table from its newest counted read, and names the next sync from the report that completed one', () => {
		const location = { id: 'loc:development', name: 'Orders', health: null, lastSeenAt: null, target: { name: 'development', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };
		const counted = (at: string, table: 'catalog' | 'statistics', records: number) => {
			const report = sync(at, table, null);
			return { ...report, report: { ...report.report, health: { ...report.report.health, mirrored_records: records } } };
		};
		const completed = counted('2026-11-08T12:03:00Z', 'statistics', 4_800);
		const observations = [
			{ ...completed, report: { ...completed.report, health: { ...completed.report.health, next_sync_at: '2026-11-08T12:33:00Z' } } },
			sync('2026-11-08T12:02:30Z', 'catalog', null),
			counted('2026-11-08T12:02:00Z', 'catalog', 120),
			counted('2026-11-08T12:01:00Z', 'catalog', 90)
		];
		const connection = toWarehouseConnection(location, observations);
		expect(connection.tables.map((t) => [t.table, t.mirroredTotal])).toEqual([
			['catalog', 120],
			['statistics', 4_800]
		]);
		expect(connection.nextSyncAt).toBe('2026-11-08T12:33:00Z');
		const uncounted = toWarehouseConnection(location, [sync('2026-11-08T12:40:00Z', 'catalog', null)]);
		expect([uncounted.nextSyncAt, uncounted.tables[0].mirroredTotal]).toEqual([null, null]);
	});

	it('marks a sync that failed reads stopped, from the newest report only', () => {
		const location = { id: 'loc:development', name: 'Orders', health: null, lastSeenAt: null, target: { name: 'development', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };
		const failed = sync('2026-11-08T12:03:00Z', 'catalog', 'read', 'permission denied for table _sqlbuild_catalog');
		const progress = { position: 2, kinds: 14, rows: 40, elapsed_millis: 900, completed: false, halted: true };
		const stopped = { ...failed, report: { ...failed.report, health: { ...failed.report.health, progress } } };
		const halted = toWarehouseConnection(location, [stopped, sync('2026-11-08T12:02:00Z', 'statistics', null)]);
		expect([halted.halted, halted.nextSyncAt, halted.tables.find((t) => t.table === 'catalog')?.error]).toEqual([true, null, 'permission denied for table _sqlbuild_catalog']);
		const resumed = toWarehouseConnection(location, [sync('2026-11-08T12:10:00Z', 'catalog', null), stopped]);
		expect(resumed.halted).toBe(false);
		expect(warehouseSyncOf([stopped])?.syncing).toBeNull();
		expect(warehouseSyncOf([{ ...stopped, report: { ...stopped.report, health: { ...stopped.report.health, progress: { ...progress, halted: false } } } }])?.syncing).toMatchObject({ position: 2, kinds: 14 });
	});

	it('shows the connection settings published with the newest snapshot, and none before it publishes them', () => {
		const location = { id: 'loc:development', name: 'Orders', health: null, lastSeenAt: null, target: { name: 'development', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };
		const published = {
			snapshot_id: 'snap-1',
			generation: 3,
			git_dirty: false,
			environment_hash: 'env',
			sqb_version: '1.0.0',
			published_at: '2026-11-08T12:00:00Z',
			changes: [],
			connection: { adapter: 'snowflake', account: 'orders-account', user: 'orders_loader', role: 'LOADER', warehouse: 'ORDERS_WH', database: 'ORDERS', auth_method: 'key_pair' }
		};
		expect(toWarehouseConnection(location, [], published).details).toEqual({
			adapter: 'snowflake',
			account: 'orders-account',
			host: null,
			user: 'orders_loader',
			role: 'LOADER',
			warehouse: 'ORDERS_WH',
			database: 'ORDERS',
			authMethod: 'key_pair'
		});
		expect(connectionDetailsOf({ ...published, connection: null })).toBeNull();
		expect(toWarehouseConnection(location, []).details).toBeNull();
	});

	it('shows a build holding the warehouse file as a wait, not a failure', () => {
		const location = { id: 'loc:development', name: 'Orders', health: null, lastSeenAt: null, target: { name: 'development', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };
		const observations = [sync('2026-11-08T12:03:00Z', 'catalog', 'busy'), sync('2026-11-08T12:01:00Z', 'catalog', null)];
		expect(toWarehouseConnection(location, observations).tables.map((t) => [t.failure, t.waiting, t.lastSyncedAt])).toEqual([['Waiting for a build that holds the warehouse file', true, '2026-11-08T12:01:00Z']]);
		expect(warehouseSyncOf(observations)).toEqual({ lastSyncedAt: '2026-11-08T12:01:00Z', lastError: null, lastReportedAt: '2026-11-08T12:03:00Z', syncing: null, completed: null });
	});

	it('says why a read failed and on what page size, without the warehouse message', () => {
		const timedOut = sync('2026-11-08T12:03:00Z', 'statistics', 'read');
		const health = { ...timedOut.report.health, read_failure: { kind: 'timed_out' as const, seconds: 130 }, page_limit: 100 };
		expect(warehouseSyncOf([{ ...timedOut, report: { ...timedOut.report, health } }])?.lastError).toBe('statistics: failed while reading the warehouse: no reply within 130 s, on a page of 100 relations');
		const erroring = sync('2026-11-08T12:04:00Z', 'catalog', 'read');
		const error = { ...erroring.report.health, read_failure: { kind: 'read_error' as const }, page_limit: 2000 };
		expect(warehouseSyncOf([{ ...erroring, report: { ...erroring.report, health: error } }])?.lastError).toBe('catalog: failed while reading the warehouse: the read failed; the location’s log has why, on a page of 2,000 rows');
	});

	it('shows a running sync and the newest completed one, and a first sync as pending until it completes', () => {
		const at = (time: string, table: 'catalog' | 'statistics', position: number, rows: number, completed: boolean) => {
			const report = sync(time, table, null);
			return { ...report, report: { ...report.report, health: { ...report.report.health, progress: { position, kinds: 14, rows, elapsed_millis: 12_800, completed } } } };
		};
		const now = Date.parse('2026-11-08T12:10:30Z');
		const first = warehouseSyncOf([at('2026-11-08T12:10:00Z', 'catalog', 13, 1640, false)]);
		expect(first?.syncing).toEqual({ position: 13, kinds: 14, table: 'catalog', rows: 1640 });
		expect(firstSyncPending(first, now)).toBe(true);
		expect(firstSyncPending(first, now + 60 * 60_000)).toBe(false);
		const later = warehouseSyncOf([at('2026-11-08T12:10:00Z', 'catalog', 13, 10, false), at('2026-11-08T12:00:00Z', 'statistics', 14, 31_204, true)]);
		expect(later?.completed).toEqual({ at: '2026-11-08T12:00:00Z', kinds: 14, rows: 31_204, elapsedMs: 12_800 });
		expect(firstSyncPending(later, now)).toBe(false);
	});

	it("shows a failed read's warehouse error after its stage", () => {
		const location = { id: 'loc:development', name: 'Orders', health: null, lastSeenAt: null, target: { name: 'development', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };
		const observations = [sync('2026-11-08T12:03:00Z', 'catalog', 'read', 'Insufficient privileges on schema STATE')];
		expect(toWarehouseConnection(location, observations).tables.map((t) => [t.failure, t.error])).toEqual([['Failed while reading the warehouse', 'Insufficient privileges on schema STATE']]);
		expect(warehouseSyncOf(observations)?.lastError).toBe('catalog: failed while reading the warehouse: Insufficient privileges on schema STATE');
	});

	it('clears the error once the failing table syncs again', () => {
		expect(warehouseSyncOf([sync('2026-11-08T12:05:00Z', 'catalog', null), sync('2026-11-08T12:03:00Z', 'catalog', 'read')])?.lastError).toBeNull();
	});
});
