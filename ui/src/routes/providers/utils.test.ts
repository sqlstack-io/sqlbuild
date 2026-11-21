import { describe, expect, it } from 'vitest';
import type { ProviderConnection } from '$lib/api';
import { kindCount, nodeHref, usedByGraph, usedBySentence, usedCount } from './utils';

type Used = Pick<ProviderConnection, 'nodes' | 'jobs' | 'jobNodes' | 'usedBy'>;

const provider: Used = {
	usedBy: [
		{ kind: 'loader', name: 'shop_loader' },
		{ kind: 'asset', name: 'inventory_snapshot' },
		{ kind: 'check', name: 'orders_fresh' },
		{ kind: 'hook', name: 'notify_orders' },
		{ kind: 'sink', name: 'order_events' }
	],
	nodes: ['source:orders', 'source:customers', 'asset:inventory_snapshot', 'check:orders_fresh', 'model:orders_daily'],
	jobs: ['load_shop', 'inventory_job'],
	jobNodes: {
		load_shop: ['source:orders', 'source:customers', 'check:orders_fresh'],
		inventory_job: ['asset:inventory_snapshot', 'check:orders_fresh']
	}
};

describe('usedByGraph', () => {
	it('groups the nodes by kind in Catalog order, each with the jobs that run it', () => {
		const graph = usedByGraph(provider, (key) => (key.startsWith('source:') ? 'loader' : null));
		expect(graph.groups.map((g) => [g.label, g.nodes, g.jobs])).toEqual([
			['Loaders', ['source:customers', 'source:orders'], ['load_shop']],
			['Models', ['model:orders_daily'], []],
			['Assets', ['asset:inventory_snapshot'], ['inventory_job']],
			['Checks', ['check:orders_fresh'], ['load_shop', 'inventory_job']]
		]);
		expect(graph.jobs).toEqual(['load_shop', 'inventory_job']);
	});

	it('keeps code without a node of its own aside', () => {
		expect(usedByGraph(provider).others.map((c) => c.name)).toEqual(['notify_orders', 'order_events']);
	});

	it('falls back to the consumer of the same name for keys without a kind', () => {
		const graph = usedByGraph({ usedBy: [{ kind: 'asset', name: 'export_orders' }], nodes: [], jobs: ['exports'], jobNodes: { exports: ['export_orders'] } });
		expect(graph.groups.map((g) => [g.kind, g.nodes, g.jobs])).toEqual([['asset', ['export_orders'], ['exports']]]);
	});

	it('is empty for an unused provider', () => {
		expect(usedByGraph({ usedBy: [], nodes: [], jobs: [], jobNodes: {} })).toEqual({ groups: [], jobs: [], others: [] });
	});
});

describe('usedBySentence', () => {
	it('counts each kind and the jobs', () => {
		const graph = usedByGraph({ ...provider, usedBy: [] });
		expect(usedBySentence('shop_api', graph)).toBe('shop_api is used by 2 sources, 1 model, 1 asset, 1 check, run by 2 jobs.');
	});

	it('names code that has no node and says when no job runs it', () => {
		const graph = usedByGraph({ usedBy: [{ kind: 'sink', name: 'order_events' }], nodes: [], jobs: [], jobNodes: {} });
		expect(usedBySentence('shop_api', graph)).toBe('shop_api is used by sink order_events, not run by any job.');
		expect(usedBySentence('shop_api', usedByGraph({ usedBy: [], nodes: [], jobs: [], jobNodes: {} }))).toBe('Nothing uses shop_api yet.');
	});

	it('words kinds in the singular and plural', () => {
		expect([kindCount('asset', 1), kindCount('incremental', 3), kindCount('event_thing', 2)]).toEqual(['1 asset', '3 incremental models', '2 event things']);
	});
});

describe('nodeHref', () => {
	it('links nodes to the Catalog and checks to the Checks list', () => {
		expect(nodeHref('source:orders', 'loader')).toBe('/catalog/source:orders');
		expect(nodeHref('check:orders_fresh', 'check')).toBe('/checks?q=orders_fresh');
	});
});

describe('usedCount', () => {
	it('counts the nodes, the code without a node and the jobs', () => {
		expect(usedCount({ ...provider, name: 'shop_api', className: '', description: '', sourceFile: '', settings: [] })).toBe(5 + 2 + 2);
	});
});
