import { describe, expect, it } from 'vitest';
import {
	choiceSummary,
	clearMatching,
	isWindowed,
	matchesAny,
	matchOptions,
	readValues,
	selectedFirst,
	selectMatching,
	showsSearch,
	splitMiddle,
	toggleValue,
	writeValues
} from './choice';

const opt = (value: string, label = value) => ({ value, label });
const OPTIONS = [opt('stg_orders'), opt('stg_customers'), opt('fct_orders'), opt('dim_customers')];

describe('choice filter search', () => {
	it('matches a fragment anywhere in the label, ignoring case', () => {
		expect(matchOptions(OPTIONS, 'ORDERS').map((o) => o.value)).toEqual(['stg_orders', 'fct_orders']);
		expect(matchOptions(OPTIONS, 'g_cust').map((o) => o.value)).toEqual(['stg_customers']);
	});

	it('matches every option for a blank search', () => {
		expect(matchOptions(OPTIONS, '  ')).toHaveLength(4);
	});
});

describe('choice filter selection', () => {
	it('selects all and clears only the options matching the search', () => {
		const matches = matchOptions(OPTIONS, 'customers');
		const chosen = selectMatching(['stg_orders'], matches);
		expect(chosen).toEqual(['stg_orders', 'stg_customers', 'dim_customers']);
		expect(selectMatching(chosen, matches)).toEqual(chosen);
		expect(clearMatching(chosen, matches)).toEqual(['stg_orders']);
	});

	it('toggles one value', () => {
		expect(toggleValue(['a'], 'b')).toEqual(['a', 'b']);
		expect(toggleValue(['a', 'b'], 'a')).toEqual(['b']);
	});

	it('lists selected options first, each part in its original order', () => {
		expect(selectedFirst(OPTIONS, new Set(['dim_customers', 'stg_customers'])).map((o) => o.value)).toEqual(['stg_customers', 'dim_customers', 'stg_orders', 'fct_orders']);
	});

	it('matches an item whose value is any of the selected values, or every item when none is', () => {
		expect(matchesAny(['a', 'b'], 'b')).toBe(true);
		expect(matchesAny(new Set(['a', 'b']), 'c')).toBe(false);
		expect(matchesAny([], 'c')).toBe(true);
		expect(matchesAny(['a'], null)).toBe(false);
	});

	it('summarises none, one or several choices', () => {
		expect(choiceSummary([], OPTIONS)).toBe('');
		expect(choiceSummary(['fct_orders'], [opt('fct_orders', 'Orders fact')])).toBe('Orders fact');
		expect(choiceSummary(['a', 'b', 'c'], OPTIONS)).toBe('3 selected');
	});
});

describe('middle truncation', () => {
	it('keeps the start and a meaningful end of a long label', () => {
		const { head, tail } = splitMiddle('orders__int_clean__delivery_window_by_region__emea');
		expect(head + tail).toBe('orders__int_clean__delivery_window_by_region__emea');
		expect(tail.endsWith('__emea')).toBe(true);
		expect(tail.length).toBeLessThanOrEqual(16);
		expect(head.startsWith('orders__int_clean__')).toBe(true);
	});

	it('takes the last 10 characters when the end has no separator', () => {
		expect(splitMiddle('a'.repeat(40)).tail).toBe('a'.repeat(10));
	});

	it('leaves short labels whole', () => {
		expect(splitMiddle('stg_orders')).toEqual({ head: 'stg_orders', tail: '' });
	});
});

describe('choice filter in the URL', () => {
	it('round-trips several values as repeated parameters', () => {
		const params = writeValues(new URLSearchParams('q=x'), 'node', ['a', 'b']);
		expect(params.toString()).toBe('q=x&node=a&node=b');
		expect(readValues(params, 'node')).toEqual(['a', 'b']);
		expect(writeValues(params, 'node', []).toString()).toBe('q=x');
	});

	it('reads a single-value link as before, and comma-separated values', () => {
		expect(readValues(new URLSearchParams('node=stg_orders'), 'node')).toEqual(['stg_orders']);
		expect(readValues(new URLSearchParams('node=a,b&node=c,a'), 'node')).toEqual(['a', 'b', 'c']);
		expect(readValues(new URLSearchParams(''), 'node')).toEqual([]);
	});

	it('writes one value exactly as a single-value link', () => {
		expect(writeValues(new URLSearchParams(), 'node', ['stg_orders']).toString()).toBe('node=stg_orders');
	});
});

describe('choice filter thresholds', () => {
	it('shows a search box from 10 options and windows above 200', () => {
		expect([showsSearch(9), showsSearch(10)]).toEqual([false, true]);
		expect([isWindowed(200), isWindowed(201), isWindowed(5000)]).toEqual([false, true, true]);
	});
});
