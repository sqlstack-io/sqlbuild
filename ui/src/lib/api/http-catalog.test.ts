import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { toCatalogNodes } from './http-catalog';

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
});
