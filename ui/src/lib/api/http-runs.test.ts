import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { groupRunNodes, parseRunCursor, queueEntriesOf, planResultOf, runDetailOf, runRowPage, runRowPageOver, runTabCountsOf, toRunNode, toRuns, toRunEvent, toRunStatements, toRunOutput } from './http-runs';

type RunRecord = components['schemas']['RunRecord'];
type W = components['schemas'];

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

	it('keeps how a run of part of a job was narrowed, and nothing for a whole-job run', () => {
		const [part, whole] = toRuns([wire('p', { narrowing: { select: [], exclude: ['customers'] } }), wire('w')], LOCATION);
		expect([part.narrowing, whole.narrowing]).toEqual([{ select: [], exclude: ['customers'] }, null]);
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
		wire('r6', { status: 'success' }),
		wire('r7', { status: 'cancelled', node_counts_json: { success: 1, failed: 1, cancelled: 2, skipped: 1 } })
	];

	it('pages one tab with an offset cursor and hides children unless asked', () => {
		const first = runRowPage(listing, LOCATION, { tab: 'all', limit: 4 });
		expect(first.rows.map((r) => r.run.id)).toEqual(['r1', 'r2', 'r3', 'r4']);
		expect(first.nextCursor).toBe('4');
		const second = runRowPage(listing, LOCATION, { tab: 'all', limit: 4, cursor: first.nextCursor });
		expect(second.rows.map((r) => r.run.id)).toEqual(['r5', 'r6', 'r7']);
		expect(second.nextCursor).toBeNull();
		expect(runRowPage(listing, LOCATION, { tab: 'failed' }).rows.map((r) => r.run.id)).toEqual(['r4', 'r5']);
		expect(runRowPage(listing, LOCATION, { parentRunId: 'r5' }).rows.map((r) => r.run.id)).toEqual(['r5-child']);
	});

	it('keeps a cancel request only while the run is active', () => {
		const rows = runRowPage(listing, LOCATION, { tab: 'all' }).rows;
		expect(rows.find((r) => r.run.id === 'r1')?.cancelRequestedAt).toBe('2026-11-06T10:05:00Z');
		expect(rows.find((r) => r.run.id === 'r2')?.cancelRequestedAt).toBeNull();
		expect(rows.find((r) => r.run.id === 'r4')?.rerunFromFailureCount).toBe(5);
		expect(rows.find((r) => r.run.id === 'r7')?.rerunFromFailureCount).toBe(2);
	});

	it('counts active and queued runs under the filter, ignoring the tab', () => {
		expect(runTabCountsOf(listing, LOCATION, { tab: 'failed' })).toEqual({ active: 2, queued: 1 });
		expect(runTabCountsOf(listing, LOCATION, { includeChildren: true })).toEqual({ active: 3, queued: 2 });
		expect(runTabCountsOf(listing, LOCATION, { filter: { requestedBy: ['ada admin'] }, includeChildren: true })).toEqual({ active: 3, queued: 2 });
		expect(runTabCountsOf(listing, LOCATION, { filter: { requestedBy: ['someone else'] } })).toEqual({ active: 0, queued: 0 });
	});
});

describe('backfill window', () => {
	it('shows both ends of an integer or timestamp window as typed, and none for other runs', () => {
		const [integer, timestamp, plain] = toRuns(
			[
				wire('i', { origin: 'backfill', window: { kind: 'integer', start: -3, end: 11 } }),
				wire('t', { window: { kind: 'timestamp', start: '2026-10-01T00:00:00Z', end: '2026-10-31T00:00:00Z' } }),
				wire('p')
			],
			LOCATION
		);
		expect(integer.window).toEqual({ start: '-3', end: '11' });
		expect(timestamp.window).toEqual({ start: '2026-10-01T00:00:00Z', end: '2026-10-31T00:00:00Z' });
		expect(plain.window).toBeNull();
	});
});

describe('run queue', () => {
	it('keeps claim order, drops runs that started meanwhile and never names a run elsewhere', () => {
		const entries = queueEntriesOf(
			[
				wire('urgent', { status: 'queued', queue: { position: 1, waits: [{ kind: 'concurrency_limit', limit: 'warehouse', holders: ['held'], elsewhere: 2 }] } }),
				wire('started', { status: 'queued' }),
				wire('blocked', {
					status: 'queued',
					queue: {
						position: 2,
						waits: [
							{ kind: 'write_claim', run_id: null, node_key: 'model.orders', other_nodes: 3 },
							{ kind: 'earlier_run', run_id: 'urgent', limit: 'warehouse' },
							{ kind: 'upstream_job', run_id: 'stage', job_key: 'staging' },
							{ kind: 'job_running', run_id: 'same' },
							{ kind: 'write_claim', run_id: 'same', node_key: 'model.customers', other_nodes: 0 }
						]
					}
				})
			],
			LOCATION
		);
		expect(entries.map((q) => [q.run.id, q.position])).toEqual([
			['urgent', 1],
			['blocked', 2]
		]);
		expect(entries[0].waits).toEqual([{ kind: 'concurrency_limit', limit: 'warehouse', holders: ['held'], elsewhere: 2 }]);
		expect(entries[1].waits).toEqual([
			{ kind: 'write_claim', runId: null, nodeKey: 'model.orders', otherNodes: 3 },
			{ kind: 'earlier_run', runId: 'urgent', limit: 'warehouse' },
			{ kind: 'upstream_job', runId: 'stage', jobKey: 'staging' },
			{ kind: 'job_running', runId: 'same' }
		]);
	});
});

describe('runs past the newest window', () => {
	const runWindow = (prefix: string, size: number) => Array.from({ length: size }, (_, i) => wire(`${prefix}${i}`));
	const ids = (page: { rows: { run: { id: string } }[] }) => page.rows.map((r) => r.run.id);

	it('reads the window count after the offset', () => {
		expect(parseRunCursor(null)).toEqual({ offset: 0, older: 0 });
		expect(parseRunCursor('7')).toEqual({ offset: 7, older: 0 });
		expect(parseRunCursor('1000+2')).toEqual({ offset: 1000, older: 2 });
	});

	it('stops at the end of a window shorter than a full one', () => {
		const page = runRowPageOver([runWindow('n', 30)], LOCATION, { tab: 'all', limit: 20, cursor: '20' });
		expect(ids(page)).toEqual(Array.from({ length: 10 }, (_, i) => `n${i + 20}`));
		expect(page.nextCursor).toBeNull();
	});

	it('asks for one more window once a full window runs out', () => {
		const newest = runWindow('n', 1000);
		const first = runRowPageOver([newest], LOCATION, { tab: 'all', limit: 600 });
		expect(first.nextCursor).toBe('600');
		const second = runRowPageOver([newest], LOCATION, { tab: 'all', limit: 600, cursor: first.nextCursor });
		expect(second.rows).toHaveLength(400);
		expect(second.nextCursor).toBe('1000+1');
	});

	it('pages into the older window and keeps its count on the cursor', () => {
		const windows = [runWindow('n', 1000), runWindow('o', 1000)];
		const page = runRowPageOver(windows, LOCATION, { tab: 'all', limit: 600, cursor: '1000+1' });
		expect(ids(page)[0]).toBe('o0');
		expect(ids(page)).toHaveLength(600);
		expect(page.nextCursor).toBe('1600+1');
		const last = runRowPageOver([runWindow('n', 1000), runWindow('o', 3)], LOCATION, { tab: 'all', limit: 600, cursor: '1000+1' });
		expect(ids(last)).toEqual(['o0', 'o1', 'o2']);
		expect(last.nextCursor).toBeNull();
	});
});

describe('run detail', () => {
	it('links parent, children and reruns, and reruns from failed and skipped nodes but not cancelled ones', () => {
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
		expect(detail.rerunFromFailureNodes).toEqual(['model.n1', 'model.n2']);
		expect(nodes[4].status).toBe('not_run');
		expect(nodes[2].skippedBecauseOf).toEqual({ nodeKey: 'model.n1', runId: 'run', jobKey: 'orders_daily' });
	});
});

describe('run Recall reads', () => {
	const inv = 'worker/c1/execution';
	const event = (type: string, extra: Partial<W['RunEventRecord']> = {}): W['RunEventRecord'] => ({ storage_id: 1, event_id: `e-${type}`, invocation_id: inv, occurred_at: '2026-11-08T06:00:00Z', event_type: type, ...extra });

	it('maps scheduler facts and sqb events, builds messages from metadata only and drops other kinds', () => {
		const mapped = [
			event('run_claimed', { invocation_id: 'scheduler/r1' }),
			event('statement_completed', { duration_ms: 1500, affected_rows: 1, resource_id: `["${inv}","model:orders"]`, statement_id: 's1' }),
			event('resource_attempt_failed', { severity: 'error', error_type: 'ExecutionFailed', error_code: 'R002', resource_id: `["${inv}","model:orders"]` }),
			event('operation_started'),
			event('location_health', { invocation_id: 'scheduler/r1' })
		].flatMap((e) => toRunEvent(e, 'r1') ?? []);
		expect(mapped.map((e) => [e.type, e.producer, e.level, e.nodeKey, e.statementId, e.message])).toEqual([
			['claimed', 'scheduler', 'info', null, null, ''],
			['statement_completed', 'sqb', 'info', 'model:orders', 's1', '1.5 s · 1 row'],
			['resource_attempt_failed', 'sqb', 'error', 'model:orders', null, 'ExecutionFailed: R002']
		]);
	});

	it('orders statements as sent and tells running from finished by the end time', () => {
		const statement = (id: string, extra: Partial<W['RunStatementRecord']>): W['RunStatementRecord'] => ({ invocation_id: inv, statement_id: id, sql_available: true, ...extra });
		const statements = toRunStatements(
			[
				statement('s1', { resource_id: `["${inv}","seed:countries"]`, statement_kind: 'CREATE', ended_at: '2026-11-08T06:00:01Z', affected_rows: 3 }),
				statement('s2', { resource_name: 'orders', outcome: 'failed', error_type: 'CatalogException', sql_available: false }),
				statement('s3', { purpose: 'model_build', started_at: '2026-11-08T06:00:02Z' })
			],
			'r1'
		);
		expect(statements.map((s) => [s.sequence, s.nodeKey, s.kind, s.status, s.rowsAffected, s.sqlRecorded, s.error])).toEqual([
			[1, 'seed:countries', 'CREATE', 'completed', 3, true, null],
			[2, 'orders', 'statement', 'failed', null, false, 'CatalogException'],
			[3, '', 'model_build', 'running', null, true, null]
		]);
	});

	it('merges error text into the output as stderr in time order', () => {
		const output = toRunOutput({
			lines: [
				{ invocation_id: inv, sequence: 2, stream: 'stdout', occurred_at: '2026-11-08T06:00:03Z', text: 'done' },
				{ invocation_id: inv, sequence: 1, stream: 'stderr', occurred_at: '2026-11-08T06:00:01Z', text: 'warning' }
			],
			errors: [{ invocation_id: inv, occurred_at: '2026-11-08T06:00:02Z', error_type: 'ValueError', message: 'RuntimeError: bad input' }]
		});
		expect(output.map((l) => [l.stream, l.text])).toEqual([
			['stderr', 'warning'],
			['stderr', 'RuntimeError: bad input'],
			['stdout', 'done']
		]);
	});
});


describe('group run nodes', () => {
	it("keeps each job run's node and the group's own row only where no job run has the node", () => {
		const group = toRuns([wire('g', { kind: 'group', job_key: 'nightly' })], LOCATION)[0];
		const staging = toRuns([wire('s', { parent_run_id: 'g', job_key: 'staging' })], LOCATION)[0];
		const node = (key: string, status: string, owner = group) => toRunNode({ node_key: key, status } as W['RunNodeRecord'], owner);
		const nodes = groupRunNodes([node('model:raw', 'success'), node('model:report', 'pending')], [node('model:raw', 'failed', staging)]);
		expect(nodes.map((n) => [n.nodeKey, n.runId, n.status])).toEqual([
			['model:report', 'g', 'pending'],
			['model:raw', 's', 'failed']
		]);
	});
});

describe('planResultOf', () => {
	it.each([
		{ description: 'a plan keeps its models and fills missing sections', outcome: { kind: 'plan', document: { selected_count: 1, models: [{ name: 'orders', reason: 'first_run' }] } }, expected: { plan: 'orders', error: null } },
		{ description: "sqb plan's own error is the result", outcome: { kind: 'plan', document: { error: { code: 'S007', message: "unknown selector name 'ordrs'" } } }, expected: { plan: null, error: "S007: unknown selector name 'ordrs'" } },
		{ description: 'a superseded plan asks to plan again', outcome: { kind: 'superseded' }, expected: { plan: null, error: 'The project changed on the location while it was planning; plan again' } }
	])('$description', ({ outcome, expected }) => {
		const result = planResultOf(outcome as never);
		expect(result.error).toBe(expected.error);
		expect(result.plan?.models[0]?.name ?? null).toBe(expected.plan);
		if (result.plan) expect(result.plan.warnings).toEqual([]);
	});
});
