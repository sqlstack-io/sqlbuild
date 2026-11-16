import { describe, expect, it } from 'vitest';
import { LINE_HEIGHT, fitTitle, wrap, type Measure } from './compact-title';

// Every character half the font wide, so widths are easy to reason about.
const measure: Measure = (text, font) => text.length * font * 0.5;

describe('compact title', () => {
	it('wraps a name after its separators', () => {
		expect(wrap('orders_daily_summary', 90, 10, measure)).toEqual(['orders_daily_', 'summary']);
	});

	it('shrinks a title before breaking a word', () => {
		// At 30 px 'housekeeping_' needs 195 px of 180; at the smallest font it fits on its line.
		const fit = fitTitle('housekeeping_1', { width: 180, height: 80, minFont: 20, maxFont: 30 }, measure);
		expect(fit.lines).toEqual(['housekeeping_', '1']);
		expect(fit.font).toBeLessThan(28);
	});

	it('breaks a part too wide for a line anywhere', () => {
		expect(wrap('abcdefghij', 30, 10, measure)).toEqual(['abcdef', 'ghij']);
	});

	it('grows a short title to fill the card', () => {
		const fit = fitTitle('orders', { width: 200, height: 54, minFont: 22, maxFont: 44 }, measure);
		expect(fit).toEqual({ font: 44, lines: ['orders'] });
	});

	it('wraps a longer title onto two lines before shrinking it', () => {
		const fit = fitTitle('customer_orders_daily', { width: 200, height: 54, minFont: 22, maxFont: 44 }, measure);
		expect(fit.lines).toEqual(['customer_orders_', 'daily']);
		expect(fit.font).toBeGreaterThanOrEqual(22);
		expect(fit.lines.length * fit.font * LINE_HEIGHT).toBeLessThanOrEqual(54);
	});

	it('shortens a title that will not fit in the middle, keeping its start and end', () => {
		const name = 'inventory_by_region_and_warehouse_snapshot_daily';
		const fit = fitTitle(name, { width: 200, height: 30, minFont: 22, maxFont: 44 }, measure);
		expect(fit.font).toBe(22);
		expect(fit.lines).toHaveLength(1);
		const [line] = fit.lines;
		expect(line).toContain('…');
		expect(name.startsWith(line.split('…')[0])).toBe(true);
		expect(name.endsWith(line.split('…')[1])).toBe(true);
		expect(measure(line, 22)).toBeLessThanOrEqual(200);
	});
});
