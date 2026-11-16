import { describe, expect, it } from 'vitest';
import { stretch } from './awkward';

describe('awkward mock data', () => {
	it('stretches a list with unique long names, keeping the originals reachable and enums untouched', () => {
		const rows = stretch([{ key: 'orders', status: 'success', description: 'Daily orders' }], 5);
		expect(rows).toHaveLength(5);
		expect(rows[0].key).toBe('orders');
		expect(new Set(rows.map((r) => r.key)).size).toBe(5);
		expect(rows.every((r) => r.status === 'success')).toBe(true);
		expect(rows[3].description.length).toBeGreaterThan(80);
	});

	it('leaves lists of plain values alone', () => {
		expect(stretch(['a', 'b'], 10)).toEqual(['a', 'b']);
	});
});
