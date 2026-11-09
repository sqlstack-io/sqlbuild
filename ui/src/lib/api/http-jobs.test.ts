import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { cadenceOf, jobDetailOf, runWindowOf, toJob, toTick, toTrigger, triggerDetailOf, triggerKindOf } from './http-jobs';
import type { Run } from './types/operate';

type W = components['schemas'];

const SNAPSHOT = '0191f0dd-0000-7000-8000-000000000000';

function trigger(id: string, name: string, jobKey: string, definition: W['TriggerDeclaration'], status: W['TriggerStatus'] = 'running'): W['TriggerRecord'] {
	return { trigger_id: id, name, job_key: jobKey, definition_json: definition, status, last_tick_for: null, next_due_at: '2026-11-07T06:00:00Z' };
}

const daily = trigger('t-daily', 'orders_daily', 'orders', { description: 'Daily orders', job_key: 'orders', schedule: { kind: 'cron', expression: '0 6 * * *', timezone: 'UTC' }, default_status: 'paused' });
const onInventory = trigger('t-inv', 'inventory_landed', 'inventory', {
	description: 'New stock feed',
	job_key: 'inventory',
	minimum_interval_seconds: 60,
	when: { kind: 'source_new_data', selection: ['source:stock_feed'] }
});

function run(id: string, jobKey: string, triggerId: string | null): Run {
	return { id, jobKey, triggerId, status: 'success', startedAt: '2026-11-07T01:00:00Z', endedAt: '2026-11-07T01:01:00Z' } as Run;
}

describe('trigger mapping', () => {
	it('classifies schedules and sensors and describes their cadence', () => {
		expect(triggerKindOf(daily.definition_json)).toBe('cron_schedule');
		expect(triggerKindOf(onInventory.definition_json)).toBe('node_sensor');
		expect(triggerKindOf({ ...onInventory.definition_json, when: { kind: 'run_event', job_key: 'orders', kinds: ['succeeded' as never] } } as W['SensorDefinition'])).toBe('job_sensor');
		expect(cadenceOf(daily.definition_json)).toBe('0 6 * * * (UTC)');
		expect(cadenceOf(onInventory.definition_json)).toBe('when source:stock_feed has new data');
	});

	it('keeps the code default apart from the effective state and links the newest run it started', () => {
		const runs = [run('r-new', 'orders', 't-daily'), run('r-manual', 'orders', null), run('r-old', 'orders', 't-daily')];
		const t = toTrigger({ ...daily, status: 'running' }, runs, null);
		expect([t.defaultState, t.state, t.type, t.cron, t.timezone]).toEqual(['paused', 'running', 'schedule', '0 6 * * *', 'UTC']);
		expect(t.lastRun?.id).toBe('r-new');
		expect(toTrigger(onInventory, runs, null).lastRun).toBeNull();
	});

	it('links a group trigger to its group run, not a child listed before it', () => {
		const child = { ...run('r-child', 'orders_member', 't-daily'), parentRunId: 'r-group' } as Run;
		const group = { ...run('r-group', 'orders', 't-daily'), status: 'failed' } as Run;
		expect(toTrigger(daily, [child, group], null).lastRun?.id).toBe('r-group');
	});

	it('reports lateness from evaluation time and hides sensor parts the server lacks', () => {
		const tick = toTick({ tick_id: 'k1', trigger_id: 't-daily', epoch: 1, scheduled_for: '2026-11-07T06:00:00Z', evaluated_at: '2026-11-07T06:02:30Z', outcome: 'requested', run_id: 'r-new' });
		expect(tick.lateBySeconds).toBe(150);
		const detail = triggerDetailOf(toTrigger(onInventory, [], tick), onInventory, [tick]);
		expect(detail.sensor).toMatchObject({ minimumIntervalSeconds: 60, cursorKnown: false });
		expect([detail.declaration, detail.upcoming, detail.evaluations, detail.stateHistory]).toEqual([null, null, null, null]);
		expect(triggerDetailOf(toTrigger(daily, [], null), daily, []).sensor).toBeNull();
	});
});

describe('job mapping', () => {
	it('links a job to its triggers and newest run, and leaves unreported policy null', () => {
		const wire: W['JobRecord'] = { job_key: 'orders', kind: 'job', description: 'Build orders', selection: ['model:customers', 'model:orders'], concurrency_groups: ['warehouse_heavy'], snapshot_id: SNAPSHOT };
		const triggers = [toTrigger(daily, [], null), toTrigger(onInventory, [], null)];
		const job = toJob(wire, triggers, [run('r-inv', 'inventory', null), run('r-ord', 'orders', null)]);
		expect(job).toMatchObject({ key: 'orders', kind: 'job', nodeCount: 2, triggerIds: ['t-daily'], concurrencyGroups: ['warehouse_heavy'], select: null, overlap: null, priority: null });
		expect(job.lastRun?.id).toBe('r-ord');
		const detail = jobDetailOf(job, wire);
		expect(detail.resolvedNodes).toEqual(['model:customers', 'model:orders']);
		expect([detail.settings, detail.concurrency, detail.memberOf, detail.cursorType]).toEqual([null, null, null, null]);
		expect(toJob({ ...wire, job_key: 'nightly', kind: 'group' }, triggers, []).kind).toBe('group');
	});
});

describe('runWindowOf', () => {
	const typed = (start: string, end: string) => runWindowOf({ type: 'timestamp', start, end });

	it('sends whole numbers on both ends as an integer window, negatives included', () => {
		expect(typed(' -3 ', '11')).toEqual({ kind: 'integer', start: -3, end: 11 });
	});

	it('sends anything else as typed for the server to validate, without reading zones or order', () => {
		expect(typed('2026-10-31T00:00:00+02:00', '2026-10-01')).toEqual({ kind: 'timestamp', start: '2026-10-31T00:00:00+02:00', end: '2026-10-01' });
		expect(typed('1200', 'yesterday')).toEqual({ kind: 'timestamp', start: '1200', end: 'yesterday' });
	});

	it('refuses a whole number JSON would round instead of sending another value', () => {
		expect(() => typed('9007199254740993', '9007199254740995')).toThrow('exactly');
	});
});

describe('schedule due times', () => {
	// Stored out of order on purpose; one time is before the server's next due time.
	const times = ['2026-11-10T09:00:00Z', '2026-11-06T09:00:00Z', '2026-11-07T09:00:00Z', '2026-11-08T09:00:00Z'];
	const calendar = (nextDueAt: string | null) => ({
		...trigger('t-cal', 'business_days', 'orders', { description: 'Business days', job_key: 'orders', schedule: { kind: 'calendar', times, refresh_at: '2026-11-08T00:00:00Z' } }),
		next_due_at: nextDueAt
	});
	const detailOf = (wire: W['TriggerRecord']) => triggerDetailOf(toTrigger(wire, [], null), wire, []);

	it('lists a calendar from the next due time on, in time order, with the last stored time and next refresh', () => {
		const detail = detailOf(calendar('2026-11-07T09:00:00Z'));
		expect(detail.upcoming).toEqual(['2026-11-07T09:00:00Z', '2026-11-08T09:00:00Z', '2026-11-10T09:00:00Z']);
		expect(detail.calendar).toMatchObject({ storedUntil: '2026-11-10T09:00:00.000Z', nextRefreshAt: '2026-11-08T00:00:00Z', horizon: null, low: false });
	});

	it('has no upcoming calendar times once the server has no next due time', () => {
		expect(detailOf(calendar(null)).upcoming).toEqual([]);
	});

	it('shows only the next due time for cron, without working out later ones', () => {
		expect(detailOf(daily).upcoming).toEqual(['2026-11-07T06:00:00Z']);
		expect(detailOf(daily).calendar).toBeNull();
	});

	it('has no due times for a sensor', () => {
		expect(detailOf(onInventory).upcoming).toBeNull();
	});
});
