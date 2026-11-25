import { describe, expect, it } from 'vitest';
import type { CatalogNode } from '$lib/api';
import { drawnCount, expandAll, folderTree, scopeGraph } from './folder-graph';
import {
	foldersParam,
	foldersToOpen,
	inSelection,
	lineageGraph,
	lineageNodesHref,
	parseFolders,
	parseNodes,
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
	node('source:shop__orders', 'sql/sources/shop.yml'),
	node('source:shop__customers', 'sql/sources/shop.yml'),
	node('model:stg_orders', 'sql/models/staging/shop/stg_orders.sql', ['source:shop__orders']),
	node('model:stg_customers', 'sql/models/staging/crm/stg_customers.sql', ['source:shop__customers']),
	node('model:orders', 'sql/models/marts/orders.sql', ['model:stg_orders', 'model:stg_customers'], { stale: [{ reason: 'behind', upstream: [], detail: '', since: '' }] as CatalogNode['stale'] }),
	node('model:order_totals', 'sql/models/marts/order_totals.sql', ['model:orders']),
	node('model:inventory', 'sql/models/inventory/inventory.sql', ['model:orders']),
	node('model:inventory_daily', 'sql/models/inventory/inventory_daily.sql', ['model:inventory'])
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

	it('joins stubs to what stands for a node at the scope when drawing by folder', () => {
		// The selected folder is the scope: its own nodes are cards, and stubs join them.
		const graph = lineageGraph(nodes, { grouped: true, open: new Set(), selected: ['marts'], tree });
		expect(graph.edges.map((e) => e.id).sort()).toEqual(['model:orders-model:order_totals', 'model:orders-stub:downstream:inventory', 'stub:upstream:staging-model:orders']);
	});

	it('narrows to out-of-date nodes without stubs', () => {
		const graph = lineageGraph(nodes, { grouped: false, open: new Set(), selected: [], staleOnly: true, tree });
		expect(graph.nodes.map((n) => n.id)).toEqual(['model:orders']);
		expect(graph.edges).toEqual([]);
	});
});

describe('picked nodes', () => {
	const known = new Set(nodes.map((n) => n.key));

	it('reads known keys from the URL, without repeats', () => {
		expect(parseNodes('model:orders,model:returns, model:inventory,model:orders', known)).toEqual(['model:orders', 'model:inventory']);
		expect(parseNodes(null, known)).toEqual([]);
	});

	it('links to the Lineage with the keys', () => {
		expect(lineageNodesHref(['source:shop__orders', 'model:orders'])).toBe('/lineage?nodes=source%3Ashop__orders,model%3Aorders');
		expect(parseNodes(new URL(`http://x${lineageNodesHref(['model:orders'])}`).searchParams.get('nodes'), known)).toEqual(['model:orders']);
	});

	it('draws only the picked nodes, with stubs for lineage outside them', () => {
		const graph = lineageGraph(nodes, { grouped: false, open: new Set(), selected: [], picked: new Set(['model:orders', 'model:inventory']), tree });
		expect(graph.nodes.map((n) => n.id)).toEqual(['model:orders', 'model:inventory', 'stub:downstream:inventory', 'stub:downstream:marts', 'stub:upstream:staging']);
		expect(graph.edges.map((e) => e.id).sort()).toEqual([
			'model:inventory-stub:downstream:inventory',
			'model:orders-model:inventory',
			'model:orders-stub:downstream:marts',
			'stub:upstream:staging-model:orders'
		]);
	});
});

describe('one rule at every scope', () => {
	const flat = (n: number, folder: string) => Array.from({ length: n }, (_, i) => node(`model:${folder.replace(/\//g, '_')}_${i}`, `sql/models/${folder}/m${i}.sql`));
	// The same shape twice: once as a whole project, once inside a `marts` folder of a larger project.
	const shape = (prefix: string) => [...flat(90, `${prefix}core`), ...flat(70, `${prefix}finance`)];
	const project = [...shape('marts/'), node('model:marts_top', 'sql/models/marts/top.sql'), ...flat(40, 'staging')];
	const ids = (g: { nodes: { id: string; type?: string; parentId?: string }[] }) =>
		g.nodes.filter((n) => n.type !== 'stub').map((n) => `${n.type}:${n.id.replace('marts/', '').replace('model:marts_', 'model:').replace('model:martstop', 'model:top')}`).sort();

	it('draws a folder filter as the default view draws the same nodes: subfolders closed, own nodes as cards', () => {
		const alone = shape('');
		const defaultView = lineageGraph(alone, { grouped: alone.length > 150, open: new Set(), selected: [] });
		const filtered = lineageGraph(project, { grouped: true, open: new Set(), selected: ['marts'], tree: folderTree(project) });
		// The folder's own node is a card at the top of the scope.
		expect(ids(filtered)).toEqual(['asset:model:top', ...ids(defaultView)]);
		expect(ids(defaultView)).toEqual(['folder:group:core', 'folder:group:finance']);
	});

	it('draws a small scope node by node and a large one by folder', () => {
		expect(scopeGraph(flat(150, 'orders'), { open: new Set() }).grouped).toBe(false);
		expect(scopeGraph(flat(151, 'orders'), { open: new Set() }).grouped).toBe(true);
	});

	it('expands every folder in scope up to the drawing cap, and says where it stopped', () => {
		const t = folderTree(project);
		const all = expandAll(t, new Set(), ['marts']);
		expect(all).toMatchObject({ complete: true, opened: 2, total: 2 });
		expect(drawnCount(t, all.open, ['marts'])).toBe(1 + 2 + 160);
		const capped = expandAll(t, new Set(), ['marts'], 100);
		expect(capped).toMatchObject({ complete: false, opened: 1, total: 2 });
		expect(drawnCount(t, capped.open, ['marts'])).toBeLessThanOrEqual(100);
	});

	it('draws several selected folders as a box each, with their contents closed', () => {
		const g = lineageGraph(project, { grouped: true, open: new Set(), selected: ['marts/core', 'staging'], tree: folderTree(project) });
		expect(g.nodes.filter((n) => n.type === 'folderBox').map((n) => n.id).sort()).toEqual(['folder:marts/core', 'folder:staging']);
	});
});
