import { groupConnections } from '$lib/platform/connections';
import { createResource } from '$lib/state/resource.svelte';
import { failing } from './utils';

/** The connections, grouped by published settings, with problem groups first when `problemsFirst`. */
export function createConnectionsState(problemsFirst = false) {
	const resource = createResource((client) => client.listWarehouseConnections(), ['warehouse_sync']);
	const groups = $derived.by(() => {
		const all = groupConnections(resource.data ?? []);
		if (!problemsFirst) return all;
		const bad = (g: (typeof all)[number]) => g.users.some((c) => c.lastTest?.outcome === 'failed' || failing(c.tables) > 0);
		return [...all.filter(bad), ...all.filter((g) => !bad(g))];
	});
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
