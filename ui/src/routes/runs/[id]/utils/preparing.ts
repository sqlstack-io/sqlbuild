// How long a run's worker spent preparing before its first node started.

import type { RunEvent, RunNode } from '$lib/api';
import { phaseSummary } from './events';

/** The worker's preparation, in epoch ms: from its launch to the first node's start, or to now while it prepares; its steps in order. */
export type PreparingSpan = { start: number; end: number; live: boolean; steps: string[] };

/** One run's preparation, or null before its worker launched or when it has no events of its own. */
export function preparingSpan(events: readonly RunEvent[], runId: string, nodes: readonly RunNode[], now: number): PreparingSpan | null {
	const own = events.filter((e) => e.runId === runId);
	const launched = own.find((e) => e.type === 'launched');
	if (!launched) return null;
	const start = Date.parse(launched.occurredAt);
	const firsts = nodes.filter((n) => n.runId === runId && n.startedAt).map((n) => Date.parse(n.startedAt!));
	const ended = own.some((e) => e.type === 'terminated' || e.type === 'presumed_failed');
	const last = own.reduce((at, e) => Math.max(at, Date.parse(e.occurredAt)), start);
	const end = firsts.length ? Math.min(...firsts) : ended ? last : now;
	const steps = own.filter((e) => e.type === 'invocation_phase' && e.phase).map((e) => phaseSummary(e.phase!));
	return { start, end: Math.max(start, end), live: !firsts.length && !ended, steps };
}
