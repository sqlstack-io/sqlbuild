import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { findNode, nodeDetailOf, nodeJobsOf, toCatalogNodes, toNodeRuns } from './http-catalog';
import { toCheck } from './http-checks';

type W = components['schemas'];

function node(key: string, extra: Partial<W['CatalogNodeRecord']> = {}): W['CatalogNodeRecord'] {
	const [kind, name] = key.split(':');
	return { node_key: key, kind, name, tags: [], dependencies: [], snapshot_id: '01a1b000-0000-7000-8000-000000000001', ...extra };
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

	it('reports the fingerprint as a sqb build and leaves unreported build state unknown, not empty', () => {
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
		expect([orders.stale, orders.checks, orders.lastOutcome, orders.group, orders.owner]).toEqual([null, null, null, null, []]);
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

	it('reports a changed definition as stale and maps the warehouse columns', () => {
		const changed = nodeDetailOf(nodes, nodes[0], status({ status: 'changed', current_local_hash: 'b', recorded_local_hash: 'a' }, observed), null, []);
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
});

describe('catalog node runs and jobs', () => {
	const snapshot = '01a1b000-0000-7000-8000-000000000001';
	const job = (key: string, kind: 'job' | 'group', selection: string[]): W['JobRecord'] => ({ job_key: key, kind, description: key, selection, concurrency_groups: [], snapshot_id: snapshot });
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
