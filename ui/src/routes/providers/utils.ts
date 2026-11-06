import type { ProviderConnection } from '$lib/api';

const plural = (count: number, noun: string) => `${count} ${noun}${count === 1 ? '' : 's'}`;

/** e.g. `1 node · 2 jobs · 1 sensor`, or `Not used yet`. */
export function usedBySummary(p: ProviderConnection): string {
	const parts = [
		p.usedBy.nodes.length ? plural(p.usedBy.nodes.length, 'node') : null,
		p.usedBy.jobs.length ? plural(p.usedBy.jobs.length, 'job') : null,
		p.usedBy.triggers.length ? plural(p.usedBy.triggers.length, 'sensor') : null
	].filter(Boolean);
	return parts.join(' · ') || 'Not used yet';
}

export function usedCount(p: ProviderConnection): number {
	return p.usedBy.nodes.length + p.usedBy.jobs.length + p.usedBy.triggers.length;
}
