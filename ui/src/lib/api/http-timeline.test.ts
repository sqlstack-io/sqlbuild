import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { timelineOf } from './http-timeline';

type W = components['schemas'];

const LOCATION = '0191f0aa-orders:development';
const WINDOW = { since: '2026-11-09T08:00:00Z', until: '2026-11-09T12:00:00Z' };
const NOW = Date.parse('2026-11-09T11:00:00Z');

function run(id: string, overrides: Partial<W['RunRecord']>): W['RunRecord'] {
	return {
		run_id: id,
		root_run_id: id,
		kind: 'job',
		origin: 'schedule',
		status: 'success',
		job_key: 'orders_daily',
		last_event_storage_id: 1,
		location_id: '0191f0aa-0000-7000-8000-000000000000',
		node_counts_json: {},
		priority: 0,
		queued_at: '2026-11-09T09:00:00Z',
		requested_by: 'system:scheduler',
		rewrite_history: false,
		selection_json: ['model.orders'],
		snapshot_id: '0191f0dd-0000-7000-8000-000000000000',
		target: 'development',
		...overrides
	};
}

function schedule(id: string, job: string, overrides: Partial<W['TriggerRecord']> = {}): W['TriggerRecord'] {
	return {
		trigger_id: id,
		name: `${job}_hourly`,
		job_key: job,
		status: 'running',
		definition_json: { description: '', job_key: job, schedule: { cron: '0 * * * *', timezone: 'UTC' } } as unknown as W['TriggerDeclaration'],
		...overrides
	} as W['TriggerRecord'];
}

const jobs = [
	{ job_key: 'orders_daily', kind: 'job' },
	{ job_key: 'nightly', kind: 'group' }
] as W['JobRecord'][];

describe('timeline', () => {
	const timeline = timelineOf(
		{
			runs: [
				run('ten-min', { started_at: '2026-11-09T09:00:00Z', ended_at: '2026-11-09T09:10:00Z' }),
				run('two-min', { started_at: '2026-11-09T10:00:00Z', ended_at: '2026-11-09T10:02:00Z' }),
				run('four-min', { started_at: '2026-11-09T10:30:00Z', ended_at: '2026-11-09T10:34:00Z' }),
				run('failed-long', { status: 'failed', started_at: '2026-11-09T09:20:00Z', ended_at: '2026-11-09T09:50:00Z' }),
				run('running', { status: 'running', started_at: '2026-11-09T10:50:00Z' }),
				run('before', { started_at: '2026-11-09T07:00:00Z', ended_at: '2026-11-09T07:30:00Z' }),
				run('child', { parent_run_id: 'p', root_run_id: 'p', started_at: '2026-11-09T09:00:00Z', ended_at: '2026-11-09T09:01:00Z' }),
				run('queued', { status: 'queued' })
			],
			jobs,
			triggers: [
				schedule('t1', 'orders_daily', { next_due_at: '2026-11-09T11:30:00Z' }),
				schedule('t2', 'nightly', { next_due_at: '2026-11-09T11:45:00Z' }),
				schedule('t3', 'orders_daily', { status: 'paused', next_due_at: '2026-11-09T11:15:00Z' }),
				schedule('t4', 'orders_daily', { next_due_at: '2026-11-09T13:00:00Z' })
			],
			ticks: [
				{ tick_id: 'k1', trigger_id: 't1', outcome: 'covered', covered_by_run_id: 'running', scheduled_for: '2026-11-09T10:55:00Z', epoch: 1, evaluated_at: '2026-11-09T10:55:01Z' },
				{ tick_id: 'k2', trigger_id: 't1', outcome: 'requested', run_id: 'two-min', scheduled_for: '2026-11-09T10:00:00Z', epoch: 1, evaluated_at: '2026-11-09T10:00:01Z' },
				{ tick_id: 'k3', trigger_id: 't1', outcome: 'covered', scheduled_for: '2026-11-09T07:55:00Z', epoch: 1, evaluated_at: '2026-11-09T07:55:01Z' }
			] as W['TickRecord'][]
		},
		LOCATION,
		WINDOW,
		NOW
	);

	it('shows started top-level runs overlapping the window, a running one up to now', () => {
		const real = timeline.bars.filter((b) => b.runId);
		expect(real.map((b) => b.runId)).toEqual(['ten-min', 'failed-long', 'two-min', 'four-min', 'running']);
		expect(real.at(-1)?.end).toBe('2026-11-09T11:00:00.000Z');
	});

	it("projects each running schedule's next tick inside the window, as long as the job's median successful run", () => {
		const projected = timeline.bars.filter((b) => b.status === 'projected');
		expect(projected).toEqual([
			{ runId: null, jobKey: 'orders_daily', kind: 'job', origin: 'schedule', status: 'projected', triggerName: 'orders_daily_hourly', start: '2026-11-09T11:30:00Z', end: '2026-11-09T11:34:00.000Z' },
			{ runId: null, jobKey: 'nightly', kind: 'group', origin: 'schedule', status: 'projected', triggerName: 'nightly_hourly', start: '2026-11-09T11:45:00Z', end: '2026-11-09T11:46:00.000Z' }
		]);
	});

	it('marks covered ticks inside the window only', () => {
		expect(timeline.markers.map((m) => [m.tickId, m.coveredByRunId, m.jobKey])).toEqual([['k1', 'running', 'orders_daily']]);
		expect(timeline.jobKeys).toEqual(['orders_daily', 'nightly']);
	});
});
