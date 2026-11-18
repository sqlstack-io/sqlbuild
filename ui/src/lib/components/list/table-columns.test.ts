import { describe, expect, it } from 'vitest';
import { TABLE_MIN_WIDTH, WINDOW_ABOVE, cellClasses, ofTotal, tableMinWidth, windowed } from './table-columns';

describe('list table columns', () => {
	it('keeps the shared minimum width unless the columns need more', () => {
		expect(tableMinWidth([{ id: 'name', label: 'Name' }, { id: 'kind', label: 'Kind', width: '140px' }])).toBe(TABLE_MIN_WIDTH);
		const wide = Array.from({ length: 6 }, (_, i) => ({ id: `c${i}`, label: '', width: '200px' }));
		expect(tableMinWidth(wide)).toBe(1200);
		expect(tableMinWidth([{ id: 'a', label: '', minWidth: 300 }, { id: 'b', label: '', width: '30%' }], 0)).toBe(420);
	});

	it('windows lists above the threshold only', () => {
		expect(windowed(WINDOW_ABOVE)).toBe(false);
		expect(windowed(WINDOW_ABOVE + 1)).toBe(true);
	});

	it('truncates cells by default, wraps on request, and right-aligns numbers', () => {
		expect(cellClasses({ id: 'n', label: '' })).toEqual(['trunc']);
		expect(cellClasses({ id: 'n', label: '', truncate: false, align: 'right' })).toEqual(['wrap', 'num']);
		expect(cellClasses(undefined)).toEqual([]);
	});

	it('counts what the toolbar shows', () => {
		expect(ofTotal(12, 40)).toBe('12 of 40');
		expect(ofTotal(5000, 5000)).toBe('5,000');
	});
});
