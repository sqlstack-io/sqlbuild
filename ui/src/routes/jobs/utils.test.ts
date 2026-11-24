import { describe, expect, it } from 'vitest';
import { ownerChip, selectionTokens } from './utils';

describe('selectionTokens', () => {
	it('splits on whitespace and separates the method prefix', () => {
		expect(selectionTokens('tag:orders  source:shop__customers')).toEqual([
			{ token: 'tag:orders', prefix: 'tag:', name: 'orders' },
			{ token: 'source:shop__customers', prefix: 'source:', name: 'shop__customers' }
		]);
	});

	it('keeps graph operators with the prefix and leaves bare names and paths whole', () => {
		expect(selectionTokens('+tag:inventory+ orders_daily models/orders')).toEqual([
			{ token: '+tag:inventory+', prefix: '+tag:', name: 'inventory+' },
			{ token: 'orders_daily', prefix: '', name: 'orders_daily' },
			{ token: 'models/orders', prefix: '', name: 'models/orders' }
		]);
	});

	it('returns nothing for an empty or missing selection', () => {
		expect(selectionTokens('')).toEqual([]);
		expect(selectionTokens(null)).toEqual([]);
	});

	it('gives each comma-joined selector its own chip', () => {
		expect(selectionTokens('tag:orders,tag:daily_orders customers').map((t) => t.token)).toEqual(['tag:orders', 'tag:daily_orders', 'customers']);
	});
});

describe('ownerChip', () => {
	it('uses the name part of an email', () => {
		const chip = ownerChip('jane.doe@example.com');
		expect(chip).toMatchObject({ initials: 'JD', name: 'Jane Doe', full: 'jane.doe@example.com' });
	});

	it('takes the first two letters of a single-word name', () => {
		expect(ownerChip('orders@example.com')).toMatchObject({ kind: 'person', initials: 'OR', name: 'Orders' });
	});

	it('shows an owner that is not an email as a team, without initials', () => {
		expect(ownerChip('team:orders')).toMatchObject({ kind: 'team', initials: null, name: 'orders', full: 'team:orders' });
		expect(ownerChip('inventory-team')).toMatchObject({ kind: 'team', initials: null, name: 'inventory-team' });
	});

	it('gives the same colour to the same person', () => {
		expect(ownerChip('jane.doe@example.com')?.colour).toBe(ownerChip('jane.doe@example.org')?.colour);
		expect(ownerChip('jane.doe@example.com')?.colour).toMatch(/^#[0-9a-f]{6}$/);
	});

	it('is null when there is no owner', () => {
		expect(ownerChip(null)).toBeNull();
		expect(ownerChip('  ')).toBeNull();
	});
});
