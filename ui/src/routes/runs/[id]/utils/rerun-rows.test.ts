import { describe, expect, it } from 'vitest';
import type { RunFailure } from './run-failures';
import { rerunRows } from './rerun-rows';

const check: RunFailure = { id: 'e1', kind: 'check', name: 'orders_match_payments', where: 'after the build', code: '42S02', message: '', cause: null, statementId: 's', nodeKey: null, checkId: 'chk', logQuery: '' };
const node: RunFailure = { ...check, id: 'r/fct_orders', kind: 'node', name: 'fct_orders', where: null, nodeKey: 'fct_orders', checkId: null };

describe('the Rerun menu', () => {
	it('shows a failed check once, as what didn’t succeed, then the whole run', () => {
		const rows = rerunRows({ failures: [check], unfinished: [], total: 93, jobKey: 'load_reference' });
		expect(rows.map((r) => [r.label, r.description, r.count])).toEqual([
			["Rerun what didn't succeed", 'The failed check orders_match_payments', '1 check'],
			['Rerun the whole run', 'Every node and check of load_reference', '93 nodes']
		]);
	});

	it('offers the failed node and the failed check apart from what didn’t succeed when they differ', () => {
		const rows = rerunRows({ failures: [node, check], unfinished: ['fct_orders', 'daily_revenue'], total: 30, jobKey: 'core_build' });
		expect(rows.map((r) => [r.key, r.count])).toEqual([
			['unfinished', '2 nodes · 1 check'],
			['node', '1 node'],
			['check', '1 check'],
			['all', '30 nodes']
		]);
	});
});
