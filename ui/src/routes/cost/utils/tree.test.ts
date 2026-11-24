import { describe, expect, it } from 'vitest';
import type { CostFact } from '$lib/api';
import { MANUAL, folderIds, indexCost, type Level } from './tree';

const fact = (job: string | null, nodeKey: string, folder: string, credits: number, runs = 2): CostFact => ({
	job,
	nodeKey,
	folder,
	credits,
	previousCredits: credits / 2,
	usd: null,
	previousUsd: null,
	seconds: credits * 100,
	runs,
	queries: 3,
	bytesScanned: 0
});

const facts = [
	fact('nightly', 'model:stg_orders', 'staging/orders', 4),
	fact('nightly', 'model:stg_customers', 'staging/customers', 2),
	fact('nightly', 'model:revenue', '', 3),
	fact('hourly', 'model:stg_orders', 'staging/orders', 1, 24),
	fact(null, 'model:revenue', '', 0.5, 1)
];
const index = indexCost({ facts });
const root: Level = { by: 'job', job: null, folder: null, flat: false };

describe('the cost tree', () => {
	it('lists jobs most expensive first, builds by hand under their own key', () => {
		expect(index.children(root).map((r) => [r.key, r.label, r.credits])).toEqual([
			['nightly', 'nightly', 9],
			['hourly', 'hourly', 1],
			[MANUAL, 'By hand or CLI', 0.5]
		]);
	});

	it("lists a job's folders and its top-level models, and a folder's subfolders", () => {
		const job: Level = { by: 'job', job: 'nightly', folder: null, flat: false };
		expect(index.children(job).map((r) => [r.kind, r.key, r.credits])).toEqual([
			['folder', 'staging', 6],
			['model', 'model:revenue', 3]
		]);
		expect(index.children({ ...job, folder: 'staging' }).map((r) => r.key)).toEqual(['staging/orders', 'staging/customers']);
		expect(index.children({ ...job, flat: true }).map((r) => [r.label, r.sub])).toEqual([
			['stg_orders', 'staging/orders'],
			['revenue', undefined],
			['stg_customers', 'staging/customers']
		]);
	});

	it('sums every job by folder, counting each job’s builds once', () => {
		const byFolder: Level = { by: 'folder', job: null, folder: null, flat: false };
		const [staging] = index.children(byFolder);
		expect([staging.key, staging.credits, staging.runs, staging.models]).toEqual(['staging', 7, 2 + 24, 2]);
		expect(index.totals(null, null)).toMatchObject({ credits: 10.5, previous: 5.25, models: 3 });
		expect(index.totals('nightly', 'staging/orders').credits).toBe(4);
		expect(index.totals(null, null, 'model:revenue').credits).toBe(3.5);
	});

	it('draws opened rows under theirs, and Expand all opens to a depth', () => {
		const open = new Set(['/job:nightly', '/job:nightly/folder:staging']);
		const rows = index.rows(root, open);
		expect(rows.map((r) => ('row' in r ? `${r.depth}:${r.row.label}` : 'more'))).toEqual(['0:nightly', '1:staging', '2:orders', '2:customers', '1:revenue', '0:hourly', '0:By hand or CLI']);
		expect(index.ids(root, 2)).toEqual(['/job:nightly', '/job:nightly/folder:staging', '/job:hourly', '/job:hourly/folder:staging', `/job:${MANUAL}`]);
		expect(index.rows({ ...root, by: 'job', job: 'nightly', flat: true }, new Set(), 1).map((r) => ('more' in r ? `+${r.more}` : r.row.label))).toEqual(['stg_orders', '+2']);
	});

	it('names the jobs that spent a model’s cost, and the rows down to its folder', () => {
		expect(index.jobsOf('model:stg_orders').map((r) => [r.key, r.credits])).toEqual([
			['nightly', 4],
			['hourly', 1]
		]);
		// Row ids are the path of ids from the level down, as the tree draws them.
		expect(folderIds('staging/orders/web', 'staging')).toEqual(['/folder:staging/orders', '/folder:staging/orders/folder:staging/orders/web']);
	});
});
