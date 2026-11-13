import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { findNode, nodeDetailOf, nodeJobsOf, toCatalogNodes, toColumnLineage, toMicrobatchCoverage, toNodeRuns } from './http-catalog';
import { toCheck } from './http-checks';

type W = components['schemas'];

function node(key: string, extra: Partial<W['CatalogNodeRecord']> = {}): W['CatalogNodeRecord'] {
	const [kind, name] = key.split(':');
	return { node_key: key, kind, name, tags: [], dependencies: [], definition_changed: false, snapshot_id: '01a1b000-0000-7000-8000-000000000001', ...extra };
}

describe('catalog node mapping', () => {
	it('derives the catalog kind from the model materialization and drops nodes the catalog does not list', () => {
		const nodes = toCatalogNodes([
			node('model:orders_view', { materialization: 'view' }),
			node('model:order_events', { materialization: 'incremental' }),
			node('model:order_partitions', { materialization: 'partition_tracked' }),
			node('seed:countries', { materialization: 'table' }),
			node('source:raw_orders'),
			node('audit:not_null:model:orders_view:order_id', { dependencies: ['model:orders_view'] }),
			node('macro:cents')
		]);
		expect(nodes.map((n) => [n.key, n.kind, n.materialization])).toEqual([
			['model:orders_view', 'view', 'view'],
			['model:order_events', 'incremental', 'incremental'],
			['model:order_partitions', 'table', 'partition_tracked'],
			['seed:countries', 'seed', null],
			['source:raw_orders', 'source', null]
		]);
	});

	it('keeps only upstream nodes that are listed, so graphs never point at a hidden node', () => {
		const [orders] = toCatalogNodes([
			node('model:orders', { dependencies: ['source:raw_orders', 'macro:cents', 'model:unpublished'] }),
			node('source:raw_orders'),
			node('macro:cents')
		]);
		expect(orders.upstream).toEqual(['source:raw_orders']);
	});

	it('reports the fingerprint as a sqb build and leaves unreported state unknown, not empty', () => {
		const [orders] = toCatalogNodes([
			node('model:orders', {
				materialization: 'table',
				description: 'One row per order',
				tags: ['marts'],
				column_count: 4,
				relation: { database: null, schema: 'main', name: 'orders' },
				last_build: { observed_at: '2026-11-07T06:00:00Z', run_id: 'sqb-run-1', version_hash: 'v1' }
			})
		]);
		expect(orders.lastBuild).toEqual({ fingerprintRunId: 'sqb-run-1', runId: null, jobKey: null, builtAt: '2026-11-07T06:00:00Z', fingerprint: 'v1' });
		expect([orders.description, orders.tags, orders.columnCount, orders.relation]).toEqual(['One row per order', ['marts'], 4, { database: null, schema: 'main', name: 'orders' }]);
		expect([orders.stale, orders.checks, orders.lastOutcome, orders.group, orders.owner]).toEqual([[], null, null, null, []]);
	});

	it('marks a changed definition, and a built table older than the data upstream of it, even through a view, as stale', () => {
		const built = (observed_at: string) => ({ last_build: { observed_at, run_id: 'sqb', version_hash: 'v' } });
		const table = (key: string, at: string | null, extra: Partial<W['CatalogNodeRecord']> = {}) =>
			node(key, { materialization: 'table', dependencies: ['model:orders', 'source:raw_orders'], ...(at ? built(at) : {}), ...extra });
		const nodes = toCatalogNodes([
			node('source:raw_orders'),
			table('model:orders', '2026-11-07T06:00:00Z', { dependencies: ['source:raw_orders'], definition_changed: true, last_outcome: { status: 'failed', at: '2026-11-07T06:30:00Z', run_id: 'r1' } }),
			table('model:order_totals', '2026-11-07T05:00:00Z', { last_outcome: { status: 'success', at: '2026-11-07T05:00:00Z' } }),
			table('model:order_daily', '2026-11-07T07:00:00Z'),
			table('model:order_view', '2026-11-07T04:00:00Z', { materialization: 'view' }),
			table('model:order_drafts', null, { definition_changed: true }),
			table('model:order_report', '2026-11-07T05:30:00Z', { dependencies: ['model:order_view'] })
		]);
		const by = Object.fromEntries(nodes.map((n) => [n.key, n]));
		expect(by['model:orders'].stale).toEqual([{ reason: 'changed', change: 'definition', detail: 'The definition changed since it was last built here.', since: '2026-11-07T06:00:00Z' }]);
		expect(by['model:order_totals'].stale).toEqual([{ reason: 'behind', upstream: ['model:orders'], detail: 'orders has newer data than this build.', since: '2026-11-07T06:00:00.000Z' }]);
		expect(by['model:order_report'].stale?.map((r) => r.reason === 'behind' && [r.upstream, r.since])).toEqual([[['model:order_view'], '2026-11-07T06:00:00.000Z']]);
		expect(['model:order_daily', 'model:order_view', 'model:order_drafts', 'source:raw_orders'].map((k) => by[k].stale)).toEqual([[], [], [], []]);
		expect(nodes.map((n) => n.lastOutcome)).toEqual([null, 'failed', 'success', null, null, null, null]);
	});

	it('finds a node by key, or by bare name preferring the model', () => {
		const nodes = toCatalogNodes([node('source:orders'), node('source:customers'), node('model:orders', { materialization: 'view' })]);
		expect([findNode(nodes, 'source:orders')?.key, findNode(nodes, 'orders')?.key, findNode(nodes, 'customers')?.key, findNode(nodes, 'order')]).toEqual(['source:orders', 'model:orders', 'source:customers', null]);
	});
});

function check(id: string, target: string, outcome: string): W['CheckResult'] {
	return { event_id: `e-${id}`, binding_key: id, audit_name: id, audit_definition_name: 'not_null', attachment_kind: 'model', attached_target_name: target, severity: 'error', evaluation_mode: 'violations', run_scope_phase: 'final', occurred_at: '2026-11-07T06:00:00Z', run_id: 'r', invocation_id: 'i', outcome, reused: 'false' };
}

function status(definition: W['NodeStatus']['definition'], physical: W['NodeStatus']['physical']): W['NodeStatus'] {
	const build = { definition_hash: 'd', event_id: 'e', observed_at: '2026-11-07T06:00:00Z', run_id: 'sqb-run-1', schema_fingerprint: 's', source_event_id: 'x', version_hash: 'v1' };
	return { node_key: 'model:orders', snapshot_id: 'snap', build, definition, physical, source_data: { status: 'unknown', reason: 'not tracked' } };
}

describe('catalog node detail', () => {
	const nodes = toCatalogNodes([node('model:orders', { materialization: 'table' }), node('model:order_totals', { materialization: 'view', dependencies: ['model:orders'] }), node('model:orders_by_day', { materialization: 'view' })]);
	const checks = [check('a', 'orders', 'pass'), check('b', 'model:orders', 'fail'), check('c', 'orders', 'warn'), check('d', 'orders_by_day', 'fail'), { ...check('e', 'orders', 'fail'), attachment_kind: 'source' }].map(toCheck);
	const observed: W['NodeStatus']['physical'] = { status: 'observed', observation: { event_id: 'o', observed_at: '2026-11-07T06:00:00Z', record: { name: 'orders', relation_type: 'table', state_schema: 'main', columns: [{ name: 'order_id', data_type: 'INTEGER' }] } } };

	it('keeps only its own checks, matched by key or by bare name of the same kind, and counts them by newest outcome', () => {
		const d = nodeDetailOf(nodes, nodes[0], null, null, checks);
		expect(d.checkResults.map((c) => c.id)).toEqual(['a', 'b', 'c']);
		expect(d.checks).toEqual({ total: 3, passing: 1, warning: 1, failing: 1 });
		expect(d.downstream).toEqual(['model:order_totals']);
	});

	it('keeps the listing staleness and maps the warehouse columns', () => {
		const [listed] = toCatalogNodes([node('model:orders', { materialization: 'table', definition_changed: true, last_build: { observed_at: '2026-11-07T06:00:00Z', run_id: 'sqb', version_hash: 'v' } })]);
		const changed = nodeDetailOf(nodes, listed, status({ status: 'changed', current_local_hash: 'b', recorded_local_hash: 'a' }, observed), null, []);
		expect(changed.stale.map((r) => [r.reason, r.since])).toEqual([['changed', '2026-11-07T06:00:00Z']]);
		expect(changed.columns).toEqual([{ name: 'order_id', type: 'INTEGER', description: '', audits: [] }]);
		const matched = nodeDetailOf(nodes, nodes[0], status({ status: 'matched', local_hash: 'a' }, { status: 'unavailable', reason: 'no catalog' }), null, []);
		expect([matched.stale, matched.columns]).toEqual([[], []]);
	});

	it('leaves refused SQL out and unreported history unknown rather than empty', () => {
		const refused = nodeDetailOf(nodes, nodes[0], null, null, []);
		expect([refused.sql, refused.runs, refused.jobs, refused.history]).toEqual([null, null, null, null]);
		const withSql = nodeDetailOf(nodes, nodes[0], null, { node_key: 'model:orders', snapshot_id: 'snap', raw_sql: 'select {{ x }}', compiled_sql: null }, []);
		expect(withSql.sql).toEqual({ raw: 'select {{ x }}', compiled: 'select {{ x }}' });
	});

	it('builds the warehouse object from the synced relation and its newest sample, and counts coverage holes', () => {
		const [stored] = toCatalogNodes([node('model:orders', { materialization: 'table', relation: { schema: 'main', name: 'orders' } })]);
		const sample = (observed_at: string, row_count: number | null, byte_count: number | null): W['WarehouseStatisticsRecord'] => ({ name: 'orders', state_schema: 'main', observed_at, row_count, byte_count });
		const statistics = [sample('2026-11-07T01:00:00Z', 10, 4096), sample('2026-11-07T02:00:00Z', null, null), sample('2026-11-07T03:00:00Z', 12, null)];
		const gap = { start: '20', end: '30', state: 'gap' as const };
		const coverage = toMicrobatchCoverage({
			model: 'orders', target_name: 'development', physical_generation_id: 'g', cursor_column: 'order_id', cursor_type: 'integer', batch_size: '10',
			earliest: '0', contiguous_frontier: '20', known_gaps: [gap, { ...gap, start: '50', end: '60' }], batches_truncated: false,
			batches: [{ start: '0', end: '10', state: 'complete', run_id: 'r1' }, { start: '10', end: '20', state: 'complete' }, gap]
		});
		const baseTable: W['NodeStatus']['physical'] = {
			status: 'observed',
			observation: { event_id: 'o', observed_at: '2026-11-07T06:00:00Z', record: { name: 'orders', relation_type: 'BASE TABLE', state_schema: 'main', columns: [{ name: 'order_id', data_type: 'INTEGER' }] } }
		};
		const d = nodeDetailOf([stored], stored, status({ status: 'matched', local_hash: 'a' }, baseTable), null, [], { statistics, microbatch: coverage });
		expect(d.warehouse).toEqual({
			relation: { database: null, schema: 'main', name: 'orders' },
			objectType: 'table',
			createdAt: null,
			lastAlteredAt: null,
			rows: 12,
			bytes: null,
			columns: [{ name: 'order_id', declaredType: null, actualType: 'INTEGER', state: null }],
			stats: [
				{ at: '2026-11-07T01:00:00Z', rows: 10, bytes: 4096 },
				{ at: '2026-11-07T03:00:00Z', rows: 12, bytes: null }
			],
			syncedAt: '2026-11-07T06:00:00Z'
		});
		expect(d.lastRows).toBe(12);
		expect([d.microbatch?.knownGaps, d.microbatch?.batches.map((b) => b.runId)]).toEqual([2, ['r1', null, null]]);
		const unsynced = nodeDetailOf([stored], stored, status({ status: 'matched', local_hash: 'a' }, { status: 'unavailable', reason: 'no catalog' }), null, [], { statistics });
		expect([unsynced.warehouse, unsynced.microbatch]).toEqual([null, null]);
	});
});

describe('catalog node runs and jobs', () => {
	const snapshot = '01a1b000-0000-7000-8000-000000000001';
	const job = (key: string, kind: 'job' | 'group', selection: string[]): W['JobRecord'] => ({ job_key: key, kind, description: key, selection, concurrency_groups: [], snapshot_id: snapshot, overlap: 'queue' });
	const trigger = (id: string, jobKey: string) => ({ trigger_id: id, job_key: jobKey }) as W['TriggerRecord'];
	const run = (id: string, status: string, extra: Partial<W['NodeRunRecord']> = {}): W['NodeRunRecord'] => ({ run_id: id, job_key: 'orders', queued_at: '2026-11-08T05:59:00Z', status, ...extra });

	it('lists the jobs and groups that select the node with their own triggers only', () => {
		const jobs = [job('orders', 'job', ['model:orders']), job('nightly', 'group', ['model:orders', 'model:customers']), job('customers', 'job', ['model:customers'])];
		const jobsOfOrders = nodeJobsOf('model:orders', jobs, [trigger('t1', 'orders'), trigger('t2', 'customers'), trigger('t3', 'orders')]);
		expect(jobsOfOrders).toEqual([
			{ key: 'orders', kind: 'job', triggerIds: ['t1', 't3'] },
			{ key: 'nightly', kind: 'group', triggerIds: [] }
		]);
	});

	it('keeps finished builds newest first with duration and the reason a node failed or was skipped', () => {
		const runs = toNodeRuns([
			run('r4', 'running', { started_at: '2026-11-08T06:00:00Z' }),
			run('r3', 'skipped', { skipped_because_of: 'model:stg_orders' }),
			run('r2', 'failed', { started_at: '2026-11-08T05:00:00Z', ended_at: '2026-11-08T05:00:01.5Z', error_code: 'warehouse_error' }),
			run('r1', 'success', { job_key: null, started_at: '2026-11-08T04:00:00Z', ended_at: '2026-11-08T04:00:30Z', rows_affected: 12 }),
			run('r0', 'cancelled')
		]);
		expect(runs.map((r) => [r.runId, r.outcome, r.jobKey, r.durationMs, r.rows, r.message])).toEqual([
			['r3', 'skipped', 'orders', null, null, 'Skipped because stg_orders did not succeed'],
			['r2', 'failed', 'orders', 1500, null, 'warehouse_error'],
			['r1', 'success', null, 30000, 12, null]
		]);
		expect(runs[0].startedAt).toBe('2026-11-08T05:59:00Z');
		const nodes = toCatalogNodes([node('model:orders', { materialization: 'table' })]);
		const d = nodeDetailOf(nodes, nodes[0], null, null, [], { runs, jobs: [] });
		expect([d.recentOutcomes, d.lastDurationMs, d.jobs]).toEqual([['success', 'failed', 'skipped'], 1500, []]);
	});
});

describe('column lineage mapping', () => {
	const nodes = toCatalogNodes([node('model:raw_orders', { materialization: 'view' }), node('model:orders', { materialization: 'table' })]);
	const edge = (source: string, target: string, transform: string): W['ColumnLineageEdge'] => ({ source_key: 'model:raw_orders', source_column: source, target_key: 'model:orders', target_column: target, transform });

	it('keeps a name through direct or star as a passthrough, a new name as a rename, and names any other transform', () => {
		const lineage = toColumnLineage(
			{
				snapshot_id: 's',
				nodes: [
					{ node_key: 'model:raw_orders', kind: 'model', columns: [{ name: 'amount', data_type: null }] },
					{ node_key: 'model:orders', kind: 'model', columns: [{ name: 'amount', data_type: 'DECIMAL(10,2)' }] },
					{ node_key: 'model:retired', kind: 'model', columns: [] }
				],
				edges: [edge('id', 'id', 'direct'), edge('note', 'note', 'star'), edge('cust', 'customer_id', 'direct'), edge('amount', 'amount', 'cast')]
			},
			nodes
		);
		expect(lineage?.edges.map(([, target, lens, label]) => [target, lens, label])).toEqual([
			['model:orders.id', 'passthrough', undefined],
			['model:orders.note', 'passthrough', undefined],
			['model:orders.customer_id', 'rename', undefined],
			['model:orders.amount', 'transform', 'Cast']
		]);
		expect(lineage?.models.map((m) => [m.id, m.name, m.kind, m.columns])).toEqual([
			['model:raw_orders', 'raw_orders', 'view', [{ name: 'amount', type: '' }]],
			['model:orders', 'orders', 'table', [{ name: 'amount', type: 'DECIMAL(10,2)' }]]
		]);
		expect([lineage?.sql, lineage?.expressions]).toEqual([{}, {}]);
	});

	it('has nothing to show when no lineage reaches the node', () => {
		expect(toColumnLineage({ snapshot_id: 's', nodes: [], edges: [] }, nodes)).toBeNull();
	});
});
