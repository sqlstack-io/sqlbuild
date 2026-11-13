import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { cadenceOf, jobDetailOf, runWindowOf, toJob, toTick, toTrigger, toTriggerStateEvent, triggerDetailOf, triggerKindOf } from './http-jobs';
import type { Run } from './types/operate';

type W = components['schemas'];

const SNAPSHOT = '0191f0dd-0000-7000-8000-000000000000';

function trigger(id: string, name: string, jobKey: string, definition: W['TriggerDeclaration'], status: W['TriggerStatus'] = 'running'): W['PersonTriggerRecord'] {
	const upcoming = 'schedule' in definition ? ['2026-11-07T06:00:00Z', '2026-11-08T06:00:00Z'] : [];
	return { trigger_id: id, name, job_key: jobKey, definition_json: definition, status, last_tick_for: null, next_due_at: '2026-11-07T06:00:00Z', upcoming };
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

	it('shows where a trigger is declared and the targets it is limited to, and nothing for an older snapshot', () => {
		const declared = trigger('t-cal', 'fulfillment_calendar', 'orders', {
			...onInventory.definition_json,
			targets: ['production'],
			declared_at: { path: 'orchestration/orders.py', line: 8 }
		} as W['SensorDefinition']);
		const detail = triggerDetailOf(toTrigger(declared, [], null), declared, [], []);
		expect([detail.sourceFile, detail.targets]).toEqual(['orchestration/orders.py:8', ['production']]);
		const older = triggerDetailOf(toTrigger(daily, [], null), daily, [], []);
		expect([older.sourceFile, older.targets]).toEqual([null, null]);
	});

	it('links a group trigger to its group run, not a child listed before it', () => {
		const child = { ...run('r-child', 'orders_member', 't-daily'), parentRunId: 'r-group' } as Run;
		const group = { ...run('r-group', 'orders', 't-daily'), status: 'failed' } as Run;
		expect(toTrigger(daily, [child, group], null).lastRun?.id).toBe('r-group');
	});

	it('reports lateness from evaluation time and hides sensor parts the server lacks', () => {
		const tick = toTick({ tick_id: 'k1', trigger_id: 't-daily', epoch: 1, scheduled_for: '2026-11-07T06:00:00Z', evaluated_at: '2026-11-07T06:02:30Z', outcome: 'requested', run_id: 'r-new' });
		expect(tick.lateBySeconds).toBe(150);
		const detail = triggerDetailOf(toTrigger(onInventory, [], tick), onInventory, [tick], []);
		expect(detail.sensor).toMatchObject({ minimumIntervalSeconds: 60, cursorKnown: false });
		expect([detail.declaration, detail.upcoming, detail.catchUp, detail.evaluations, detail.stateHistory]).toEqual([null, null, null, null, []]);
		const schedule = triggerDetailOf(toTrigger(daily, [], null), daily, [], []);
		expect([schedule.sensor, schedule.upcoming, schedule.catchUp]).toEqual([null, ['2026-11-07T06:00:00Z', '2026-11-08T06:00:00Z'], { mode: 'latest', max: 1 }]);
	});

	it("shows a sensor's checks newest first, its cursor and when it moved, and flags code changed since the last check", () => {
		const check = (id: string, at: string, before: W['PersonSensorCursor'], after: W['PersonSensorCursor'] | null, fingerprint: string): W['PersonSensorEvaluation'] => ({
			evaluation_id: id,
			phase: after ? 'completed' : 'failed',
			requested_at: at,
			finished_at: at.replace(':00Z', ':05Z'),
			cursor_before: before,
			cursor_after: after,
			run_count: after ? 2 : 0,
			failure_code: after ? null : 'location_unreachable',
			definition_fingerprint: fingerprint
		});
		const state: W['PersonSensorState'] = {
			cursor: { event_storage_id: 42, custom: 'feed/page-[REDACTED]' },
			definition_fingerprint: 'v2',
			evaluations: [
				check('e-new', '2026-11-07T06:10:00Z', { event_storage_id: 40, custom: null }, null, 'v1'),
				check('e-old', '2026-11-07T06:00:00Z', { event_storage_id: 0, custom: null }, { event_storage_id: 40, custom: null }, 'v1')
			],
			run_requests: [{ run_key: 'stock-7', run_id: 'r-7', evaluation_id: 'e-old', queued_at: '2026-11-07T06:00:05Z', status: 'success' }]
		};
		const reset = { id: 'c1', at: '2026-11-07T05:00:00Z', kind: 'cursor_reset', actor: { kind: 'user', name: 'ada', displayName: 'ada' }, reason: null, value: null } as const;
		const detail = triggerDetailOf(toTrigger(onInventory, [], null), onInventory, [], [reset], state);
		expect(detail.sensor).toMatchObject({ cursor: 'feed/page-[REDACTED]', cursorKnown: true, cursorUpdatedAt: '2026-11-07T06:00:05Z', currentFingerprint: 'v2', lastEvaluatedFingerprint: 'v1' });
		expect(detail.definitionChanged).toBe(true);
		expect(detail.evaluations?.map((e) => [e.id, e.outcome, e.cursorBefore, e.cursorAfter, e.error])).toEqual([
			['e-new', 'failed', 'event 40', null, 'location_unreachable'],
			['e-old', 'completed', null, 'event 40', null]
		]);
		expect(detail.runRequests).toEqual([{ runKey: 'stock-7', runId: 'r-7', evaluationId: 'e-old', requestedAt: '2026-11-07T06:00:05Z', runStatus: 'success' }]);
		const fresh = triggerDetailOf(toTrigger(onInventory, [], null), onInventory, [], [], { ...state, cursor: null, evaluations: [] });
		expect([fresh.sensor?.cursor, fresh.sensor?.cursorKnown, fresh.definitionChanged, fresh.sensor?.cursorUpdatedAt]).toEqual([null, true, false, null]);
	});
});

describe('job mapping', () => {
	it('links a job to its triggers and newest run, shows where it is declared, and leaves unreported policy null', () => {
		const wire: W['JobRecord'] = {
			job_key: 'orders',
			kind: 'job',
			description: 'Build orders',
			selection: ['model:customers', 'model:orders'],
			concurrency_groups: ['warehouse_heavy'],
			snapshot_id: SNAPSHOT,
			select: '+orders',
			declared_at: { path: 'orchestration/jobs.py', line: 12 }
		};
		const triggers = [toTrigger(daily, [], null), toTrigger(onInventory, [], null)];
		const job = toJob(wire, triggers, [run('r-inv', 'inventory', null), run('r-ord', 'orders', null)]);
		expect(job).toMatchObject({ key: 'orders', kind: 'job', nodeCount: 2, triggerIds: ['t-daily'], concurrencyGroups: ['warehouse_heavy'], select: '+orders', sourceFile: 'orchestration/jobs.py:12', overlap: null, priority: null });
		expect(job.lastRun?.id).toBe('r-ord');
		const detail = jobDetailOf(job, { ...wire, settings: { vars: '[REDACTED]', concurrency: 4, empty_ok: true, models: ['a', 'b'] }, members: [], member_of: ['nightly'], critical_nodes: ['model:orders'], exclude: ' stg_orders  audit_orders', targets: ['production'], cursor_type: 'integer' });
		expect(detail.resolvedNodes).toEqual(['model:customers', 'model:orders']);
		expect(detail.settings).toEqual({ vars: '[REDACTED]', concurrency: '4', empty_ok: true, models: '["a","b"]' });
		expect([detail.groupMembers, detail.members, detail.memberOf, detail.criticalNodes]).toEqual([null, null, ['nightly'], ['model:orders']]);
		expect([detail.exclude, detail.targets, detail.cursorType]).toEqual([['stg_orders', 'audit_orders'], ['production'], 'integer']);
		expect([detail.concurrency, detail.declaration]).toEqual([null, null]);
		expect(toJob({ ...wire, job_key: 'nightly', kind: 'group' }, triggers, []).kind).toBe('group');
	});

	it("steps a group's members by what they wait for, in the server's run order rather than by name", () => {
		const wire: W['JobDetailRecord'] = {
			job_key: 'nightly',
			kind: 'group',
			description: 'Everything nightly',
			selection: [],
			concurrency_groups: [],
			snapshot_id: SNAPSHOT,
			settings: {},
			member_of: [],
			critical_nodes: [],
			select: null,
			exclude: null,
			targets: [],
			declared_at: null,
			cursor_type: null,
			members: [
				{ job_key: 'stock', waits_for: [] },
				{ job_key: 'orders', waits_for: [] },
				{ job_key: 'customers', waits_for: ['orders'] },
				{ job_key: 'audit', waits_for: ['customers', 'stock'] }
			]
		};
		const detail = jobDetailOf(toJob(wire, [], []), wire);
		expect(detail.groupMembers?.map((m) => [m.jobKey, m.stage, m.upstream.map((u) => u.jobKey)])).toEqual([
			['stock', 0, []],
			['orders', 0, []],
			['customers', 1, ['orders']],
			['audit', 2, ['customers', 'stock']]
		]);
		expect(detail.groupMembers?.flatMap((m) => m.upstream.map((u) => u.source))).toEqual([null, null, null]);
		expect(detail.members).toEqual(['stock', 'orders', 'customers', 'audit']);
		expect([detail.select, detail.sourceFile, detail.exclude, detail.targets, detail.cursorType]).toEqual([null, null, [], null, null]);
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
	// Stored out of order on purpose.
	const times = ['2026-11-10T09:00:00Z', '2026-11-06T09:00:00Z', '2026-11-07T09:00:00Z', '2026-11-08T09:00:00Z'];
	const calendar = trigger('t-cal', 'business_days', 'orders', { description: 'Business days', job_key: 'orders', schedule: { kind: 'calendar', times, refresh_at: '2026-11-08T00:00:00Z' } });
	const detailOf = (wire: W['PersonTriggerRecord']) => triggerDetailOf(toTrigger(wire, [], null), wire, [], []);

	it('reports the last stored calendar time, the next refresh, and whether it is running out of dates', () => {
		const at = (now: string) => triggerDetailOf(toTrigger(calendar, [], null), calendar, [], [], null, Date.parse(now)).calendar;
		expect(at('2026-11-08T12:00:00Z')).toMatchObject({ storedUntil: '2026-11-10T09:00:00.000Z', nextRefreshAt: '2026-11-08T00:00:00Z', horizon: null, low: false });
		// The last date is 23 hours away: under the server's one-day warning.
		expect(at('2026-11-09T10:00:00Z')).toMatchObject({ low: true, lowHorizonWarningBelow: '1 day of dates' });
		expect(detailOf(daily).calendar).toBeNull();
	});

	it('takes due times from the server, and has none for a sensor', () => {
		expect(detailOf({ ...daily, upcoming: [] }).upcoming).toEqual([]);
		expect(detailOf(onInventory).upcoming).toBeNull();
	});
});

describe('trigger on/off history', () => {
	it('names a person, keeps a system actor, carries a set cursor and drops a blank reason', () => {
		const change = (extra: Partial<W['PersonTriggerChange']>): W['PersonTriggerChange'] => ({
			event_id: 'e1',
			change: { kind: 'pause' },
			actor: 'principal:0193',
			actor_name: 'ada-admin',
			reason: 'Hold for the warehouse upgrade',
			occurred_at: '2026-11-09T08:00:00Z',
			snapshot_id: 's1',
			...extra
		});
		expect(toTriggerStateEvent(change({}))).toEqual({
			id: 'e1',
			at: '2026-11-09T08:00:00Z',
			kind: 'paused',
			actor: { kind: 'user', name: 'ada-admin', displayName: 'ada-admin' },
			reason: 'Hold for the warehouse upgrade',
			value: null
		});
		const cursor = toTriggerStateEvent(change({ change: { kind: 'set_cursor', time: '2026-11-01T00:00:00Z' }, actor: 'local-operator', actor_name: null, reason: '  ' }));
		expect([cursor.kind, cursor.value, cursor.actor.kind, cursor.actor.name, cursor.reason]).toEqual(['cursor_set', '2026-11-01T00:00:00Z', 'system', 'local-operator', null]);
	});
});
