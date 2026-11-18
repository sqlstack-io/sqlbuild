import { describe, expect, it } from 'vitest';
import { compactTails, compactTitleOf, siblingTails } from './compact-tails';

describe('compact pill names', () => {
	it('show the shortest ending of `__` parts no other card shares', () => {
		const tails = compactTails(['orders__stg__events', 'customers__stg__events', 'orders__mart__daily_totals', 'regions', 'finance__regions']);
		expect(Object.fromEntries(tails)).toEqual({
			orders__stg__events: 'orders__stg__events',
			customers__stg__events: 'customers__stg__events',
			orders__mart__daily_totals: 'daily_totals',
			// A name that is another's ending keeps itself; the other needs more of its parts.
			regions: 'regions',
			finance__regions: 'finance__regions'
		});
	});

	it('takes as many parts as it needs, and never the same tail twice', () => {
		const names = ['a__x__y__z', 'b__x__y__z', 'c__w__y__z', 'd__q'];
		const tails = compactTails(names);
		expect(names.map((n) => tails.get(n))).toEqual(['a__x__y__z', 'b__x__y__z', 'w__y__z', 'q']);
		expect(new Set(tails.values()).size).toBe(names.length);
	});

	it('tells cards apart only from their siblings in the same box', () => {
		const tails = siblingTails([
			{ id: 'a', parent: 'orders/staging', title: 'orders__stg__events' },
			{ id: 'b', parent: 'orders/staging', title: 'orders__stg__daily' },
			{ id: 'c', parent: 'customers/staging', title: 'customers__stg__events' },
			{ id: 'd', parent: '', title: 'orders__mart__events' },
			{ id: 'e', parent: '', title: 'customers__mart__events' }
		]);
		expect(Object.fromEntries(tails)).toEqual({ a: 'events', b: 'daily', c: 'events', d: 'orders__mart__events', e: 'customers__mart__events' });
	});

	it('reads a card’s title from its label, name, job or folder', () => {
		expect(compactTitleOf({ label: 'Marts', name: 'x' })).toBe('Marts');
		expect(compactTitleOf({ jobKey: 'core_build' })).toBe('core_build');
		expect(compactTitleOf({ group: 'staging' })).toBe('staging');
		expect(compactTitleOf({})).toBeNull();
	});
});
