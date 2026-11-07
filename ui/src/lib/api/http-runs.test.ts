import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { runDetailOf, runRowPage, runTabCountsOf, toRunNode, toRuns } from './http-runs';

type RunRecord = components['schemas']['RunRecord'];

const LOCATION = '0191f0aa-orders:development';
const PERSON_ID = '0191f0cc-0000-7000-8000-000000000001';

function wire(id: string, overrides: Partial<RunRecord> = {}): RunRecord {
	return {
		run_id: id,
		root_run_id: id,
		kind: 'job',
		origin: 'manual',
		status: 'success',
		job_key: 'orders_daily',
		last_event_storage_id: 1,
		location_id: '0191f0aa-0000-7000-8000-000000000000',
		node_counts_json: {},
		priority: 0,
		queued_at: '2026-11-06T10:00:00Z',
		requested_by: `principal:${PERSON_ID}`,
		requester_name: 'Ada Admin',
		rewrite_history: false,
		selection_json: ['model.orders'],
		snapshot_id: '0191f0dd-0000-7000-8000-000000000000',
		target: 'development',
		...overrides
	};
}

describe('run mapping', () => {
	it('shows a person by display name and a machine origin by its trigger', () => {
		const [person, sensor, scheduler] = toRuns(
			[
				wire('a'),
				wire('b', { origin: 'sensor', requested_by: 'system:sensor', requester_name: null, trigger_name: 'new_inventory_file' }),
				wire('c', { origin: 'schedule', requested_by: 'system:scheduler', requester_name: null })
			],
			LOCATION
		);
		expect(person.requestedBy).toEqual({ kind: 'user', name: PERSON_ID, displayName: 'Ada Admin' });
		expect(sensor.requestedBy).toEqual({ kind: 'sensor', name: 'new_inventory_file', displayName: 'new_inventory_file' });
		expect(scheduler.requestedBy).toEqual({ kind: 'scheduler', name: 'system:scheduler', displayName: 'system:scheduler' });
	});

	it('counts cancelled nodes as failed and counts each run’s direct children', () => {
		const runs = toRuns(
			[
				wire('group', { kind: 'group', node_counts_json: { success: 3, cancelled: 2, failed: 1, pending: 4 } }),
				wire('child-1', { parent_run_id: 'group', root_run_id: 'group' }),
				wire('child-2', { parent_run_id: 'group', root_run_id: 'group' }),
				wire('grandchild', { parent_run_id: 'child-1', root_run_id: 'group' })
			],
			LOCATION
		);
		expect(runs[0].nodeCounts).toEqual({ success: 3, warn: 0, failed: 3, skipped: 0, running: 0, pending: 4 });
		expect(runs.map((r) => r.childRunCount)).toEqual([2, 1, 0, 0]);
	});
});

describe('runs list', () => {
	const listing = [
		wire('r1', { status: 'running', cancel_requested_at: '2026-11-06T10:05:00Z' }),
		wire('r2', { status: 'cancelled', cancel_requested_at: '2026-11-06T09:05:00Z' }),
		wire('r3', { status: 'queued' }),
		wire('r4', { status: 'failed', node_counts_json: { success: 1, failed: 2, skipped: 3 } }),
		wire('r5', { status: 'partial' }),
		wire('r5-child', { status: 'queued', parent_run_id: 'r5', root_run_id: 'r5' }),
		wire('r6', { status: 'success' })
	];

	it('pages one tab with an offset cursor and hides children unless asked', () => {
		const first = runRowPage(listing, LOCATION, { tab: 'all', limit: 4 });
		expect(first.rows.map((r) => r.run.id)).toEqual(['r1', 'r2', 'r3', 'r4']);
		expect(first.nextCursor).toBe('4');
		const second = runRowPage(listing, LOCATION, { tab: 'all', limit: 4, cursor: first.nextCursor });
		expect(second.rows.map((r) => r.run.id)).toEqual(['r5', 'r6']);
		expect(second.nextCursor).toBeNull();
		expect(runRowPage(listing, LOCATION, { tab: 'failed' }).rows.map((r) => r.run.id)).toEqual(['r4', 'r5']);
		expect(runRowPage(listing, LOCATION, { parentRunId: 'r5' }).rows.map((r) => r.run.id)).toEqual(['r5-child']);
	});

	it('keeps a cancel request only while the run is active', () => {
		const rows = runRowPage(listing, LOCATION, { tab: 'all' }).rows;
		expect(rows.find((r) => r.run.id === 'r1')?.cancelRequestedAt).toBe('2026-11-06T10:05:00Z');
		expect(rows.find((r) => r.run.id === 'r2')?.cancelRequestedAt).toBeNull();
		expect(rows.find((r) => r.run.id === 'r4')?.rerunFromFailureCount).toBe(5);
	});

	it('counts active and queued runs under the filter, ignoring the tab', () => {
		expect(runTabCountsOf(listing, LOCATION, { tab: 'failed' })).toEqual({ active: 2, queued: 1 });
		expect(runTabCountsOf(listing, LOCATION, { includeChildren: true })).toEqual({ active: 3, queued: 2 });
		expect(runTabCountsOf(listing, LOCATION, { filter: { requestedBy: ['ada admin'] }, includeChildren: true })).toEqual({ active: 3, queued: 2 });
		expect(runTabCountsOf(listing, LOCATION, { filter: { requestedBy: ['someone else'] } })).toEqual({ active: 0, queued: 0 });
	});
});

describe('run detail', () => {
	it('links parent, children and reruns, and reruns from failed, skipped and cancelled nodes', () => {
		const wires = [
			wire('parent', { kind: 'group' }),
			wire('run', { parent_run_id: 'parent', root_run_id: 'parent', status: 'failed' }),
			wire('child-late', { parent_run_id: 'run', root_run_id: 'parent', queued_at: '2026-11-06T10:02:00Z' }),
			wire('child-early', { parent_run_id: 'run', root_run_id: 'parent', queued_at: '2026-11-06T10:01:00Z' }),
			wire('retry', { rerun_of_run_id: 'run' })
		];
		const runs = toRuns(wires, LOCATION);
		const run = runs[1];
		const nodes = (['success', 'failed', 'skipped', 'cancelled', 'bogus'] as const).map((status, i) =>
			toRunNode({ node_key: `model.n${i}`, status, skipped_because_of: status === 'skipped' ? 'model.n1' : null } as components['schemas']['RunNodeRecord'], run)
		);
		const detail = runDetailOf(run, wires[1], runs, nodes);
		expect(detail.parent?.id).toBe('parent');
		expect(detail.children.map((r) => r.id)).toEqual(['child-early', 'child-late']);
		expect(detail.reruns.map((r) => r.id)).toEqual(['retry']);
		expect(detail.rerunFromFailureNodes).toEqual(['model.n1', 'model.n2', 'model.n3']);
		expect(nodes[4].status).toBe('not_run');
		expect(nodes[2].skippedBecauseOf).toEqual({ nodeKey: 'model.n1', runId: 'run', jobKey: 'orders_daily' });
	});
});
