import { describe, expect, it } from 'vitest';
import type { Node } from '@xyflow/svelte';
import type { CatalogNode } from '$lib/api';
import { boxId, jobGraph, layoutJobGraph } from './job-graph';

const node = (key: string, upstream: string[] = []) => ({ key, kind: 'table', upstream, tags: [], stale: null, checks: null }) as unknown as CatalogNode;
const measured = (nodes: Node[]) => nodes.map((n) => (n.type === 'job' ? n : { ...n, measured: { width: 236, height: 61 } }));

// staging builds raw → stg; orders reads stg; reports waits for orders only through `after`.
const catalog = [node('model:raw'), node('model:stg', ['model:raw']), node('model:orders', ['model:stg']), node('model:report')];
const all = catalog.map((n) => n.key);
const boxes = [
	{ jobKey: 'staging', nodes: ['model:raw', 'model:stg'] },
	{ jobKey: 'orders', nodes: ['model:orders', 'model:stg'] },
	{ jobKey: 'reports', nodes: ['model:report'] }
];
const members = [
	{ jobKey: 'staging', upstream: [], stage: 0 },
	{ jobKey: 'orders', upstream: [{ jobKey: 'staging', source: 'lineage' as const }], stage: 1 },
	{ jobKey: 'reports', upstream: [{ jobKey: 'orders', source: 'after' as const }, { jobKey: 'staging', source: 'lineage' as const }], stage: 2 }
];

describe('job graph', () => {
	it('puts each node in the first box that builds it and draws `after` only where no lineage links two boxes', () => {
		const { nodes, edges } = jobGraph(all, catalog, boxes, members);
		const parent = Object.fromEntries(nodes.filter((n) => n.type === 'asset').map((n) => [n.id, n.parentId]));
		expect(parent).toEqual({ 'model:raw': boxId('staging'), 'model:stg': boxId('staging'), 'model:orders': boxId('orders'), 'model:report': boxId('reports') });
		// Boxes come before their cards, as Svelte Flow requires.
		expect(nodes.findIndex((n) => n.id === boxId('reports'))).toBeLessThan(nodes.findIndex((n) => n.id === 'model:raw'));
		expect(edges.map((e) => `${e.source}>${e.target}`).sort()).toEqual(
			['model:raw>model:stg', 'model:stg>model:orders', `${boxId('orders')}>${boxId('reports')}`, `${boxId('staging')}>${boxId('reports')}`].sort()
		);
	});

	it('sizes boxes around their cards, places cards inside them, and orders boxes by their dependencies', () => {
		const { nodes, edges } = jobGraph(all, catalog, boxes, members);
		const laid = layoutJobGraph(measured(nodes), edges);
		const box = (k: string) => laid.find((n) => n.id === boxId(k))!;
		for (const card of laid.filter((n) => n.parentId)) {
			const b = laid.find((n) => n.id === card.parentId)!;
			expect(card.position.x).toBeGreaterThan(0);
			expect(card.position.y).toBeGreaterThan(0);
			expect(card.position.x + 236).toBeLessThan(b.width!);
			expect(card.position.y + 61).toBeLessThan(b.height!);
		}
		expect(box('staging').width).toBeGreaterThan(2 * 236);
		expect(box('staging').position.x + box('staging').width!).toBeLessThan(box('orders').position.x);
		expect(box('orders').position.x + box('orders').width!).toBeLessThan(box('reports').position.x);
	});

	it('puts a shared node in the job that builds fewer nodes, even when a broader job runs first', () => {
		// `everything` runs first and also builds `orders`, which `orders_only` exists to build.
		const shared = [{ jobKey: 'everything', nodes: all }, { jobKey: 'orders_only', nodes: ['model:orders'] }];
		const { nodes } = jobGraph(all, catalog, shared);
		expect(nodes.find((n) => n.id === 'model:orders')?.parentId).toBe(boxId('orders_only'));
		expect(nodes.find((n) => n.id === 'model:stg')?.parentId).toBe(boxId('everything'));
		const data = (k: string) => nodes.find((n) => n.id === boxId(k))!.data;
		expect([data('everything').nodeCount, data('everything').drawnElsewhere]).toEqual([4, 1]);
		expect([data('orders_only').nodeCount, data('orders_only').drawnElsewhere]).toEqual([1, 0]);
	});

	it('hides a collapsed box’s cards and joins their lineage to the box', () => {
		const { nodes, edges } = jobGraph(all, catalog, boxes, members, new Set(['staging']));
		expect(nodes.filter((n) => n.type === 'asset').map((n) => n.id).sort()).toEqual(['model:orders', 'model:report']);
		expect(nodes.find((n) => n.id === boxId('staging'))!.data.collapsed).toBe(true);
		// raw → stg sits inside the box and disappears; stg → orders now leaves the box. Staging and
		// reports still have no lineage between them, so their `after` edge stays.
		expect(edges.map((e) => `${e.source}>${e.target}`).sort()).toEqual(
			[`${boxId('staging')}>model:orders`, `${boxId('orders')}>${boxId('reports')}`, `${boxId('staging')}>${boxId('reports')}`].sort()
		);
		const laid = layoutJobGraph(measured(nodes), edges);
		expect(laid.find((n) => n.id === boxId('staging'))!.height).toBeLessThan(61);
	});

	it('wraps a tall column into several, keeping every node right of what it reads', () => {
		// 40 marts with no lineage inside the job, and 30 models off one source: 71 cards, at most 15 to a column.
		const wide = [
			...Array.from({ length: 40 }, (_, i) => node(`model:mart_${i}`)),
			node('model:src'),
			...Array.from({ length: 30 }, (_, i) => node(`model:stg_${i}`, ['model:src']))
		];
		const { nodes, edges } = jobGraph(wide.map((n) => n.key), wide);
		const laid = layoutJobGraph(measured(nodes), edges);
		const columns = Map.groupBy(laid, (n) => Math.round(n.position.x));
		expect(Math.max(...[...columns.values()].map((c) => c.length))).toBeLessThanOrEqual(15);
		// The wrapped columns sit side by side rather than staggered: the graph is no taller than one full column.
		const ys = laid.map((n) => n.position.y);
		expect(Math.max(...ys) - Math.min(...ys)).toBeLessThan(15 * (61 + 24));
		const x = (id: string) => laid.find((n) => n.id === id)!.position.x;
		for (let i = 0; i < 30; i++) expect(x(`model:stg_${i}`)).toBeGreaterThan(x('model:src'));
	});

	it('leaves a graph that needs no wrapping as dagre lays it out', () => {
		// `extra` only feeds the end of a long chain, so it sits just before it, not back at the start.
		const chain = [node('model:a'), node('model:b', ['model:a']), node('model:c', ['model:b']), node('model:d', ['model:c']), node('model:extra'), node('model:end', ['model:d', 'model:extra'])];
		const { nodes, edges } = jobGraph(chain.map((n) => n.key), chain);
		const x = new Map(layoutJobGraph(measured(nodes), edges).map((n) => [n.id, n.position.x]));
		expect(x.get('model:extra')).toBe(x.get('model:d'));
	});

	it('keeps every box right of the boxes it runs after when their columns wrap', () => {
		const many = (job: string) => Array.from({ length: 30 }, (_, i) => node(`model:${job}_${i}`));
		const wide = [...many('a'), ...many('b')];
		const groupBoxes = [
			{ jobKey: 'a', nodes: wide.slice(0, 30).map((n) => n.key) },
			{ jobKey: 'b', nodes: wide.slice(30).map((n) => n.key) },
			{ jobKey: 'empty', nodes: [] }
		];
		const after = (jobKey: string) => ({ jobKey, upstream: [{ jobKey: 'a', source: 'after' as const }], stage: 1 });
		const { nodes, edges } = jobGraph(wide.map((n) => n.key), wide, groupBoxes, [{ jobKey: 'a', upstream: [], stage: 0 }, after('b'), after('empty')]);
		const laid = layoutJobGraph(measured(nodes), edges);
		const box = (k: string) => laid.find((n) => n.id === boxId(k))!;
		expect(box('a').height).toBeLessThan(15 * (61 + 24) + 60);
		expect(box('b').position.x).toBeGreaterThan(box('a').position.x + box('a').width!);
		expect(box('empty').position.x).toBeGreaterThan(box('a').position.x + box('a').width!);
	});

	it('keeps lineage left to right when it leaves a box and comes back', () => {
		// `broad` builds raw → stg → mart, but `narrow` builds only stg, so stg sits in narrow's box.
		const chain = [node('model:raw'), node('model:stg', ['model:raw']), node('model:mart', ['model:stg'])];
		const nested = [{ jobKey: 'broad', nodes: chain.map((n) => n.key) }, { jobKey: 'narrow', nodes: ['model:stg'] }];
		const { nodes, edges } = jobGraph(chain.map((n) => n.key), chain, nested);
		const laid = layoutJobGraph(measured(nodes), edges);
		const x = (id: string) => {
			const card = laid.find((n) => n.id === id)!;
			return card.position.x + (laid.find((n) => n.id === card.parentId)?.position.x ?? 0);
		};
		expect(x('model:raw')).toBeLessThan(x('model:stg'));
		expect(x('model:stg')).toBeLessThan(x('model:mart'));
	});

	it('draws a plain job without boxes', () => {
		const { nodes } = jobGraph(['model:raw', 'model:stg'], catalog);
		expect(nodes.map((n) => [n.id, n.type, n.parentId])).toEqual([
			['model:raw', 'asset', undefined],
			['model:stg', 'asset', undefined]
		]);
	});
});
