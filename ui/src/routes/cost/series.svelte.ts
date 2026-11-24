// Daily series for rows deeper than jobs and top-level folders, read in batches of up to 200 groups
// as rows come into view, and kept until the read they belong to changes.

import type { ApiClient, CostSelectionQuery, CostSeriesGroup, Scope } from '$lib/api';

/** The most groups one read asks for; the server's limit. */
const BATCH = 200;

export const groupKey = (g: CostSeriesGroup) => `${g.manual ? '\u0001' : (g.job ?? '')}\u0000${g.folder ?? ''}\u0000${g.nodeKey ?? ''}`;

export function createSeries() {
	let held = new Map<string, readonly number[]>();
	let asked = new Set<string>();
	let read = '';
	let version = $state(0);
	let failed = $state<string | null>(null);

	return {
		/** A group's series, once read; null while unknown. */
		get(group: CostSeriesGroup): readonly number[] | null {
			void version;
			return held.get(groupKey(group)) ?? null;
		},
		get failed() {
			return failed;
		},
		/** Reads the groups not yet read or asked for, for `query` at `scope`; a new query drops what was held. */
		want(client: ApiClient, scope: Scope, query: CostSelectionQuery, groups: CostSeriesGroup[]) {
			const at = `${scope.locationId}\u0000${query.days}\u0000${query.select}\u0000${query.exclude}`;
			if (at !== read) {
				read = at;
				held = new Map();
				asked = new Set();
				failed = null;
				version += 1;
			}
			const missing = groups.filter((g) => !asked.has(groupKey(g)));
			for (const g of missing) asked.add(groupKey(g));
			for (let i = 0; i < missing.length; i += BATCH) {
				const batch = missing.slice(i, i + BATCH);
				client.readCostSeries(scope, query, batch).then(
					(series) => {
						if (read !== at) return;
						batch.forEach((g, n) => held.set(groupKey(g), series[n] ?? []));
						version += 1;
					},
					(error: unknown) => {
						if (read !== at) return;
						for (const g of batch) asked.delete(groupKey(g));
						failed = error instanceof Error ? error.message : String(error);
					}
				);
			}
		}
	};
}
