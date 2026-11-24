import { describe, expect, it } from 'vitest';
import { compactTails, compactTitleOf, siblingTails } from './compact-tails';
import { pillText, type Measure } from './compact-title';

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

describe('a pill name too long to fit', () => {
	// Seeds of one job, named as a real project names them: shared endings across `__` layers, and long
	// single-part names that differ only at their end.
	const names = [
		'seed__order_type__alpha',
		'seed__order_type__beta',
		'seed__order_type__gamma',
		'seed__order_type__delta',
		'seed__layout__alpha',
		'seed__layout__beta',
		'seed__layout__gamma',
		'seed__region_usability_country',
		'seed__region_usability_check',
		'seed__region_usability_profile',
		'seed__canonical_course_mapping__north',
		'seed__canonical_course_mapping__south'
	];
	const measure: Measure = (text, font) => text.length * font * 0.6;

	it('takes the shortest unique ending of `__` parts among its siblings, for every name alike', () => {
		const tails = compactTails(names);
		expect(names.map((n) => tails.get(n))).toEqual([
			'order_type__alpha',
			'order_type__beta',
			'order_type__gamma',
			'delta',
			'layout__alpha',
			'layout__beta',
			'layout__gamma',
			'region_usability_country',
			'region_usability_check',
			'region_usability_profile',
			'north',
			'south'
		]);
	});

	it('loses its end, keeping its start, never its middle', () => {
		const tails = compactTails(names);
		const shown = names.map((n) => pillText(tails.get(n)!, 12, 100, measure));
		// Thirteen characters fit: twelve from the start and an ellipsis.
		expect(shown[0]).toBe('order_type__…');
		expect(shown[7]).toBe('region_usabi…');
		expect(shown[3]).toBe('delta');
		for (const [i, s] of shown.entries()) expect(tails.get(names[i])!.startsWith(s.replace(/…$/, ''))).toBe(true);
		for (const s of shown) expect(measure(s, 12)).toBeLessThanOrEqual(100);
	});

});
