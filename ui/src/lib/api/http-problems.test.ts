import { describe, expect, it } from 'vitest';
import { toPlatformEventRecords, toPlatformEvents, toPlatformHealth } from './http-problems';

const NOW = Date.parse('2026-11-08T12:00:00Z');
const tick = (name: string, location: string, scheduledFor: string) => ({
	trigger_id: `t-${name}`,
	name,
	location_id: location,
	location_name: location === 'l1' ? 'orders' : 'inventory',
	target: 'development',
	scheduled_for: scheduledFor
});

describe('toPlatformEvents', () => {
	it('reports nothing while the scheduler leads and nothing is late', () => {
		expect(toPlatformEvents({ scheduler_leader: true, scheduler_lease_expires_at: '2026-11-08T12:00:10Z', broken_locations: [], overdue_ticks: [] }, NOW)).toEqual([]);
	});

	it('dates a stopped scheduler from its lease expiry, and from now when no server has led', () => {
		const [stopped] = toPlatformEvents({ scheduler_leader: false, scheduler_lease_expires_at: '2026-11-08T11:50:00Z', broken_locations: [], overdue_ticks: [] }, NOW);
		expect(stopped).toMatchObject({ kind: 'scheduler_no_leader', severity: 'critical', locationId: null, occurredAt: '2026-11-08T11:50:00Z' });
		const [never] = toPlatformEvents({ scheduler_leader: false, broken_locations: [], overdue_ticks: [] }, NOW);
		expect(never.occurredAt).toBe('2026-11-08T12:00:00.000Z');
	});

	it('keys broken locations by location and target', () => {
		const [broken] = toPlatformEvents(
			{ scheduler_leader: true, broken_locations: [{ location_id: 'l2', location_name: 'inventory', target: 'development', last_seen_at: '2026-11-08T11:55:00Z' }], overdue_ticks: [] },
			NOW
		);
		expect(broken).toMatchObject({ kind: 'location_broken', locationId: 'l2:development', occurredAt: '2026-11-08T11:55:00Z', subject: 'inventory · development' });
	});

	it('groups late ticks per location, dated and named by the oldest', () => {
		const events = toPlatformEvents(
			{
				scheduler_leader: true,
				broken_locations: [],
				overdue_ticks: [tick('hourly_orders', 'l1', '2026-11-08T11:40:00Z'), tick('daily_orders', 'l1', '2026-11-08T11:10:00Z'), tick('stock_levels', 'l2', '2026-11-08T11:59:00Z')]
			},
			NOW
		);
		expect(events.map((e) => [e.locationId, e.occurredAt, e.message])).toEqual([
			['l1:development', '2026-11-08T11:10:00Z', '2 schedules in orders · development are late, starting with daily_orders.'],
			['l2:development', '2026-11-08T11:59:00Z', 'stock_levels in inventory · development is late.']
		]);
	});
});

describe('toPlatformHealth', () => {
	it('reports a lease only while a server leads, with the replica and heartbeat left unknown', () => {
		const leading = toPlatformHealth({ scheduler_leader: true, scheduler_lease_expires_at: '2026-11-08T12:00:10Z', broken_locations: [], overdue_ticks: [] });
		expect(leading).toEqual({ scheduler: { lease: { holder: null, acquiredAt: null, renewedAt: null, expiresAt: '2026-11-08T12:00:10Z' }, instances: null }, heartbeat: null });
		expect('sink' in leading).toBe(false);
	});

	it('has no lease once the last one ran out, even though its expiry is still reported', () => {
		expect(toPlatformHealth({ scheduler_leader: false, scheduler_lease_expires_at: '2026-11-08T11:50:00Z', broken_locations: [], overdue_ticks: [] }).scheduler.lease).toBeNull();
	});
});

describe('toPlatformEventRecords', () => {
	it('marks the stored event of an open problem as ongoing instead of listing it twice, and keeps each location target', () => {
		const open = toPlatformEvents({ scheduler_leader: false, scheduler_lease_expires_at: null, broken_locations: [], overdue_ticks: [] }, NOW);
		const records = toPlatformEventRecords(
			open,
			[
				{ storage_id: 9, event_id: 'e9', occurred_at: '2026-11-08T11:00:00Z', location_id: 'l1', target: 'development', event: { kind: 'launch_failed', run_id: 'r1', claim_id: 'c1', code: 'image_missing' } },
				{ storage_id: 4, event_id: 'e4', occurred_at: '2026-11-08T10:00:00Z', event: { kind: 'scheduler_no_leader' } }
			],
			new Map([['l1:development', 'orders']])
		);
		expect(records.map((r) => [r.kind, r.active, r.locationId])).toEqual([
			['launch_failed', false, 'l1:development'],
			['scheduler_no_leader', true, null]
		]);
		expect(records[0]).toMatchObject({ subject: 'orders · development', message: "A run in orders · development couldn't start (image_missing).", links: [{ kind: 'run', id: 'r1', locationId: 'l1:development' }], payload: { code: 'image_missing' } });
	});

	it('lists an open problem nothing was stored for first', () => {
		const open = toPlatformEvents({ scheduler_leader: true, scheduler_lease_expires_at: null, broken_locations: [{ location_id: 'l2', location_name: 'inventory', target: 'development', last_seen_at: '2026-11-08T11:00:00Z' }], overdue_ticks: [] }, NOW);
		const records = toPlatformEventRecords(open, [{ storage_id: 1, event_id: 'e1', occurred_at: '2026-11-08T09:00:00Z', location_id: 'l1', target: 'development', event: { kind: 'location_broken', location_id: 'l1', target: 'development', last_seen_at: '2026-11-08T08:59:00Z' } }], new Map());
		expect(records.map((r) => [r.kind, r.active, r.locationId])).toEqual([
			['location_broken', true, 'l2:development'],
			['location_broken', false, 'l1:development']
		]);
	});
});
