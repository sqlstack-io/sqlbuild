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
					settings: [{ name: 'api_key', type: 'SecretStr', required: true, secret: true, env: 'FULFILLMENT_API_KEY', description: null, missing: true }],
					used_by: [
						{ kind: 'asset', name: 'publish_orders' },
						{ kind: 'sink', name: 'order_events' },
						{ kind: 'event_exporter', name: 'run_events' }
					],
					nodes: ['asset:publish_orders'],
					jobs: ['order_exports'],
					job_nodes: { order_exports: ['asset:publish_orders'] }
				}
			]
		};
		const [provider] = toProviders(wire);
		expect(provider.className).toBe('providers.fulfillment.FulfillmentApi');
		expect(provider.settings[0]).toEqual({ name: 'api_key', type: 'SecretStr', required: true, secret: true, env: 'FULFILLMENT_API_KEY', description: null, missing: true });
		expect(provider.usedBy.map((c) => `${c.kind}:${c.name}`)).toEqual(['asset:publish_orders', 'sink:order_events', 'event_exporter:run_events']);
		expect(provider.jobs).toEqual(['order_exports']);
		expect(provider.nodes).toEqual(['asset:publish_orders']);
		expect(provider.jobNodes).toEqual({ order_exports: ['asset:publish_orders'] });
	});

	it('defaults the nodes to none when the server leaves them out', () => {
		const [provider] = toProviders({ providers: [{ name: 'chat', class: 'providers.chat.Chat', description: '', path: 'providers/chat.py', settings: [], used_by: [] }] });
		expect([provider.nodes, provider.jobs, provider.jobNodes]).toEqual([[], [], {}]);
	});
});
