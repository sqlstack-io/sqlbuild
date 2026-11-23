import { groupConnections } from '$lib/platform/connections';
import { createResource } from '$lib/state/resource.svelte';

/** The connections, grouped by their published settings. */
export function createConnectionsState() {
	const resource = createResource((client) => client.listWarehouseConnections(), ['warehouse_sync']);
	const groups = $derived(groupConnections(resource.data ?? []));
	return {
		get resource() {
			return resource;
		},
		get groups() {
			return groups;
		},
		refresh: () => resource.refresh()
	};
}
