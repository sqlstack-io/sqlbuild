import { describe, expect, it } from 'vitest';
import { allowedSecretsNote, secretsSummary } from './utils';

const orders = { provider: 'orders_api', path: 'providers/orders_api.py', settings: [{ field: 'api_key', variable: 'ORDERS_API_KEY', line: 18 }], nodes: ['source:raw_orders'], allowed: false };

describe("a location's providers missing secrets", () => {
	it('sums up each provider with its variables', () => {
		const two = { ...orders, provider: 'crm', settings: [{ field: 'a', variable: 'CRM_A', line: null }, { field: 'b', variable: 'CRM_B', line: null }] };
		expect(secretsSummary([orders, two])).toBe('orders_api needs ORDERS_API_KEY · crm needs CRM_A and CRM_B');
	});

	it('notes an allowed provider quietly, still saying what can not run', () => {
		expect(allowedSecretsNote({ ...orders, allowed: true })).toBe(
			'orders_api is missing ORDERS_API_KEY, which this target allows (allow_missing_secrets); runs of raw_orders stop before the warehouse'
		);
	});
});
