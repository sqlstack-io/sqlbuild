import { withServerStatus } from '$lib/build-area/status-fixtures';
import { describe, expect, it } from 'vitest';
import type { CatalogNode } from '$lib/api';
import { cardData, folderOf } from './catalog-graph';

function node(key: string, path: string, upstream: string[] = [], extra: Partial<CatalogNode> = {}): CatalogNode {
	const kind = key.startsWith('source:') ? 'source' : key.startsWith('seed:') ? 'seed' : 'table';
	return withServerStatus({ key, kind, path, upstream, group: null, stale: [], lastOutcome: null, lastBuild: { builtAt: '2026-11-07T06:00:00Z' }, ...extra } as CatalogNode);
}

describe('lineage folders', () => {
	it('groups models below sql/models, other SQL below sql, Python below python/nodes, and lets a declared group win', () => {
		expect([
			folderOf(node('model:dim_orders', 'sql/models/marts/core/dim_orders.sql')),
			folderOf(node('model:orders', 'sql/models/orders.sql')),
			folderOf(node('source:shop__orders', 'sql/sources/shop.yml')),
			folderOf(node('seed:countries', 'sql/seeds/countries.csv')),
			folderOf(node('seed:regions', 'sql/seeds/geo/regions.csv')),
			folderOf(node('udf:clean', 'sql/functions/clean.sql', [], { kind: 'udf' })),
			folderOf(node('udf:to_cents', 'sql/models/staging/_sqlbuild/functions/to_cents.sql', [], { kind: 'udf' })),
			folderOf(node('asset:export_csv', 'python/nodes/exports/export_csv.py', [], { kind: 'asset' })),
			folderOf(node('asset:score', 'python/nodes/score.py', [], { kind: 'asset' })),
			folderOf(node('model:stg_orders', 'sql/models/staging/stg_orders.sql', [], { group: 'shop' }))
		]).toEqual(['marts/core', 'models', 'sources/shop', 'seeds', 'seeds/geo', 'functions', 'staging/_sqlbuild/functions', 'python/exports', 'python', 'shop']);
	});
});

describe('a card while its job runs', () => {
	it('shows the node’s state in the active run, which outranks its last build and whether it was stale', () => {
		const stale = node('seed:order_types', 'seeds/order_types.csv', [], { lastOutcome: 'success', lastBuild: { builtAt: '2026-11-01T00:00:00Z' } as CatalogNode['lastBuild'], stale: [{ reason: 'behind' } as never] });
		expect(cardData(stale)).toMatchObject({ status: 'ok', stale: 'behind', live: false });
		expect(cardData({ ...stale, liveStatus: 'run' })).toMatchObject({ status: 'run', stale: undefined, live: true });
		expect(cardData({ ...stale, liveStatus: 'fail' })).toMatchObject({ status: 'fail', live: true });
	});
});
