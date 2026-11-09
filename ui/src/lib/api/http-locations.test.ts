import { describe, expect, it } from 'vitest';
import { connectionTestOf, healthOf, warehouseSyncOf } from './http-locations';

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

describe('warehouseSyncOf', () => {
	const sync = (at: string, table: 'catalog' | 'statistics', failure: 'read' | 'delivery' | null) => ({
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
		expect(result).toEqual({ lastSyncedAt: '2026-11-08T12:02:00Z', lastError: 'catalog: failed while reading the warehouse' });
	});

	it('clears the error once the failing table syncs again', () => {
		expect(warehouseSyncOf([sync('2026-11-08T12:05:00Z', 'catalog', null), sync('2026-11-08T12:03:00Z', 'catalog', 'read')])?.lastError).toBeNull();
	});
});
