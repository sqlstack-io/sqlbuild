import { describe, expect, it } from 'vitest';
import { connectionTestOf, healthOf, instancesOf, toLocationDetail, toWarehouseConnection, warehouseSyncOf } from './http-locations';

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

	it('counts each live instance once', () => {
		const observations = [seen(10, { kind: 'heartbeat' }), seen(20, { kind: 'connection', generation: 2, healthy: true }), seen(30, { kind: 'heartbeat' }, 's-2')];
		expect(healthOf(observations, now)).toMatchObject({ health: 'healthy', liveInstances: 2, lastSeenAt: at(10) });
	});

	it('falls back after a failed load until a newer generation connects', () => {
		const failed = seen(20, { kind: 'compile_failed', generation: 3 });
		expect(healthOf([seen(5, { kind: 'heartbeat' }), failed, seen(40, { kind: 'connection', generation: 2, healthy: true })], now).health).toBe('falling_back');
		expect(healthOf([seen(5, { kind: 'connection', generation: 4, healthy: true }), failed], now).health).toBe('healthy');
	});

	it('needs attention when the latest connection check failed', () => {
		const observations = [seen(5, { kind: 'connection', generation: 2, healthy: false }), seen(60, { kind: 'connection', generation: 1, healthy: true })];
		expect(healthOf(observations, now).health).toBe('degraded');
		expect(connectionTestOf(observations)).toMatchObject({ outcome: 'failed', at: at(5) });
	});
});

describe('location processes', () => {
	it('lists each session once by its newest check-in, newest first, and stale after 90 seconds', () => {
		const observations = [seen(200, { kind: 'heartbeat' }, 'old'), seen(30, { kind: 'heartbeat' }), seen(95, { kind: 'heartbeat' }, 'old'), seen(5, { kind: 'connection', generation: 2, healthy: true })];
		expect(instancesOf(observations, now).map((i) => [i.id, i.lastSeenAt, i.state])).toEqual([
			['s-1', at(5), 'live'],
			['old', at(95), 'stale']
		]);
	});
});

describe('toLocationDetail', () => {
	const location = { id: 'loc:prod', name: 'orders', health: null, lastSeenAt: null, target: { name: 'prod', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };

	it('shows a failed reload as its health, without inventing a failed deploy or error text', () => {
		const detail = toLocationDetail(location, [seen(5, { kind: 'heartbeat' }), seen(20, { kind: 'compile_failed', generation: 3 }), seen(40, { kind: 'connection', generation: 2, healthy: true })], null, now);
		expect([detail.health, detail.failedDeploy, detail.lastLoadError, detail.snapshot]).toEqual(['falling_back', null, null, null]);
	});

	it('is healthy again once a newer generation connects', () => {
		const detail = toLocationDetail(location, [seen(5, { kind: 'connection', generation: 4, healthy: true }), seen(20, { kind: 'compile_failed', generation: 3 })], null, now);
		expect([detail.health, detail.connectionTest?.outcome]).toEqual(['healthy', 'ok']);
	});
});

describe('warehouseSyncOf', () => {
	const sync = (at: string, table: 'catalog' | 'statistics', failure: 'read' | 'delivery' | 'busy' | null) => ({
		received_at: at,
		report: { event_id: at, session_id: 's1', target: 'development', health: { kind: 'warehouse_sync' as const, generation: 1, location: { schema: 'main' }, table, failure, records_sent: 0, elapsed_millis: 5 } }
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
		expect(result).toEqual({ lastSyncedAt: '2026-11-08T12:02:00Z', lastError: 'catalog: failed while reading the warehouse', lastReportedAt: '2026-11-08T12:03:00Z' });
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

	it('shows a build holding the warehouse file as a wait, not a failure', () => {
		const location = { id: 'loc:development', name: 'Orders', health: null, lastSeenAt: null, target: { name: 'development', adapter: null, connectionSummary: null }, warehouseSync: { lastSyncedAt: null, lastError: null } };
		const observations = [sync('2026-11-08T12:03:00Z', 'catalog', 'busy'), sync('2026-11-08T12:01:00Z', 'catalog', null)];
		expect(toWarehouseConnection(location, observations).tables.map((t) => [t.failure, t.lastSyncedAt])).toEqual([['Waiting for a build that holds the warehouse file', '2026-11-08T12:01:00Z']]);
		expect(warehouseSyncOf(observations)).toEqual({ lastSyncedAt: '2026-11-08T12:01:00Z', lastError: null, lastReportedAt: '2026-11-08T12:03:00Z' });
	});

	it('clears the error once the failing table syncs again', () => {
		expect(warehouseSyncOf([sync('2026-11-08T12:05:00Z', 'catalog', null), sync('2026-11-08T12:03:00Z', 'catalog', 'read')])?.lastError).toBeNull();
	});
});
