// Summaries of change-feed entries. Pure; no side effects.

import type { SnapshotChange } from '$lib/api';

const count = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** e.g. "3 nodes · 1 trigger · 1 role · dependencies". */
export function summarizeChange(change: SnapshotChange): string {
	const parts: string[] = [];
	if (change.nodes.length) parts.push(count(change.nodes.length, 'node'));
	const orchestration = change.orchestration.length;
	if (orchestration) parts.push(count(orchestration, 'job or trigger', 'jobs and triggers'));
	if (change.roles.length) parts.push(count(change.roles.length, 'role'));
	if (change.environment) parts.push('environment');
	return parts.length ? parts.join(' · ') : 'No definition changes';
}
