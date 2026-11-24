import { describe, expect, it } from 'vitest';
import type { CatalogNode, StaleReason } from '$lib/api';
import { shownNodes } from './folder-selection';
import { viewSummary } from './view-summary';

function node(key: string, kind: CatalogNode['kind'], path: string, stale: StaleReason['reason'][] = [], lastOutcome: CatalogNode['lastOutcome'] = null): CatalogNode {
	const reasons = stale.map((reason) => ({ reason, upstream: [], detail: '', since: '' }) as unknown as StaleReason);
	return { key, kind, path, upstream: [], group: null, tags: [], stale: reasons, lastOutcome, lastBuild: null } as unknown as CatalogNode;
}

const nodes = [
	node('source:raw_orders', 'source', 'sources/shop.yml'),
	node('model:orders', 'table', 'sql/models/marts/orders.sql', ['changed', 'behind']),
	node('model:customers', 'table', 'sql/models/marts/customers.sql', ['behind']),
	node('model:order_view', 'view', 'sql/models/marts/order_view.sql', ['changed_upstream']),
	node('model:inventory', 'incremental', 'sql/models/stock/inventory.sql', [], 'failed')
];

describe('view summary', () => {
	it('counts nodes by kind in kind order, and those needing a build once each', () => {
		expect(viewSummary(nodes)).toEqual({
			total: 5,
			byKind: [
				{ kind: 'source', count: 1 },
				{ kind: 'view', count: 1 },
				{ kind: 'table', count: 2 },
				{ kind: 'incremental', count: 1 }
			],
			changed: 1,
			behind: 2,
			failed: 1
		});
	});

	it('summarises only the nodes drawn for the selected folders or the out-of-date filter', () => {
		expect(viewSummary(shownNodes(nodes, { selected: ['stock'] })).total).toBe(1);
		expect(shownNodes(nodes, { selected: [], staleOnly: true }).map((n) => n.key)).toEqual(['model:orders', 'model:customers', 'model:order_view']);
		expect(shownNodes(nodes, { selected: ['stock'], picked: new Set(['model:orders']) }).map((n) => n.key)).toEqual(['model:orders']);
	});
});
