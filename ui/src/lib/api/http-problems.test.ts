import { describe, expect, it } from 'vitest';
import { toPlatformEvents } from './http-problems';

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
