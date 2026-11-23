import { describe, expect, it } from 'vitest';
import { pillText, type Measure } from '$lib/components/graph/compact-title';
import { breakable, cardLabel, discoveredStep, sharedLeads } from './discovered-labels';

/** Each character 6 px wide, whatever the font. */
const measure: Measure = (text) => text.length * 6;

describe('discovered node labels', () => {
	it('name a discovered node by its step, and any other node by its name', () => {
		expect(discoveredStep('asset:forecast_eu[gbm_chain_d1]')).toEqual({ asset: 'forecast_eu', step: 'gbm_chain_d1' });
		expect(cardLabel('asset:forecast_eu[gbm_chain_d1]')).toBe('gbm_chain_d1');
		expect(cardLabel('model:orders')).toBe('orders');
		expect(discoveredStep('model:orders')).toBeNull();
	});

	it('find the prefix each name shares with a sibling, up to a joint', () => {
		const leads = sharedLeads(['hierarchy_category', 'hierarchy_supplier', 'gbm_chain_d1', 'gbm_chain_d3', 'itemloop']);
		expect(Object.fromEntries(leads)).toEqual({
			hierarchy_category: 'hierarchy_'.length,
			hierarchy_supplier: 'hierarchy_'.length,
			gbm_chain_d1: 'gbm_chain_'.length,
			gbm_chain_d3: 'gbm_chain_'.length,
			itemloop: 0
		});
	});

	it('keep a shared prefix when without it two names would read the same', () => {
		const leads = sharedLeads(['stg_customers', 'dim_customers', 'stg_orders', 'dim_orders_daily']);
		expect(Object.fromEntries(leads)).toEqual({ stg_customers: 0, dim_customers: 0, stg_orders: 'stg_'.length, dim_orders_daily: 'dim_'.length });
	});

	it('shorten a name by its shared prefix, keeping the part that tells it apart', () => {
		const lead = 'hierarchy_'.length;
		expect(pillText('hierarchy_supplier', 12, 15 * 6, measure, lead)).toBe('hierar…supplier');
		expect(pillText('hierarchy_supplier', 12, 9 * 6, measure, lead)).toBe('…supplier');
		// A rest too long for the room then loses its end.
		expect(pillText('hierarchy_supplier_regional', 12, 12 * 6, measure, lead)).toBe('…supplier_r…');
		// With no shared prefix the end goes, never the middle.
		expect(pillText('transformer_predictions', 12, 15 * 6, measure)).toBe('transformer_pr…');
	});


	it('break long names only after `_ . [ / :`', () => {
		expect(breakable('forecast_eu[gbm.chain/d1:x]')).toEqual(['forecast_', 'eu[', 'gbm.', 'chain/', 'd1:', 'x]']);
	});
});
