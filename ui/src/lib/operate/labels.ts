// Labels and tones for the OPERATE area's vocabulary. Pure; no side effects.

import { formatStamp } from '$lib/format/time';

import type {
	LivenessState,
	NodeCounts,
	OnFailure,
	OverlapPolicy,
	PoolUse,
	QueueWait,
	RunCost,
	RunEvent,
	RunNodeStatus,
	SensorEvaluationOutcome,
	AbandonReason,
	TickOutcome,
	TriggerKind,
	TriggerStateEventKind
} from '$lib/api';

export const nodeStatusTone: Record<RunNodeStatus, string> = {
	pending: 'var(--sb-text-faint)',
	running: 'var(--sb-running)',
	success: 'var(--sb-success)',
	warn: 'var(--sb-warning)',
	failed: 'var(--sb-error)',
	skipped: 'var(--sb-text-faint)',
	cancelled: 'var(--sb-text-faint)',
	not_run: 'var(--sb-text-faint)'
};

export const nodeStatusLabel: Record<RunNodeStatus, string> = {
	pending: 'Pending',
	running: 'Running',
	success: 'Succeeded',
	warn: 'Warned',
	failed: 'Failed',
	skipped: 'Skipped',
	cancelled: 'Cancelled',
	not_run: 'Not run'
};

/** Rail order: problems and live work first. */
export const NODE_STATUS_ORDER: RunNodeStatus[] = ['failed', 'running', 'warn', 'success', 'skipped', 'cancelled', 'not_run', 'pending'];

export const livenessLabel: Record<LivenessState, string> = {
	healthy: 'Heartbeating',
	unresponsive: 'Unresponsive',
	presumed_failed: 'Presumed failed',
	never_started: 'Never started',
	exited: 'Exited'
};

export const livenessTone: Record<LivenessState, string> = {
	healthy: 'var(--sb-success)',
	unresponsive: 'var(--sb-warning)',
	presumed_failed: 'var(--sb-error)',
	never_started: 'var(--sb-error)',
	exited: 'var(--sb-text-faint)'
};

export function formatCost(cost: RunCost | null): string {
	if (!cost) return '—';
	return `${cost.value < 0.01 ? cost.value.toFixed(4) : cost.value.toFixed(3)} cr`;
}

export const overlapHelp: Record<OverlapPolicy, string> = {
	queue: 'A schedule tick or sensor request while a run of this job is active queues another run after it',
	skip: 'A schedule tick or sensor request while a run of this job is active is skipped and recorded on the tick or check'
};

/** Short plain wording of an overlap policy; the policy name goes in a tooltip. */
export const overlapPlain: Record<OverlapPolicy, string> = {
	queue: 'Queue',
	skip: 'Skip'
};

/** A whole delay in its largest exact unit, e.g. 300 → `5 min`, 5400 → `90 min`, 7200 → `2 h`. */
function delayText(seconds: number): string {
	if (seconds && seconds % 3600 === 0) return `${seconds / 3600} h`;
	if (seconds && seconds % 60 === 0) return `${seconds / 60} min`;
	return `${seconds} s`;
}

/** A `[retry]` policy as a fact beside the "Retries" key, e.g. "2 × 5 min", "1 × 15 min" or "3 × immediate". */
export function onFailureText(policy: OnFailure): string {
	return `${policy.retries} × ${policy.retryDelaySeconds ? delayText(policy.retryDelaySeconds) : 'immediate'}`;
}

/** A job's overlap, retries and priority for a list cell; empty when it uses the defaults. */
export function jobPolicyText(job: { overlap: OverlapPolicy | null; onFailure: OnFailure | null; priority: number | null }): string {
	return [job.overlap === 'skip' ? 'Skip if running' : null, job.onFailure ? `Retry ${onFailureText(job.onFailure)}` : null, job.priority ? `priority ${job.priority}` : null].filter(Boolean).join(' · ');
}

export type WaitPart = { text: string; href?: string; code?: boolean };

/** What went wrong in a finished run, such as "3 failed, 12 skipped"; empty when nothing did. */
export function outcomeCounts(c: NodeCounts): string {
	return [c.failed ? `${c.failed} failed` : null, c.skipped ? `${c.skipped} skipped` : null].filter(Boolean).join(', ');
}

/** `run-4f2a` → `4f2a`, a UUID → its first 8 digits: the short form shown in lists; the full ID stays in links and copies. */
export function shortRunId(id: string): string {
	const short = id.replace(/^run-/, '');
	return /^[0-9a-f]{8}-[0-9a-f]{4}-/i.test(short) ? short.slice(0, 8) : short;
}

/** How many of the jobs sharing a pool are named before "+N more". */
export const SHARED_SHOWN = 3;

/** The docs page the Concurrency panel links to. */
export const CONCURRENCY_DOCS = 'https://sqlbuild.com/docs/concepts/concurrency/';

/**
 * A run's concurrency as the panel states it. A group's limit is the target's, which each member
 * job's run uses unless the job sets its own.
 */
export function runConcurrencyText(limit: number, kind: 'job' | 'group'): string {
	return kind === 'group'
		? `Members without their own limit: at most ${limit} nodes at once`
		: `Inside a run: at most ${limit} of this job's nodes at once`;
}

/** What a job does when triggered while a run of it is active, finishing "If triggered while running: …". */
export const overlapWhileRunning: Record<OverlapPolicy, string> = { queue: 'waits', skip: 'is skipped' };

/**
 * A pool the job takes, in plain words. Normally "supplier_api: takes 1 of 2", against the limit on
 * this server, with the cap as a quiet detail when the server lowered the project's limit. When the
 * job asks for more than this server allows, it takes the whole pool, and `clamped` says so: "Takes
 * the whole warehouse_heavy pool on this server (capped at 1; the project asks for 3)". The other
 * jobs and groups sharing it follow: the first `SHARED_SHOWN`, then `more`.
 */
export function poolText(use: PoolUse): { text: string; detail: string | null; clamped: boolean; shared: string[]; more: number } {
	const shared = use.sharedWith.slice(0, SHARED_SHOWN);
	const more = use.sharedWith.length - shared.length;
	if (use.clamped) {
		// copy-allow: the Concurrency panel's wording, agreed with the product owner
		return { text: `Takes the whole ${use.pool} pool on this server`, detail: `capped at ${use.serverLimit}; the project asks for ${use.amount}`, clamped: true, shared, more };
	}
	const capped = use.serverLimit < use.limit;
	// copy-allow: the Concurrency panel's wording, agreed with the product owner
	return { text: `${use.pool}: takes ${use.amount} of ${use.serverLimit}`, detail: capped ? `capped on this server; the project allows ${use.limit}` : null, clamped: false, shared, more };
}

const runLink = (id: string): WaitPart => ({ text: `run ${shortRunId(id)}`, href: `/runs/${id}` });

/**
 * A queue wait as short linkable facts, e.g. "Writing orders · run 7f3a"; `waitTechnical` gives the
 * scheduler's own wording for the tooltip.
 */
export function describeWait(wait: QueueWait): WaitPart[] {
	switch (wait.kind) {
		case 'write_claim':
			return [
				{ text: 'Writing ' },
				{ text: wait.nodeKey, href: `/catalog/${wait.nodeKey}` },
				...(wait.otherNodes ? [{ text: ` +${wait.otherNodes}` }] : []),
				{ text: ' · ' },
				wait.runId ? runLink(wait.runId) : { text: 'another location' }
			];
		case 'pool_full':
			return [
				{ text: `${wait.pool} pool full · ` },
				...wait.holders.flatMap((id, i) => [...(i ? [{ text: ', ' }] : []), runLink(id)])
			];
		case 'earlier_run':
			return [{ text: 'Behind ' }, wait.runId ? runLink(wait.runId) : { text: 'a run elsewhere' }, { text: ` · ${wait.pool} pool` }];
		case 'job_running':
			return [{ text: 'Job running · ' }, runLink(wait.runId)];
		case 'upstream_job':
			return [{ text: 'After ' }, { text: wait.jobKey || shortRunId(wait.runId), href: `/runs/${wait.runId}` }, ...(wait.attempt ? [{ text: ' · retrying' }] : [])];
		case 'group_parent':
			return [{ text: 'Group run ' }, { text: shortRunId(wait.runId), href: `/runs/${wait.runId}` }, { text: ' not started' }];
		case 'retry_delay':
			return [{ text: `Retry ${wait.attempt} at ${formatStamp(wait.dueAt)}` }];
	}
}

/** The wait as one plain line without links, for status lines and summaries. */
export function waitText(wait: QueueWait): string {
	return describeWait(wait)
		.map((p) => p.text)
		.join('');
}

/** The scheduler's own name for a wait, for tooltips and debugging. */
export function waitTechnical(wait: QueueWait): string {
	switch (wait.kind) {
		case 'write_claim':
			return `write claim held by ${wait.runId ?? 'another location or target'} on ${wait.nodeKey}${wait.otherNodes ? ` (+${wait.otherNodes})` : ''}`;
		case 'pool_full':
			return `pool ${wait.pool} full, slots held by ${wait.holders.join(', ')}`;
		case 'earlier_run':
			return `${wait.runId ?? 'a run elsewhere'} is ahead on pool ${wait.pool} (priority, then queue order)`;
		case 'job_running':
			return `job already running in ${wait.runId}`;
		case 'upstream_job':
			return `upstream job ${wait.jobKey} (${wait.runId}) still active${wait.attempt ? `, attempt ${wait.attempt}` : ''}`;
		case 'group_parent':
			return `group run ${wait.runId} not running yet`;
		case 'retry_delay':
			return `retry ${wait.attempt}, not before ${wait.dueAt}`;
	}
}

export const tickOutcomeTone: Record<TickOutcome, string> = {
	requested: 'var(--sb-success)',
	skipped: 'var(--sb-text-faint)',
	covered: 'var(--sb-running)',
	failed: 'var(--sb-error)'
};

export const tickOutcomeText: Record<TickOutcome, string> = {
	requested: 'Run started',
	skipped: 'Skipped',
	covered: 'Merged',
	failed: 'Failed'
};

/** What each tick outcome means, with the scheduler's own name for debugging. */
export const tickOutcomeHelp: Record<TickOutcome, string> = {
	requested: 'It was due and started a run (requested)',
	skipped: 'It was due while a run of this job was still active, and the job’s overlap is skip (skipped)',
	covered: 'A run of this job was already waiting, so this one merged into it (covered)',
	failed: 'The scheduler couldn’t start a run for it (failed)'
};

export const evaluationTone: Record<SensorEvaluationOutcome, string> = {
	running: 'var(--sb-running)',
	handled: 'var(--sb-success)',
	skipped: 'var(--muted-foreground)',
	started: 'var(--sb-success)',
	failed: 'var(--sb-error)',
	timed_out: 'var(--sb-error)',
	abandoned: 'var(--sb-warning)'
};

export const evaluationLabel: Record<SensorEvaluationOutcome, string> = {
	running: 'Running',
	handled: 'Handled',
	skipped: 'Skipped',
	started: 'Started',
	failed: 'Failed',
	timed_out: 'Timed out',
	abandoned: 'Abandoned'
};

export const evaluationHelp: Record<SensorEvaluationOutcome, string> = {
	running: 'The function is being called (running)',
	handled: 'The function returned None (handled)',
	skipped: 'The function returned a SkipReason (skipped)',
	started: 'The function returned run requests, which started runs (started)',
	failed: 'The function raised, or returned something it may not; the event is retried after the minimum interval (failed)',
	timed_out: 'The function took longer than its timeout; the event is retried after the minimum interval (timed_out)',
	abandoned: 'Ended before it finished; only a location that did not answer counts as failing (abandoned)'
};

export const abandonReasonLabel: Record<AbandonReason, string> = {
	unanswered: "The location didn't answer in time",
	superseded: 'The location replaced it, as when it restarted',
	republished: 'The location published again',
	paused: 'The sensor was paused',
	cursor_reset: 'The cursor was reset or moved',
	leader_changed: 'Another server took over scheduling'
};

export const triggerKindLabel: Record<TriggerKind, string> = {
	cron_schedule: 'Schedule',
	calendar_schedule: 'Calendar schedule',
	job_sensor: 'Job sensor',
	node_sensor: 'Data sensor',
	check_sensor: 'Check sensor',
	platform_sensor: 'Problem sensor',
	polling_sensor: 'Polling sensor'
};

export const triggerKindHelp: Record<TriggerKind, string> = {
	cron_schedule: 'Runs at times set by a cron expression (cron_schedule)',
	calendar_schedule: 'Runs at times listed by a calendar, computed ahead of time (calendar_schedule)',
	job_sensor: 'Calls its function when another job or group finishes (job_sensor)',
	node_sensor: 'Calls its function when a model or source is rebuilt, fails or gets new data (node_sensor)',
	check_sensor: 'Calls its function when a check, audit or SQL test fails or warns in a run (check_sensor)',
	platform_sensor: 'Calls its function when a problem is recorded, e.g. to post a message (platform_sensor)',
	polling_sensor: 'Calls its function every interval, which decides whether to start runs (polling_sensor)'
};

export const stateEventLabel: Record<TriggerStateEventKind, string> = {
	enabled: 'Turned on',
	paused: 'Paused',
	reset_to_default: 'Reset to code',
	cursor_set: 'Position set',
	cursor_reset: 'Started over',
	event_skipped: 'Event skipped'
};

export const eventTone = (event: RunEvent): string =>
	event.level === 'error'
		? 'var(--sb-error)'
		: event.level === 'warn'
			? 'var(--sb-warning)'
			: event.producer === 'scheduler'
				? 'var(--sb-stale)'
				: event.type.endsWith('_completed')
					? 'var(--sb-success)'
					: 'var(--sb-running)';

/** e.g. `42s`, `3m 05s`, `2h 10m`. */
export function formatSeconds(seconds: number): string {
	const s = Math.max(0, Math.round(seconds));
	if (s < 60) return `${s}s`;
	if (s < 3600) return `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, '0')}s`;
	return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}

/** `HH:MM:SS.mmm` local time for event tables. */
export function clockTime(iso: string): string {
	const d = new Date(iso);
	const pad = (n: number, w = 2) => String(n).padStart(w, '0');
	return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}

/** The selectors a run of part of a job was narrowed with, as `sqb build` options. */
export function narrowingText(narrowing: { select: string[]; exclude: string[] }): string {
	return [narrowing.select.length ? `--select ${narrowing.select.join(' ')}` : '', narrowing.exclude.length ? `--exclude ${narrowing.exclude.join(' ')}` : ''].filter(Boolean).join(' ');
}

/** A node key as a run shows it: `seed:countries` reads `countries`; the kind shows elsewhere, and the full key on hover. */
export function nodeLabel(key: string): string {
	return key.replace(/^[a-z_]+:(?=.)/, '');
}
