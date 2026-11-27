import { describe, expect, it } from 'vitest';
import { toPlatformEventRecords, toPlatformEvents, toPlatformHealth } from './http-problems';
import { formatClock } from '$lib/format/time';

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
	it('makes a failed connection check a problem that stops runs, and a failed load on the previous version one that needs a look', () => {
		const serving = { snapshot_id: 's-11', location_id: 'l1', target: 'production', generation: 11, manifest_digest: 'd', published_at: '2026-11-08T09:00:00Z' };
		const failing = (state: 'falling_back' | 'degraded', error: string | null) => ({ location_id: 'l1', location_name: 'orders', target: 'production', state, failed_at: '2026-11-08T11:59:00Z', error, serving });
		const [failed, degraded] = toPlatformEvents({ scheduler_leader: true, broken_locations: [], overdue_ticks: [], failing_locations: [failing('falling_back', 'job loaders_job: path selectors require an explicit root'), failing('degraded', null)] }, NOW);
		expect([failed.kind, failed.severity, failed.state, failed.active, failed.occurredAt]).toEqual(['location_failing', 'warning', 'falling_back', true, '2026-11-08T11:59:00Z']);
		expect(failed.message).toMatch(/^orders · production: latest code failed to load: job loaders_job: path selectors require an explicit root\. Serving generation 11, published /);
		expect([degraded.severity, degraded.state, degraded.message.split('. ')[0]]).toEqual(['critical', 'degraded', 'orders · production: the warehouse connection check failed']);
	});

	it('makes a failing run-history sync a problem since its first failure, with its reason and how it retries', () => {
		const sync = (unreachable: boolean, halted: boolean) => ({
			location_id: 'l1',
			location_name: 'orders',
			target: 'production',
			since: '2026-11-08T11:42:00Z',
			reason: '250001 Could not connect to Snowflake backend after 2 attempt(s)',
			unreachable,
			halted,
			retry_at: halted ? null : '2026-11-08T12:00:40Z',
			retry_seconds: halted ? null : 60,
			last_success_at: '2026-11-08T11:40:00Z'
		});
		const events = toPlatformEvents({ scheduler_leader: true, broken_locations: [], overdue_ticks: [], failing_syncs: [sync(true, false), sync(false, true)] }, NOW);
		const since = formatClock('2026-11-08T11:42:00Z', NOW);
		expect(events.map((e) => [e.kind, e.severity, e.occurredAt, e.locationId, e.message, e.sync?.halted])).toEqual([
			['warehouse_sync_failed', 'warning', '2026-11-08T11:42:00Z', 'l1:production', `orders · production: run history can't reach the warehouse since ${since} · retrying every 60 s: 250001 Could not connect to Snowflake backend after 2 attempt(s)`, false],
			['warehouse_sync_failed', 'warning', '2026-11-08T11:42:00Z', 'l1:production', `orders · production: run history sync stopped after failed reads in a row since ${since}: 250001 Could not connect to Snowflake backend after 2 attempt(s). Resume it once the cause is fixed.`, true]
		]);
	});

	it('makes a sink that loses events, and a webhook the server can’t deliver to, problems since they began', () => {
		const sink = { location_id: 'l1', location_name: 'orders', target: 'production', sink: 'kafka', since: '2026-11-08T11:42:00Z', reason: '8 of 8 events not delivered (8 failed)', run_id: 'r-9', last_success_at: null };
		const forwarding = { name: 'Platform event forwarding', since: '2026-11-08T11:50:00Z', reason: 'Can’t deliver platform events: the webhook answered HTTP 503; they are kept and retried every 5 s', last_success_at: null };
		const events = toPlatformEvents({ scheduler_leader: true, broken_locations: [], overdue_ticks: [], failing_sinks: [sink], failing_forwarding: [forwarding] }, NOW);
		const at = (iso: string) => formatClock(iso, NOW);
		expect(events.map((e) => [e.kind, e.severity, e.locationId, e.subject, e.message])).toEqual([
			['event_sink_failed', 'warning', 'l1:production', 'kafka', `orders · production: event sink kafka losing events since ${at(sink.since)}: 8 of 8 events not delivered (8 failed).`],
			['event_forwarding_failed', 'warning', null, 'Platform event forwarding', `Platform event forwarding failing since ${at(forwarding.since)}: ${forwarding.reason}.`]
		]);
	});

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

	it('groups low calendars per location, dated and named by the one that runs out first', () => {
		const calendar = (name: string, location: string, horizon_end: string | null) => ({
			trigger_id: `c-${name}`,
			name,
			location_id: location,
			location_name: location === 'l1' ? 'orders' : 'inventory',
			target: 'development',
			refresh_at: '2026-11-08T13:00:00Z',
			horizon_end
		});
		const events = toPlatformEvents(
			{
				scheduler_leader: true,
				broken_locations: [],
				overdue_ticks: [],
				low_calendars: [calendar('month_end', 'l1', '2026-11-09T06:00:00Z'), calendar('quarter_end', 'l1', '2026-11-08T18:00:00Z'), calendar('stock_counts', 'l2', null)]
			},
			NOW
		);
		expect(events.map((e) => [e.kind, e.locationId, e.occurredAt, e.message])).toEqual([
			['calendar_horizon_low', 'l1:development', '2026-11-08T18:00:00Z', '2 calendars in orders · development are running out of dates, starting with quarter_end.'],
			['calendar_horizon_low', 'l2:development', '2026-11-08T13:00:00Z', 'stock_counts in inventory · development has no dates left; the location works out more at its next refresh.']
		]);
	});

	it('groups skipped triggers per location, dated and named by the first one skipped, with its reason', () => {
		const skipped = (name: string, location: string, skipped_at: string) => ({
			trigger_id: `s-${name}`,
			name,
			location_id: location,
			location_name: location === 'l1' ? 'orders' : 'inventory',
			target: 'development',
			skipped_at,
			reason: "this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again"
		});
		const events = toPlatformEvents(
			{
				scheduler_leader: true,
				broken_locations: [],
				overdue_ticks: [],
				skipped_triggers: [skipped('nightly_orders', 'l1', '2026-11-08T11:30:00Z'), skipped('hourly_orders', 'l1', '2026-11-08T11:00:00Z'), skipped('stock_levels', 'l2', '2026-11-08T11:45:00Z')]
			},
			NOW
		);
		expect(events.map((e) => [e.kind, e.locationId, e.occurredAt, e.members, e.message])).toEqual([
			['trigger_skipped', 'l1:development', '2026-11-08T11:00:00Z', ['s-hourly_orders', 's-nightly_orders'], "2 triggers in orders · development need attention, starting with hourly_orders: this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again."],
			['trigger_skipped', 'l2:development', '2026-11-08T11:45:00Z', ['s-stock_levels'], "stock_levels in inventory · development: this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again."]
		]);
	});

	it('lists sensors checked again from scratch apart from skipped triggers, under their own kind', () => {
		const events = toPlatformEvents(
			{
				scheduler_leader: true,
				broken_locations: [],
				overdue_ticks: [],
				rechecked_sensors: [{ trigger_id: 's-health', name: 'health_orders', location_id: 'l1', location_name: 'orders', target: 'development', rechecked_at: '2026-11-08T11:30:00Z' }]
			},
			NOW
		);
		expect(events.map((e) => [e.kind, e.severity, e.locationId, e.members, e.message])).toEqual([
			['sensor_rechecked', 'info', 'l1:development', ['s-health'], 'health_orders in orders · development is being checked again from scratch: its last check was recorded by an earlier SQLBuild release and can’t be read.']
		]);
	});

	it('names the calendar times a republish dropped, under their own kind', () => {
		const times = Array.from({ length: 12 }, (_, i) => `2026-11-${String(i + 1).padStart(2, '0')}T00:00:00Z`);
		const [event] = toPlatformEvents(
			{
				scheduler_leader: true,
				broken_locations: [],
				overdue_ticks: [],
				dropped_calendar_times: [{ trigger_id: 'c-month', name: 'month_end', location_id: 'l1', location_name: 'orders', target: 'development', dropped_at: '2026-11-08T11:30:00Z', times }]
			},
			NOW
		);
		expect([event.kind, event.locationId, event.members]).toEqual(['calendar_times_dropped', 'l1:development', ['c-month']]);
		expect(event.message).toBe(
			`month_end in orders · development won’t tick ${times.slice(0, 10).join(', ')} and 2 more: its previous definition was published by an earlier SQLBuild release and can’t be read.`
		);
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
				{ storage_id: 7, event_id: 'e7', occurred_at: '2026-11-08T10:30:00Z', event: { kind: 'run_failed', run_id: 'r2', run_kind: 'job', job_key: 'orders_daily', location_id: 'l1', target: 'development', outcome: 'failed', trigger: { kind: 'schedule' }, failed_nodes: [{ node_key: 'model:orders' }], failed_node_count: 1, failed_audits: [], failed_audit_count: 0, succeeded_node_count: 3, skipped_nodes: [], skipped_node_count: 0 } },
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
