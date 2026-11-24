// A job's discovered sets and how its graph frames them. Pure.

import type { DiscoveredSet } from '$lib/api';
import type { JobBox } from '$lib/lineage/job-graph';

/** The sets with at least one node in the job. */
export function setsInJob(sets: DiscoveredSet[], keys: string[]): DiscoveredSet[] {
	const inJob = new Set(keys);
	return sets.filter((s) => s.nodes.some((n) => inJob.has(n.key)));
}

/** A hash as people quote it: its first 7 characters, without an algorithm prefix. */
export function shortHash(hash: string | null | undefined): string | null {
	return hash ? hash.replace(/^[a-z0-9]+:/, '').slice(0, 7) : null;
}

/** One box per asset of each set, around the asset's steps in the job. */
export function discoveredBoxes(sets: DiscoveredSet[], keys: string[]): JobBox[] {
	const inJob = new Set(keys);
	return sets.flatMap((s) =>
		s.assets.map((asset) => ({
			jobKey: asset,
			nodes: s.nodes.filter((n) => n.asset === asset && inJob.has(n.key)).map((n) => n.key),
			discovered: { set: s.name, status: s.status, release: shortHash(s.tags.release), releaseFull: s.tags.release ?? null, error: s.error }
		}))
	);
}

/** The set and item behind a node, or null for a node no set made. */
export function itemOf(sets: DiscoveredSet[], key: string): { set: DiscoveredSet; item: DiscoveredSet['nodes'][number] } | null {
	for (const set of sets) {
		const item = set.nodes.find((n) => n.key === key);
		if (item) return { set, item };
	}
	return null;
}
