import { describe, expect, it } from 'vitest';
import { READABLE_ZOOM, fitViewport, largestBox } from './fit';

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
