import { describe, expect, it } from 'vitest';
import { READABLE_ZOOM, absolutePosition, anchoredViewport, firstCard, fitViewport, largestBox } from './fit';

describe('fitting a graph to the view', () => {
	it('centres a graph that fits, never closer than full size', () => {
		expect(fitViewport({ x: 0, y: 0, width: 400, height: 200 }, 1000, 500)).toEqual({ zoom: 1, x: 300, y: 150 });
	});

	it('shrinks a larger graph to fit while it stays readable', () => {
		const v = fitViewport({ x: 0, y: 0, width: 1500, height: 300 }, 1000, 500, { padding: 0 });
		expect(v.zoom).toBeCloseTo(1000 / 1500);
		expect(v.x).toBeCloseTo(0);
	});

	it('stops at the readable zoom and starts at the top-left of a graph too large to show whole', () => {
		const v = fitViewport({ x: 100, y: 50, width: 10_000, height: 4000 }, 1000, 500, { padding: 0 });
		expect(v).toEqual({ zoom: READABLE_ZOOM, x: -100 * READABLE_ZOOM, y: -50 * READABLE_ZOOM });
	});

	it('centres on the largest cluster, without panning past the graph’s edges', () => {
		const bounds = { x: 0, y: 0, width: 10_000, height: 4000 };
		const middle = fitViewport(bounds, 1000, 500, { padding: 0, focus: { x: 5000, y: 2000, width: 400, height: 200 } });
		expect(middle).toEqual({ zoom: READABLE_ZOOM, x: 500 - 5200 * READABLE_ZOOM, y: 250 - 2100 * READABLE_ZOOM });
		// A cluster in the corner keeps the graph's corner at the view's corner, not empty space.
		const corner = fitViewport(bounds, 1000, 500, { padding: 0, focus: { x: 0, y: 0, width: 100, height: 100 } });
		expect(corner).toEqual({ zoom: READABLE_ZOOM, x: 0, y: 0 });
	});

	it('centres the axis along which a too-large graph still fits', () => {
		const v = fitViewport({ x: 0, y: 0, width: 10_000, height: 100 }, 1000, 500, { padding: 0 });
		expect(v.y).toBeCloseTo(250 - 50 * READABLE_ZOOM);
		expect(v.x).toBe(0);
	});

	it('picks the largest box as the cluster, and none when there are only cards', () => {
		const card = { x: 0, y: 0, width: 236, height: 61, box: false };
		expect(largestBox([card, { x: 10, y: 10, width: 500, height: 400, box: true }, { x: 0, y: 0, width: 300, height: 300, box: true }])).toEqual({ x: 10, y: 10, width: 500, height: 400 });
		expect(largestBox([card])).toBeNull();
	});
});

describe('keeping the view steady when a folder opens or closes', () => {
	it('pans by the clicked box’s layout shift, at the same zoom, so it stays put on screen', () => {
		const view = { x: 40, y: -20, zoom: 0.8 };
		const before = { x: 300, y: 120 };
		const after = { x: 360, y: 90 };
		const next = anchoredViewport(view, before, after);
		expect(next.zoom).toBe(0.8);
		// The pan moves by the shift, scaled to the screen, and the box's screen position is unchanged.
		expect(next.x - view.x).toBeCloseTo(-(after.x - before.x) * view.zoom);
		expect(next.y - view.y).toBeCloseTo(-(after.y - before.y) * view.zoom);
		expect(after.x * next.zoom + next.x).toBeCloseTo(before.x * view.zoom + view.x);
		expect(after.y * next.zoom + next.y).toBeCloseTo(before.y * view.zoom + view.y);
	});

	it('finds a card’s place on the graph through the boxes around it', () => {
		const nodes = [
			{ id: 'folder:orders', position: { x: 100, y: 50 } },
			{ id: 'folder:orders/staging', position: { x: 14, y: 44 }, parentId: 'folder:orders' },
			{ id: 'model:orders', position: { x: 14, y: 44 }, parentId: 'folder:orders/staging' }
		];
		expect(absolutePosition(nodes, 'model:orders')).toEqual({ x: 128, y: 138 });
		expect(absolutePosition(nodes, 'group:missing')).toBeNull();
	});
});

describe('a large graph without a box', () => {
	it('starts on the card nearest its top-left, not on an empty corner', () => {
		// A layered layout: the first column centred low, a later column reaching the top.
		const cards = [
			{ x: 0, y: 1800, width: 236, height: 82 },
			{ x: 0, y: 1900, width: 236, height: 82 },
			{ x: 3000, y: 0, width: 236, height: 82 }
		];
		const bounds = { x: 0, y: 0, width: 3236, height: 1982 };
		const inside = (r: (typeof cards)[number], view: { x: number; y: number; zoom: number }) =>
			r.x * view.zoom + view.x >= 0 && (r.x + r.width) * view.zoom + view.x <= 1000 && r.y * view.zoom + view.y >= 0 && (r.y + r.height) * view.zoom + view.y <= 500;
		const start = firstCard(cards)!;
		expect(start).toEqual(cards[0]);
		const v = fitViewport(bounds, 1000, 500, { padding: 0, minZoom: 0.6, start });
		// The start card's left edge is at the view's left; vertically the view stops at the graph's bottom edge.
		expect(v.x + start.x * v.zoom).toBe(0);
		expect(inside(start, v)).toBe(true);
		// The bare corner would have shown no card at all.
		const corner = fitViewport(bounds, 1000, 500, { padding: 0, minZoom: 0.6 });
		expect(cards.some((c) => inside(c, corner))).toBe(false);
	});
});
