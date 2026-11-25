import { describe, expect, it } from 'vitest';
import { folderOfPath, pathOfFolder, placeCrumbs, placeOf, selectorOf, union } from './selector';

describe('the Cost page’s selection as a place', () => {
	it('reads a job, a folder and a model shared by every token', () => {
		expect(placeOf('job:nightly,path:sql/models/staging/orders,stg_orders')).toEqual({ base: [], job: 'nightly', folder: 'staging/orders', model: 'stg_orders' });
		expect(placeOf('tag:finance,job:nightly')).toEqual({ base: ['tag:finance'], job: 'nightly', folder: null, model: null });
	});

	it('keeps what the tree can’t break down as the base: unions, upstream, and any except', () => {
		expect(placeOf('+orders')).toEqual({ base: ['+orders'], job: null, folder: null, model: null });
		expect(placeOf('job:nightly', 'tag:pii').base).toEqual(['job:nightly']);
		expect(placeOf('job:a job:b').base).toEqual(['job:a', 'job:b']);
	});

	it('writes folders as path selectors under the project layout, and back', () => {
		expect(pathOfFolder('staging/orders')).toBe('sql/models/staging/orders');
		expect(pathOfFolder('python/loaders')).toBe('python/nodes/loaders');
		expect(folderOfPath('python/nodes/loaders')).toBe('python/loaders');
		expect(folderOfPath('sql/models/staging/')).toBe('staging');
		expect(selectorOf({ base: ['tag:finance'], job: 'nightly', folder: 'marts', model: null })).toBe('tag:finance,job:nightly,path:sql/models/marts');
		expect(union('job:a', 'job:b')).toBe('job:a job:b');
	});

	it('gives a breadcrumb for each step up, each the selection there', () => {
		expect(placeCrumbs({ base: [], job: 'nightly', folder: 'staging/orders', model: null }).map((c) => [c.label, c.select])).toEqual([
			['All', ''],
			['nightly', 'job:nightly'],
			['staging', 'job:nightly,path:sql/models/staging'],
			['orders', 'job:nightly,path:sql/models/staging/orders']
		]);
	});
});
