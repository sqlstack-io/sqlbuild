import { describe, expect, it } from 'vitest';
import type { CatalogNode } from '$lib/api';
import { folderCatalogHref, folderSelections } from './folders';

const node = (key: string, path: string, kind = 'table', group: string | null = null) => ({ key, path, kind, group }) as unknown as CatalogNode;

describe('folder selections', () => {
	it('selects a folder by path when it holds exactly the group', () => {
		const out = folderSelections([
			node('model:orders', 'sql/models/marts/orders.sql'),
			node('source:shop', 'sql/models/marts/sources.yml', 'source'),
			node('model:stg_a', 'sql/models/staging/shop/stg_a.sql'),
			node('model:stg_b', 'sql/models/staging/crm/stg_b.sql')
		]);
		// A source in the folder isn't counted, so its path would select more: the nodes are named instead.
		expect(out.get('marts')).toBe('orders');
		expect(out.get('staging')).toBe('path:sql/models/staging');
	});

	it('names the nodes of a group no folder stands for', () => {
		const out = folderSelections([node('seed:codes', 'sql/seeds/codes.csv', 'seed'), node('model:top', 'sql/models/top.sql'), node('model:orders', 'sql/models/marts/orders.sql')]);
		expect(out.get('seeds')).toBe('codes');
		expect(out.get('models')).toBe('top');
	});

	it('names the nodes of a folder outside models/ and python/, which a path selector rejects', () => {
		const out = folderSelections([node('seed:codes', 'sql/seeds/lookups/codes.csv', 'seed'), node('seed:regions', 'sql/seeds/lookups/regions.csv', 'seed')]);
		expect(out.get('seeds')).toBe('codes regions');
	});

	it('names the nodes when another group shares the folder', () => {
		const out = folderSelections([node('model:orders', 'sql/models/marts/orders.sql'), node('model:legacy', 'sql/models/marts/legacy.sql', 'table', 'archive')]);
		expect(out.get('marts')).toBe('orders');
		expect(out.get('archive')).toBe('legacy');
	});

	it('links to the catalog with the selection', () => {
		expect(folderCatalogHref('path:sql/models/marts')).toBe('/catalog?select=path%3Asql%2Fmodels%2Fmarts');
	});
});
