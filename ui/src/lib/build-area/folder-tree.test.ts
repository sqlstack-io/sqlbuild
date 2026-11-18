import { describe, expect, it } from 'vitest';
import type { CatalogNode, StaleReason } from '$lib/api';
import { folderPaths, folderTree, visibleTreeRows, type TreeRow } from './folder-tree';

const behind = [{ reason: 'behind', upstream: [], detail: '', since: '2026-11-18T00:00:00Z' }] as StaleReason[];
const node = (key: string, path: string, extra: Partial<CatalogNode> = {}): CatalogNode =>
	({ key, kind: 'table', path, group: null, upstream: [], stale: [], lastOutcome: 'success', lastBuild: { builtAt: '2026-11-18T00:00:00Z' }, ...extra }) as CatalogNode;

const nodes = [
	node('card_events', 'models/payments/providers/card/card_events.sql', { stale: behind }),
	node('card_daily', 'models/payments/providers/card/card_daily.sql'),
	node('bank_events', 'models/payments/providers/bank/bank_events.sql'),
	node('settled', 'models/payments/settlement/settled.sql', { lastOutcome: 'failed' }),
	node('payments_summary', 'models/payments/payments_summary.sql'),
	node('orders', 'models/orders/marts/orders.sql'),
	node('raw_orders', 'sources/shop.yml', { kind: 'source' })
];
const label = (r: TreeRow) => (r.kind === 'folder' ? `${'  '.repeat(r.folder.depth)}${r.folder.group}${r.expandable ? (r.expanded ? ' v' : ' >') : ''}` : r.kind === 'node' ? `${'  '.repeat(r.depth)}· ${r.node.key}` : `${'  '.repeat(r.folder.depth + 1)}+${r.hidden} more`);
const rows = (opts: Partial<Parameters<typeof visibleTreeRows>[1]> = {}) =>
	visibleTreeRows(folderTree(nodes), { expanded: new Set(), sort: 'urgency', reverse: false, attentionOnly: false, listNodes: false, ...opts }).map(label);

describe('the catalog folder tree', () => {
	it('counts each folder with its subfolders, and selects exactly its nodes', () => {
		const [payments, orders] = folderTree(nodes);
		expect([payments.path, payments.total, payments.current, payments.behind, payments.failed]).toEqual(['payments', 5, 3, 1, 1]);
		expect(payments.selection).toBe('path:models/payments');
		expect(payments.children.map((c) => [c.path, c.total, c.selection])).toEqual([
			['payments/providers', 3, 'path:models/payments/providers'],
			['payments/settlement', 1, 'path:models/payments/settlement']
		]);
		expect(payments.members.map((n) => n.key)).toEqual(['payments_summary']);
		expect(orders.total).toBe(1);
		expect(folderPaths(folderTree(nodes))).toEqual(['payments', 'payments/providers', 'payments/providers/card', 'payments/providers/bank', 'payments/settlement', 'orders', 'orders/marts']);
	});

	it('expands in place, most urgent first within each level', () => {
		expect(rows()).toEqual(['payments >', 'orders >']);
		expect(rows({ expanded: new Set(['payments', 'payments/providers']) })).toEqual(['payments v', '  settlement', '  providers v', '    card', '    bank', 'orders >']);
		expect(rows({ expanded: new Set(['payments']), sort: 'group' })).toEqual(['orders >', 'payments v', '  providers >', '  settlement']);
	});

	it('lists a folder’s own nodes after its subfolders when asked, capped with a row for the rest', () => {
		expect(rows({ expanded: new Set(['payments', 'payments/providers/card']), listNodes: true, nodeLimit: 1 })).toEqual(['payments v', '  settlement >', '  providers >', '  · payments_summary', 'orders >']);
		expect(rows({ expanded: new Set(['payments', 'payments/providers', 'payments/providers/card']), listNodes: true, nodeLimit: 1 })).toEqual([
			'payments v',
			'  settlement >',
			'  providers v',
			'    card v',
			'      · card_events',
			'      +1 more',
			'    bank >',
			'  · payments_summary',
			'orders >'
		]);
	});

	it('leaves out fully current folders and nodes when showing only what needs attention', () => {
		expect(rows({ attentionOnly: true, expanded: new Set(['payments', 'payments/providers', 'payments/providers/card']), listNodes: true })).toEqual([
			'payments v',
			'  settlement >',
			'  providers v',
			'    card v',
			'      · card_events'
		]);
	});
});
