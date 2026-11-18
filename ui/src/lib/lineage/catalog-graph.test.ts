import { describe, expect, it } from 'vitest';
import type { CatalogNode } from '$lib/api';
import { folderOf } from './catalog-graph';

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
});
