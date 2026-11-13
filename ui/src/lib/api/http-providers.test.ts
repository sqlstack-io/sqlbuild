import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { toProviders } from './http-providers';

type W = components['schemas'];

describe('toProviders', () => {
	it('maps settings, consumers and jobs without values', () => {
		const wire: W['ProvidersRead'] = {
			snapshot_id: '00000000-0000-0000-0000-000000000001',
			providers: [
				{
					name: 'fulfillment_api',
					class: 'providers.fulfillment.FulfillmentApi',
					description: 'Client for the fulfillment service.',
					path: 'providers/fulfillment.py',
					settings: [{ name: 'api_key', type: 'SecretStr', required: true, secret: true, env: 'FULFILLMENT_API_KEY', description: null }],
					used_by: [
						{ kind: 'task', name: 'publish_orders' },
						{ kind: 'sink', name: 'order_events' },
						{ kind: 'event_exporter', name: 'run_events' }
					],
					jobs: ['order_exports']
				}
			]
		};
		const [provider] = toProviders(wire);
		expect(provider.className).toBe('providers.fulfillment.FulfillmentApi');
		expect(provider.settings[0]).toEqual({ name: 'api_key', type: 'SecretStr', required: true, secret: true, env: 'FULFILLMENT_API_KEY', description: null });
		expect(provider.usedBy.map((c) => `${c.kind}:${c.name}`)).toEqual(['task:publish_orders', 'sink:order_events', 'event_exporter:run_events']);
		expect(provider.jobs).toEqual(['order_exports']);
	});
});
