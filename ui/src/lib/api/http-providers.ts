import type { components } from '$control/schema.d.ts';
import type { ProviderConnection, ProviderConsumer } from './types';

type Wire = components['schemas'];

export function toProviders(wire: Wire['ProvidersRead']): ProviderConnection[] {
	return wire.providers.map((p) => ({
		name: p.name,
		className: p.class,
		description: p.description,
		sourceFile: p.path,
		settings: p.settings.map((s) => ({ name: s.name, type: s.type, required: s.required, secret: s.secret, env: s.env, description: s.description ?? null })),
		usedBy: p.used_by.map((c) => ({ kind: c.kind as ProviderConsumer['kind'], name: c.name })),
		jobs: p.jobs ?? []
	}));
}
