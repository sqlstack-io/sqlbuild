import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { findNode, nodeDetailOf, toCatalogNodes, toColumnLineage } from './http-catalog';
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
