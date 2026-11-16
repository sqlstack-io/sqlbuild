import { describe, expect, it } from 'vitest';
import { capItems, capLines } from './utils';

describe('capLines', () => {
	it('shows every item when they fit in the lines', () => {
		expect(capLines(['orders_hourly', 'orders_daily'])).toEqual({ shown: ['orders_hourly', 'orders_daily'], hidden: [] });
	});

	it('keeps the last line for "+N more" when they do not fit', () => {
		expect(capLines(['orders_hourly', 'orders_daily', 'inventory_sensor'])).toEqual({ shown: ['orders_hourly'], hidden: ['orders_daily', 'inventory_sensor'] });
	});

	it('handles an empty list', () => {
		expect(capLines([])).toEqual({ shown: [], hidden: [] });
	});
});

describe('capItems', () => {
	it('keeps the first items and returns the rest', () => {
		expect(capItems(['orders', 'customers', 'inventory', 'returns'], 3)).toEqual({ shown: ['orders', 'customers', 'inventory'], hidden: ['returns'] });
	});

	it('hides nothing when the list is short', () => {
		expect(capItems(['orders'], 3)).toEqual({ shown: ['orders'], hidden: [] });
	});
});
