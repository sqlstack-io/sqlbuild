import { describe, expect, it } from 'vitest';
import { countBy } from './count-by';

describe('countBy', () => {
	it('counts items per key in one pass, skipping missing keys', () => {
		const items = [{ node: 'orders' }, { node: 'customers' }, { node: 'orders' }, { node: null }];
		expect(countBy(items, (i) => i.node)).toEqual(new Map([['orders', 2], ['customers', 1]]));
	});

	it('counts an item under each of several keys', () => {
		expect(countBy([['a', 'b'], ['b']], (keys) => keys)).toEqual(new Map([['a', 1], ['b', 2]]));
	});

	it('handles 5,000 items with 2,000 distinct keys quickly', () => {
		const items = Array.from({ length: 5000 }, (_, i) => ({ node: `node_${i % 2000}` }));
		const started = performance.now();
		const counts = countBy(items, (i) => i.node);
		expect(counts.size).toBe(2000);
		expect(performance.now() - started).toBeLessThan(50);
	});
});
