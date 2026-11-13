import type { ProviderConnection, ProviderConsumer } from '$lib/api';

const plural = (count: number, noun: string) => `${count} ${noun}${count === 1 ? '' : 's'}`;

export const consumerKindLabel: Record<ProviderConsumer['kind'], string> = {
	loader: 'loader',
	task: 'task',
	asset: 'asset',
	check: 'check',
	hook: 'hook',
	materialization: 'materialization',
	event_exporter: 'event exporter',
	sink: 'sink'
};

/** e.g. `1 loader · 1 sink · 2 jobs`, or `Not used yet`. */
export function usedBySummary(p: ProviderConnection): string {
	const kinds = new Map<ProviderConsumer['kind'], number>();
	for (const c of p.usedBy) kinds.set(c.kind, (kinds.get(c.kind) ?? 0) + 1);
	const parts = [...[...kinds].map(([kind, count]) => plural(count, consumerKindLabel[kind])), p.jobs.length ? plural(p.jobs.length, 'job') : null].filter(Boolean);
	return parts.join(' · ') || 'Not used yet';
}

export function usedCount(p: ProviderConnection): number {
	return p.usedBy.length + p.jobs.length;
}
