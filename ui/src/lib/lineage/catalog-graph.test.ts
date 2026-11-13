import { describe, expect, it } from 'vitest';
import type { CatalogNode } from '$lib/api';
import { folderOf, groupedGraph } from './catalog-graph';

function node(key: string, path: string, upstream: string[] = [], extra: Partial<CatalogNode> = {}): CatalogNode {
	const kind = key.startsWith('source:') ? 'source' : key.startsWith('seed:') ? 'seed' : 'table';
	return { key, kind, path, upstream, group: null, stale: [], lastOutcome: null, lastBuild: { builtAt: '2026-11-07T06:00:00Z' }, ...extra } as CatalogNode;
}

describe('lineage folders', () => {
	it('groups models by folder, sources by file, and lets a declared group win', () => {
		expect([
			folderOf(node('model:dim_orders', 'models/marts/core/dim_orders.sql')),
			folderOf(node('source:shop__orders', 'sources/shop.yml')),
			folderOf(node('seed:countries', 'seeds/countries.csv')),
			folderOf(node('model:orders', 'orders.sql')),
			folderOf(node('model:stg_orders', 'models/staging/stg_orders.sql', [], { group: 'shop' }))
		]).toEqual(['marts/core', 'sources/shop', 'seeds', 'models', 'shop']);
	});

	const nodes = [
		node('source:shop__orders', 'sources/shop.yml'),
		node('source:shop__customers', 'sources/shop.yml'),
		node('model:stg_orders', 'models/staging/shop/stg_orders.sql', ['source:shop__orders']),
		node('model:stg_customers', 'models/staging/crm/stg_customers.sql', ['source:shop__customers'], { stale: [{ reason: 'behind', upstream: [], detail: '', since: '' }] }),
		node('model:orders', 'models/marts/orders.sql', ['model:stg_orders', 'model:stg_customers'], { lastOutcome: 'failed' }),
		node('model:order_totals', 'models/marts/order_totals.sql', ['model:orders'])
	];

	it('draws each closed top folder once, with one edge per folder pair and none inside a folder', () => {
		const graph = groupedGraph(nodes, new Set());
		expect(graph.nodes.map((n) => [n.id, n.data.count])).toEqual([
			['group:sources', 2],
			['group:staging', 2],
			['group:marts', 2]
		]);
		expect(graph.edges.map((e) => e.id)).toEqual(['group:sources-group:staging', 'group:staging-group:marts']);
		expect(graph.nodes.map((n) => [n.data.ok, n.data.stale, n.data.failed])).toEqual([[0, 0, 0], [1, 1, 0], [1, 0, 1]]);
	});

	it('opens a folder into the folders inside it, and those into their nodes, joined to the closed folders around them', () => {
		expect(groupedGraph(nodes, new Set(['staging'])).nodes.map((n) => n.id)).toEqual(['group:sources', 'group:staging/shop', 'group:staging/crm', 'group:marts']);
		const graph = groupedGraph(nodes, new Set(['staging', 'staging/shop']));
		expect(graph.nodes.map((n) => n.id)).toEqual(['group:sources', 'model:stg_orders', 'group:staging/crm', 'group:marts']);
		expect(graph.edges.map((e) => e.id)).toEqual([
			'group:sources-model:stg_orders',
			'group:sources-group:staging/crm',
			'model:stg_orders-group:marts',
			'group:staging/crm-group:marts'
		]);
	});

	it('draws every node once all folders around it are open', () => {
		const graph = groupedGraph(nodes, new Set(['sources', 'sources/shop', 'staging', 'staging/shop', 'staging/crm', 'marts']));
		expect(graph.nodes.every((n) => n.type === 'asset')).toBe(true);
		expect(graph.edges).toHaveLength(5);
	});
});
