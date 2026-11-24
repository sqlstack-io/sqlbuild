import { describe, expect, it } from 'vitest';
import { ANCHOR_MARGIN, CARD_TITLE_PX, COMPACT_ZOOM, FULL_ZOOM, LABEL_MIN_ZOOM, MIN_TITLE_PX, NAME_SCREEN_PX, NAME_ZOOM, PILL_ZOOM, READABLE_ZOOM, cardStage, compactLevel, absolutePosition, anchoredViewport, firstCard, fitViewport, largestBox } from './fit';

describe('fitting a graph to the view', () => {
	it('centres a graph that fits, never closer than full size', () => {
		expect(fitViewport({ x: 0, y: 0, width: 400, height: 200 }, 1000, 500)).toEqual({ zoom: 1, x: 300, y: 150 });
	});

	it('shrinks a larger graph to fit while it stays readable', () => {
		const v = fitViewport({ x: 0, y: 0, width: 1100, height: 300 }, 1000, 500, { padding: 0 });
		expect(v.zoom).toBeCloseTo(1000 / 1100);
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
		// A cluster in the corner keeps the graph's corner near the view's corner, the cluster a margin inside.
		const corner = fitViewport(bounds, 1000, 500, { padding: 0, focus: { x: 0, y: 0, width: 100, height: 100 } });
		expect(corner).toEqual({ zoom: READABLE_ZOOM, x: ANCHOR_MARGIN, y: ANCHOR_MARGIN });
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
		// The start card's left edge is a margin inside the view's left.
		expect(v.x + start.x * v.zoom).toBe(ANCHOR_MARGIN);
		expect(inside(start, v)).toBe(true);
		// The bare corner would have shown no card at all.
		const corner = fitViewport(bounds, 1000, 500, { padding: 0, minZoom: 0.6 });
		expect(cards.some((c) => inside(c, corner))).toBe(false);
	});
});

describe('the readable zoom', () => {
	it('keeps card titles at least 11 px on screen', () => {
		expect(MIN_TITLE_PX).toBeGreaterThanOrEqual(11);
		expect(CARD_TITLE_PX * READABLE_ZOOM).toBeGreaterThanOrEqual(MIN_TITLE_PX - 1e-9);
	});
});

describe('the card a large graph opens on', () => {
	it('is the top of the leftmost column, wholly in view with a margin, even when a later column starts higher', () => {
		// Folder cards: the leftmost column sits lower than a card in the second column.
		const cards = [
			{ x: 0, y: 900, width: 236, height: 61 },
			{ x: 0, y: 1000, width: 236, height: 61 },
			{ x: 300, y: 0, width: 236, height: 61 },
			{ x: 3000, y: 500, width: 236, height: 61 }
		];
		const start = firstCard(cards)!;
		expect(start).toBe(cards[0]);
		const view = { width: 1376, height: 723 };
		const v = fitViewport({ x: 0, y: 0, width: 3236, height: 1061 }, view.width, view.height, { start });
		const [left, top] = [start.x * v.zoom + v.x, start.y * v.zoom + v.y];
		expect(left).toBeGreaterThanOrEqual(ANCHOR_MARGIN);
		expect(top).toBeGreaterThanOrEqual(ANCHOR_MARGIN);
		expect(left + start.width * v.zoom).toBeLessThanOrEqual(view.width - ANCHOR_MARGIN);
		expect(top + start.height * v.zoom).toBeLessThanOrEqual(view.height - ANCHOR_MARGIN);
	});
});

describe('a small graph opens in full form', () => {
	// A job's graph as the job page draws it: 1136 × 294 in a 1054 × 635 canvas.
	const bounds = { x: 0, y: 0, width: 1136, height: 294 };
	it("opens at the compact zoom, not below it for the sake of the margin, when it fits whole there", () => {
		const v = fitViewport(bounds, 1054, 635, { minZoom: LABEL_MIN_ZOOM });
		expect(v.zoom).toBe(COMPACT_ZOOM);
		expect(v.x).toBeGreaterThanOrEqual(ANCHOR_MARGIN);
		expect(v.x + bounds.width * v.zoom).toBeLessThanOrEqual(1054 - ANCHOR_MARGIN);
	});
	it('still goes compact for a graph too wide for that', () => {
		expect(fitViewport({ ...bounds, width: 1600 }, 1054, 635, { minZoom: LABEL_MIN_ZOOM }).zoom).toBeLessThan(COMPACT_ZOOM);
	});
});

describe('semantic zoom', () => {
	it('draws cards in stages: full, name only, pill, dot', () => {
		expect([1, FULL_ZOOM, FULL_ZOOM - 0.01, NAME_ZOOM, NAME_ZOOM - 0.01, PILL_ZOOM, PILL_ZOOM - 0.01, 0.1].map(cardStage)).toEqual(['full', 'full', 'name', 'name', 'pill', 'pill', 'dot', 'dot']);
		expect([1, 0.5, 0.33, 0.2].map(compactLevel)).toEqual([0, 1, 1, 2]);
	});
	it('keeps each stage while what it shows reads at about 10 px or more', () => {
		// The full card's 11 px status line, and the name-only card's name, which grows to 26 canvas px.
		expect(11 * FULL_ZOOM).toBeGreaterThanOrEqual(8.8);
		expect(26 * NAME_ZOOM).toBeCloseTo(NAME_SCREEN_PX);
		// Graphs still open with full cards where they did.
		expect(COMPACT_ZOOM).toBeGreaterThanOrEqual(FULL_ZOOM);
	});
});
