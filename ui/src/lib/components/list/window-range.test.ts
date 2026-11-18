import { describe, expect, it } from 'vitest';
import { windowRange } from './window-range';

describe('windowRange', () => {
	it('draws the rows in view plus the overscan at the top of the list', () => {
		expect(windowRange(0, 520, 52, 8, 1000)).toEqual({ first: 0, last: 18 });
	});

	it('draws overscan rows either side further down', () => {
		expect(windowRange(52 * 100, 520, 52, 8, 1000)).toEqual({ first: 92, last: 118 });
	});

	it('clamps to the end of the list', () => {
		expect(windowRange(52 * 995, 520, 52, 8, 1000)).toEqual({ first: 987, last: 1000 });
	});

	it('keeps first at or before last when the list shrank below the scroll position', () => {
		expect(windowRange(52 * 500, 520, 52, 8, 3)).toEqual({ first: 3, last: 3 });
	});

	it('draws nothing for an empty list', () => {
		expect(windowRange(0, 520, 52, 8, 0)).toEqual({ first: 0, last: 0 });
	});
});
