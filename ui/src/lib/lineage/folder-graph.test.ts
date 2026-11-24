import { withServerStatus } from '$lib/build-area/status-fixtures';
import { describe, expect, it } from 'vitest';
import type { Node } from '@xyflow/svelte';
import type { CatalogNode } from '$lib/api';
import { MAX_DRAWN, folderGraph, folderTree, layoutFolderGraph, tooManyToDraw, withFolderClosed, withFoldersOpen } from './folder-graph';

function node(key: string, path: string, upstream: string[] = [], extra: Partial<CatalogNode> = {}): CatalogNode {
	const kind = key.startsWith('source:') ? 'source' : key.startsWith('seed:') ? 'seed' : 'table';
	return withServerStatus({ key, kind, path, upstream, group: null, tags: [], stale: [], lastOutcome: null, lastBuild: { builtAt: '2026-11-07T06:00:00Z' }, ...extra } as CatalogNode);
}

const nodes = [
	node('source:shop__orders', 'sql/sources/shop.yml'),
	node('source:shop__customers', 'sql/sources/shop.yml'),
	node('model:stg_orders', 'sql/models/staging/shop/stg_orders.sql', ['source:shop__orders']),
	node('model:stg_customers', 'sql/models/staging/crm/stg_customers.sql', ['source:shop__customers'], { stale: [{ reason: 'behind', upstream: [], detail: '', since: '' }] }),
	node('model:orders', 'sql/models/marts/orders.sql', ['model:stg_orders', 'model:stg_customers'], { lastOutcome: 'failed' }),
	node('model:order_totals', 'sql/models/marts/order_totals.sql', ['model:orders']),
	node('model:pick_lists', 'sql/models/orders/fulfilment/staging/pick_lists.sql', ['model:orders'])
];
const ids = (graph: { nodes: Node[] }) => graph.nodes.map((n) => [n.id, n.parentId ?? null]);

describe('folder tree', () => {
	it('merges a folder whose only content is one subfolder into it', () => {
		const tree = folderTree(nodes);
		expect(tree.roots).toEqual(['sources/shop', 'staging', 'marts', 'orders/fulfilment/staging']);
		expect(tree.folders.get('staging')?.folders).toEqual(['staging/shop', 'staging/crm']);
		expect(tree.folders.get('staging/crm')?.label).toBe('crm');
		expect(tree.chains.get('model:stg_orders')).toEqual(['staging', 'staging/shop']);
	});

	it('opens every folder around a node, and closes a folder with the folders inside it', () => {
		const tree = folderTree(nodes);
		const open = withFoldersOpen(tree, 'model:stg_customers', new Set(['marts']));
		expect([...open].sort()).toEqual(['marts', 'staging', 'staging/crm']);
		expect([...withFolderClosed(open, 'staging')]).toEqual(['marts']);
	});
});

describe('folder graph', () => {
	it('draws each closed top folder as a card, with one edge per folder pair and none inside a folder', () => {
		const graph = folderGraph(nodes, new Set());
		expect(ids(graph)).toEqual([
			['group:sources/shop', null],
			['group:staging', null],
			['group:marts', null],
			['group:orders/fulfilment/staging', null]
		]);
		expect(graph.edges.map((e) => e.id)).toEqual(['group:sources/shop-group:staging', 'group:staging-group:marts', 'group:marts-group:orders/fulfilment/staging']);
		expect(graph.nodes.map((n) => [n.data.count, n.data.ok, n.data.stale, n.data.failed])).toEqual([[2, 0, 0, 0], [2, 1, 1, 0], [2, 1, 0, 1], [1, 1, 0, 0]]);
	});

	it('draws an open folder as a box around its subfolders, with edges meeting the cards inside', () => {
		const graph = folderGraph(nodes, new Set(['staging']));
		expect(ids(graph)).toEqual([
			['group:sources/shop', null],
			['folder:staging', null],
			['group:staging/shop', 'folder:staging'],
			['group:staging/crm', 'folder:staging'],
			['group:marts', null],
			['group:orders/fulfilment/staging', null]
		]);
		expect(graph.nodes[1].data).toMatchObject({ folder: 'staging', label: 'staging', nodeCount: 2, stale: 1 });
		expect(graph.nodes[2].data.label).toBe('shop');
		expect(graph.edges.map((e) => e.id)).toContain('group:sources/shop-group:staging/crm');
		expect(graph.edges.some((e) => e.source === 'folder:staging' || e.target === 'folder:staging')).toBe(false);
	});

	it('nests open subfolders as boxes, with edges attached to the nodes inside', () => {
		const closed: string[] = [];
		const graph = folderGraph(nodes, new Set(['staging', 'staging/shop']), { onclose: (f) => closed.push(f) });
		expect(ids(graph).slice(1, 5)).toEqual([
			['folder:staging', null],
			['folder:staging/shop', 'folder:staging'],
			['model:stg_orders', 'folder:staging/shop'],
			['group:staging/crm', 'folder:staging']
		]);
		expect(graph.edges.map((e) => e.id)).toEqual(expect.arrayContaining(['group:sources/shop-model:stg_orders', 'model:stg_orders-group:marts']));
		(graph.nodes[2].data.ontoggle as (f: string) => void)('staging/shop');
		expect(closed).toEqual(['staging/shop']);
	});

	it('refuses to lay out more cards than the cap', () => {
		const many = Array.from({ length: MAX_DRAWN + 1 }, (_, i) => node(`model:orders_${i}`, `sql/models/orders/orders_${i}.sql`));
		expect(tooManyToDraw(folderGraph(many, new Set()))).toBe(false);
		expect(tooManyToDraw(folderGraph(many, new Set(['orders'])))).toBe(true);
	});
});

describe('folder graph layout', () => {
	const within = (inner: { x: number; y: number; w: number; h: number }, outer: { w: number; h: number }) =>
		inner.x >= 0 && inner.y >= 0 && inner.x + inner.w <= outer.w && inner.y + inner.h <= outer.h;

	it('sizes each box around its contents, places them inside it, and keeps siblings apart', () => {
		const graph = folderGraph(nodes, new Set(['staging', 'staging/shop', 'staging/crm', 'marts']));
		const laid = layoutFolderGraph(graph.nodes, graph.edges);
		const byId = new Map(laid.map((n) => [n.id, n]));
		const box = (id: string) => ({ ...byId.get(id)!.position, w: byId.get(id)!.width!, h: byId.get(id)!.height! });
		const card = (id: string) => ({ ...byId.get(id)!.position, w: 236, h: 61 });
		expect(within(box('folder:staging/shop'), box('folder:staging'))).toBe(true);
		expect(within(box('folder:staging/crm'), box('folder:staging'))).toBe(true);
		expect(within(card('model:stg_orders'), box('folder:staging/shop'))).toBe(true);
		const [a, b] = [box('folder:staging/shop'), box('folder:staging/crm')];
		const overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
		expect(overlap).toBe(false);
		// Lineage orders the top-level boxes left to right.
		expect(box('folder:staging').x + box('folder:staging').w).toBeLessThanOrEqual(box('folder:marts').x);
	});
});

describe('opening a folder', () => {
	it('moves its siblings out of the way: no boxes or cards overlap at any level', () => {
		const many: CatalogNode[] = [];
		for (let f = 0; f < 14; f++)
			for (let i = 0; i < 5; i++) many.push(node(`model:f${f}_${i}`, `sql/models/marts/f${f}/sub${i % 2}/m${i}.sql`, i ? [`model:f${f}_0`] : f ? ['model:f0_0'] : []));
		const t = folderTree(many);
		const size = (n: Node) => ({ w: n.width ?? 236, h: n.height ?? 61 });
		for (const open of [['marts/f0'], ['marts/f3'], ['marts/f3', 'marts/f3/sub0'], ['marts/f13'], ['marts/f3', 'marts/f3/sub0', 'marts/f3/sub1']]) {
			const g = folderGraph(many, new Set(open), { tree: t, scope: ['marts'] });
			const placed = layoutFolderGraph(g.nodes, g.edges);
			const overlaps = [...Map.groupBy(placed, (n) => n.parentId ?? '').values()].flatMap((group) =>
				group.flatMap((a) =>
					group.filter((b) => {
						if (a.id >= b.id) return false;
						const [sa, sb] = [size(a), size(b)];
						return a.position.x < b.position.x + sb.w && b.position.x < a.position.x + sa.w && a.position.y < b.position.y + sb.h && b.position.y < a.position.y + sa.h;
					}).map((b) => `${a.id} × ${b.id}`)
				)
			);
			expect(overlaps, open.join(', ')).toEqual([]);
		}
	});
});
