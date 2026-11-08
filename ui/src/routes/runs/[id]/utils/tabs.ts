// The run page's bottom tabs that the server can load, in display order.

import type { ApiClient, Capability } from '$lib/api';
import type { BottomTab } from '../types';

// Output can quote warehouse data, so it shows only where the person may view evidence.
const TABS: { value: BottomTab; label: string; loads: keyof ApiClient; needs?: Capability }[] = [
	{ value: 'statements', label: 'Statements', loads: 'listRunStatements' },
	{ value: 'events', label: 'Events', loads: 'listRunEvents' },
	{ value: 'output', label: 'Output', loads: 'listRunOutput', needs: 'view.evidence' }
];

export function bottomTabs(client: Pick<ApiClient, 'supports'>, can: (capability: Capability) => boolean): { value: BottomTab; label: string }[] {
	return TABS.filter((t) => client.supports(t.loads) && (!t.needs || can(t.needs))).map(({ value, label }) => ({ value, label }));
}
