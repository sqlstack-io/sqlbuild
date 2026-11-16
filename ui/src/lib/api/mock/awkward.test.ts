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

	it('points copies at a bounded set of long names, shared by several rows', () => {
		const rows = stretch([{ key: 'a', node: 'orders' }, { key: 'b', node: 'customers' }, { key: 'c', node: 'payments' }], 5000);
		expect(new Set(rows.map((r) => r.node)).size).toBe(2000);
		expect(rows[0].node).toBe('orders');
	});

	it('leaves lists of plain values alone', () => {
		expect(stretch(['a', 'b'], 10)).toEqual(['a', 'b']);
	});
});
