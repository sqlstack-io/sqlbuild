import { describe, expect, it } from 'vitest';
import { overviewOf } from './http-overview';
import type { CatalogNode, Check, NodeChange, PlatformEvent, Run, SnapshotChange } from './types';

const now = Date.parse('2026-11-09T12:00:00Z');
const hoursAgo = (h: number) => new Date(now - h * 3_600_000).toISOString();

function run(id: string, status: Run['status'], queuedHoursAgo: number, endedHoursAgo: number | null, parentRunId: string | null = null): Run {
	return { id, status, parentRunId, jobKey: 'orders', kind: 'job', statusReason: null, queuedAt: hoursAgo(queuedHoursAgo), startedAt: hoursAgo(queuedHoursAgo), endedAt: endedHoursAgo === null ? null : hoursAgo(endedHoursAgo) } as Run;
}

const node = (key: string, kind: CatalogNode['kind']) => ({ key, kind }) as CatalogNode;
const check = (id: string, outcome: string | null) => ({ id, latest: outcome ? { outcome } : null }) as unknown as Check;
const change = (publishedHoursAgo: number, nodes: [NodeChange['op'], string][]) =>
	({ snapshotId: `s-${publishedHoursAgo}`, publishedAt: hoursAgo(publishedHoursAgo), nodes: nodes.map(([op, key]) => ({ key, op })) }) as SnapshotChange;
const event = (id: string, active: boolean) => ({ id, active }) as PlatformEvent;

describe('overview from live reads', () => {
	// Newest queued first, as the runs listing returns them.
	const runs = [
		run('active', 'running', 1, null),
		run('child-failed', 'failed', 2, 2, 'group'),
		run('failed', 'failed', 3, 2.5),
		run('presumed', 'presumed_failed', 4, 3.5),
		run('partial', 'partial', 5, 4.5),
		run('failed-4th', 'failed', 6, 5.5),
		run('ok', 'success', 7, 0.5),
		run('stuck', 'unresponsive', 8, null),
		run('warn', 'warn', 23.5, 23),
		run('yesterday-failed', 'failed', 25, 24.5)
	];
	const overview = overviewOf({
		runs,
		checks: [check('a', 'pass'), check('b', 'fail'), check('c', 'error'), check('d', 'warn'), check('e', null)],
		nodes: [node('model:orders', 'table'), node('model:totals', 'view'), node('model:events', 'incremental'), node('source:raw', 'source'), node('seed:regions', 'seed')],
		changes: [change(1, [['added', 'model:orders'], ['added', 'seed:regions'], ['removed', 'model:old']]), change(24 * 8, [['added', 'model:totals']])],
		events: [event('scheduler', true), event('cleared', false)],
		now
	});

	it('counts top-level runs queued in the last 24 hours', () => {
		expect(overview.metrics.runs24h).toEqual({ total: 8, success: 1, warn: 1, partial: 1, failed: 3, active: 2 });
	});

	it('lists the three newest failed runs that ended today, then unresponsive runs and active platform events', () => {
		expect(overview.attention.map((i) => (i.kind === 'platform_event' ? `event:${i.event.id}` : 'run' in i ? `${i.kind}:${i.run.id}` : i.kind))).toEqual([
			'run_problem:failed',
			'run_problem:presumed',
			'run_problem:partial',
			'run_unresponsive:stuck',
			'event:scheduler'
		]);
	});

	it('counts models and this week’s model changes only', () => {
		expect(overview.metrics.models).toEqual({ total: 3, addedThisWeek: 1, removedThisWeek: 1 });
	});

	it('counts check outcomes, with errors as failing', () => {
		expect(overview.metrics.checks).toEqual({ total: 5, passing: 1, warning: 1, failing: 2 });
	});

	it('names the run that ended last, not the newest queued', () => {
		expect(overview.latestRun?.id).toBe('ok');
	});

	it('leaves parts without a server read empty instead of zero', () => {
		expect([overview.metrics.stale, overview.metrics.cost, overview.groups]).toEqual([null, null, null]);
	});
});
