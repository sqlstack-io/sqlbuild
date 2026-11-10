import type { components } from '$control/schema.d.ts';
import { toRuns } from './http-runs';
import type { Timeline, TimelineBar, TimelineTickMarker } from './types/operate';

type Wire = components['schemas'];

/** A projected tick's length when its job has no finished successful run to go by. */
const FALLBACK_TYPICAL_MS = 60_000;

export type TimelineReads = {
	runs: Wire['RunRecord'][];
	jobs: Wire['JobRecord'][];
	triggers: Wire['TriggerRecord'][];
	ticks: Wire['TickRecord'][];
};

/** Top-level job runs overlapping the window, each running schedule's next due tick, and ticks an existing run covered. */
export function timelineOf(reads: TimelineReads, locationId: string, window: { since: string; until: string }, now: number): Timeline {
	const nowIso = new Date(now).toISOString();
	const bars: TimelineBar[] = toRuns(reads.runs, locationId).flatMap((r) => {
		if (r.parentRunId || !r.jobKey || !r.startedAt) return [];
		const end = r.endedAt ?? nowIso;
		if (end <= window.since || r.startedAt >= window.until) return [];
		return [{ runId: r.id, jobKey: r.jobKey, kind: r.kind, origin: r.origin, status: r.status, triggerName: r.triggerName, start: r.startedAt, end }];
	});
	const kinds = new Map(reads.jobs.map((j) => [j.job_key, j.kind]));
	for (const t of reads.triggers) {
		const due = t.next_due_at;
		if (!('schedule' in t.definition_json) || t.status !== 'running' || !due || due < window.since || due >= window.until) continue;
		bars.push({
			runId: null,
			jobKey: t.job_key,
			kind: kinds.get(t.job_key) === 'group' ? 'group' : 'job',
			origin: 'schedule',
			status: 'projected',
			triggerName: t.name,
			start: due,
			end: new Date(Date.parse(due) + typicalMs(bars, t.job_key)).toISOString()
		});
	}
	const triggers = new Map(reads.triggers.map((t) => [t.trigger_id, t]));
	const markers: TimelineTickMarker[] = reads.ticks.flatMap((tick) => {
		const trigger = triggers.get(tick.trigger_id);
		if (!trigger || tick.outcome !== 'covered' || tick.scheduled_for < window.since || tick.scheduled_for >= window.until) return [];
		return [
			{
				tickId: tick.tick_id,
				triggerId: tick.trigger_id,
				triggerName: trigger.name,
				jobKey: trigger.job_key,
				scheduledFor: tick.scheduled_for,
				outcome: 'covered',
				coveredByRunId: tick.covered_by_run_id ?? null,
				reason: null
			}
		];
	});
	return { now: nowIso, jobKeys: reads.jobs.map((j) => j.job_key), bars: bars.sort((a, b) => a.start.localeCompare(b.start)), markers };
}

/** The median length of the job's successful runs on the timeline. */
function typicalMs(bars: TimelineBar[], jobKey: string): number {
	const lengths = bars
		.filter((b) => b.jobKey === jobKey && b.status === 'success')
		.map((b) => Date.parse(b.end) - Date.parse(b.start))
		.sort((a, b) => a - b);
	return lengths.length ? lengths[Math.floor(lengths.length / 2)] : FALLBACK_TYPICAL_MS;
}
