import { describe, expect, it } from 'vitest';
import type { Node } from '@xyflow/svelte';
import type { CatalogNode } from '$lib/api';
import { boxId, jobGraph, layoutJobGraph } from './job-graph';

const node = (key: string, upstream: string[] = []) => ({ key, kind: 'table', upstream, owner: [], tags: [], stale: null, checks: null }) as unknown as CatalogNode;
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

	it('draws a plain job without boxes', () => {
		const { nodes } = jobGraph(['model:raw', 'model:stg'], catalog);
		expect(nodes.map((n) => [n.id, n.type, n.parentId])).toEqual([
			['model:raw', 'asset', undefined],
			['model:stg', 'asset', undefined]
		]);
	});
});
