// The run page's bottom tabs that the server can load, in display order.

import type { ApiClient, Capability } from '$lib/api';
import type { BottomTab } from '../types';

// The log holds events and, where the person may view evidence, printed output, which can quote warehouse data.
export function bottomTabs(client: Pick<ApiClient, 'supports'>, can: (capability: Capability) => boolean): { value: BottomTab; label: string }[] {
	return [
		...(client.supports('listRunEvents') || (client.supports('listRunOutput') && can('view.evidence')) ? [{ value: 'log' as const, label: 'Log' }] : []),
		...(client.supports('listRunStatements') ? [{ value: 'statements' as const, label: 'Statements' }] : [])
	];
}
