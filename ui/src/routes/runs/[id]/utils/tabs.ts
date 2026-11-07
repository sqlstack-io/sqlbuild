// The run page's bottom tabs that the server can load, in display order.

import type { ApiClient } from '$lib/api';
import type { BottomTab } from '../types';

const TABS: { value: BottomTab; label: string; loads: keyof ApiClient }[] = [
	{ value: 'statements', label: 'Statements', loads: 'listRunStatements' },
	{ value: 'events', label: 'Events', loads: 'listRunEvents' },
	{ value: 'output', label: 'Output', loads: 'listRunOutput' }
];

export function bottomTabs(client: Pick<ApiClient, 'supports'>): { value: BottomTab; label: string }[] {
	return TABS.filter((t) => client.supports(t.loads)).map(({ value, label }) => ({ value, label }));
}
