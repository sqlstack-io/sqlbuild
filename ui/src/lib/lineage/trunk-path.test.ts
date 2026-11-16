import { describe, expect, it } from 'vitest';
import { TRUNK, trunkPath } from './trunk-path';

describe('trunk paths', () => {
	it('bring every edge into a card through the same point, then straight in', () => {
		const into = (sx: number, sy: number) => trunkPath(sx, sy, 500, 200)!;
		const joins = [into(100, 50), into(100, 350), into(250, 200)].map((p) => p.match(/ (\d+),200 L500,200$/)?.[1]);
		expect(joins).toEqual([String(500 - TRUNK), String(500 - TRUNK), String(500 - TRUNK)]);
	});

	it('fall back to an ordinary curve when the target is too close or behind', () => {
		expect(trunkPath(480, 0, 500, 0)).toBeNull();
		expect(trunkPath(600, 0, 500, 0)).toBeNull();
	});
});
