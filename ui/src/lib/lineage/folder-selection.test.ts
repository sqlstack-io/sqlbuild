import { describe, expect, it } from 'vitest';
import type { CatalogNode } from '$lib/api';
import { folderTree } from './folder-graph';
import {
	foldersParam,
	foldersToOpen,
	inSelection,
	lineageGraph,
	parseFolders,
	selectionStubs,
	stubFolder,
	stubLabel,
	withFolderAdded,
	withFolderToggled
} from './folder-selection';

function node(key: string, path: string, upstream: string[] = [], extra: Partial<CatalogNode> = {}): CatalogNode {
	const kind = key.startsWith('source:') ? 'source' : 'table';
	return { key, kind, path, upstream, group: null, tags: [], stale: [], lastOutcome: null, lastBuild: { builtAt: '2026-11-07T06:00:00Z' }, ...extra } as CatalogNode;
}

const nodes = [
	node('source:shop__orders', 'sources/shop.yml'),
	node('source:shop__customers', 'sources/shop.yml'),
	node('model:stg_orders', 'models/staging/shop/stg_orders.sql', ['source:shop__orders']),
	node('model:stg_customers', 'models/staging/crm/stg_customers.sql', ['source:shop__customers']),
	node('model:orders', 'models/marts/orders.sql', ['model:stg_orders', 'model:stg_customers'], { stale: [{ reason: 'behind', upstream: [], detail: '', since: '' }] as CatalogNode['stale'] }),
	node('model:order_totals', 'models/marts/order_totals.sql', ['model:orders']),
	node('model:inventory', 'models/inventory/inventory.sql', ['model:orders']),
	node('model:inventory_daily', 'models/inventory/inventory_daily.sql', ['model:inventory'])
];
const tree = folderTree(nodes);

describe('folder selection', () => {
	it('adds a folder, dropping folders inside it, and ignores a folder already inside the selection', () => {
		expect(withFolderAdded(['staging/shop', 'marts'], 'staging')).toEqual(['marts', 'staging']);
		expect(withFolderAdded(['staging'], 'staging/crm')).toEqual(['staging']);
	});

	it('toggles a folder in and out', () => {
		expect(withFolderToggled(['marts'], 'inventory')).toEqual(['marts', 'inventory']);
		expect(withFolderToggled(['marts', 'inventory'], 'marts')).toEqual(['inventory']);
	});

	it('round-trips through the URL, dropping folders the project no longer has', () => {
		expect(foldersParam([])).toBeNull();
		expect(foldersParam(['marts', 'inventory'])).toBe('marts,inventory');
		expect(parseFolders('marts,returns,inventory', tree)).toEqual(['marts', 'inventory']);
		expect(parseFolders('staging,staging/crm', tree)).toEqual(['staging']);
		expect(parseFolders(null, tree)).toEqual([]);
	});

	it('knows which nodes the selection holds and which folders to open to show them', () => {
		expect(inSelection(tree, 'model:stg_orders', ['staging'])).toBe(true);
		expect(inSelection(tree, 'model:orders', ['staging'])).toBe(false);
		expect(foldersToOpen(tree, ['staging/crm']).sort()).toEqual(['staging', 'staging/crm']);
	});
});

describe('stubs for lineage outside the selection', () => {
	it('names the outermost folder that neither holds nor sits inside the selection', () => {
		expect(stubFolder(['staging', 'staging/shop'], ['marts'])).toBe('staging');
		expect(stubFolder(['staging', 'staging/shop'], ['staging/crm'])).toBe('staging/shop');
	});

	it('counts distinct upstream and downstream nodes per folder', () => {
		const stubs = selectionStubs(nodes, tree, ['marts']);
		expect(stubs.map((s) => [s.direction, s.folder, s.count, s.targets])).toEqual([
			['downstream', 'inventory', 1, ['model:orders']],
			['upstream', 'staging', 2, ['model:orders']]
		]);
		expect(stubLabel(stubs[1])).toBe('+2 upstream in staging');
		expect(selectionStubs(nodes, tree, [])).toEqual([]);
	});
});

describe('lineage graph', () => {
	it('draws every node when nothing is selected', () => {
		const graph = lineageGraph(nodes, { grouped: false, open: new Set(), selected: [], tree });
		expect(graph.nodes).toHaveLength(nodes.length);
	});

	it('draws only the selection, with stubs joined to the nodes they feed', () => {
		const graph = lineageGraph(nodes, { grouped: false, open: new Set(), selected: ['marts'], tree });
		expect(graph.nodes.map((n) => n.id)).toEqual(['model:orders', 'model:order_totals', 'stub:downstream:inventory', 'stub:upstream:staging']);
		expect(graph.edges.map((e) => e.id).sort()).toEqual(['model:orders-model:order_totals', 'model:orders-stub:downstream:inventory', 'stub:upstream:staging-model:orders']);
	});

	it('joins stubs to a closed folder card when drawing by folder', () => {
		const graph = lineageGraph(nodes, { grouped: true, open: new Set(), selected: ['marts'], tree });
		expect(graph.edges.map((e) => e.id).sort()).toEqual(['group:marts-stub:downstream:inventory', 'stub:upstream:staging-group:marts']);
	});

	it('narrows to out-of-date nodes without stubs', () => {
		const graph = lineageGraph(nodes, { grouped: false, open: new Set(), selected: [], staleOnly: true, tree });
		expect(graph.nodes.map((n) => n.id)).toEqual(['model:orders']);
		expect(graph.edges).toEqual([]);
	});
});
