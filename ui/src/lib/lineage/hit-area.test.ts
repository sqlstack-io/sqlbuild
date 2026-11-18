import { describe, expect, it } from 'vitest';
import { MIN_HIT, unzoom } from './hit-area';

describe('graph hit areas', () => {
	it('keep their on-screen size when zoomed out, and never shrink below it', () => {
		// On screen, a control is its size in graph units times the zoom.
		for (const zoom of [0.1, 0.3, 0.6, 1, 1.5]) expect(MIN_HIT * unzoom(zoom) * zoom).toBeGreaterThanOrEqual(MIN_HIT - 1e-9);
		expect(unzoom(0.5)).toBe(2);
		expect(unzoom(2)).toBe(1);
	});
});
