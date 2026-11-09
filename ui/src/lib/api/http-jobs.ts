import type { components } from '$control/schema.d.ts';
import { ApiError } from './client';
import type { CursorWindow, Job, JobDetail, JobGroupMember, Run, RunRef, Tick, Trigger, TriggerDetail, TriggerKind, TriggerStateEvent, TriggerStateEventKind } from './types/operate';

type Wire = components['schemas'];

const refOf = (run: Run): RunRef => ({ id: run.id, status: run.status, startedAt: run.startedAt, endedAt: run.endedAt });

/** The newest listed run matching, as listed newest first. */
const newest = (runs: Run[], match: (run: Run) => boolean): RunRef | null => {
	const run = runs.find(match);
	return run ? refOf(run) : null;
};

function isSchedule(declaration: Wire['TriggerDeclaration']): declaration is Wire['TriggerDefinition'] {
	return 'schedule' in declaration;
}

export function triggerKindOf(declaration: Wire['TriggerDeclaration']): TriggerKind {
	if (isSchedule(declaration)) return declaration.schedule.kind === 'cron' ? 'cron_schedule' : 'calendar_schedule';
	const kind = declaration.when.kind;
	if (kind === 'run_event') return 'job_sensor';
	if (kind === 'platform_event') return 'platform_sensor';
	if (kind === 'custom') return 'custom_sensor';
	return 'node_sensor';
}

/** The declared cadence or condition in words. */
export function cadenceOf(declaration: Wire['TriggerDeclaration']): string {
	if (isSchedule(declaration)) {
		const s = declaration.schedule;
		return s.kind === 'cron' ? `${s.expression} (${s.timezone})` : `${s.times.length} calendar time${s.times.length === 1 ? '' : 's'}`;
	}
	const w = declaration.when;
	if (w.kind === 'run_event') return `when ${w.job_key} ${w.kinds.join(' or ')}`;
	if (w.kind === 'node_event') return `when ${w.selection.join(', ')} ${w.kinds.join(' or ')}`;
	if (w.kind === 'source_new_data') return `when ${w.selection.join(', ')} has new data`;
	if (w.kind === 'source_stale') return `when ${w.selection.join(', ')} is stale for ${w.after_seconds}s`;
	if (w.kind === 'platform_event') return `on platform events: ${w.kinds.join(', ')}`;
	return `custom check ${w.function}`;
}

export function toTick(wire: Wire['TickRecord']): Tick {
	return {
		id: wire.tick_id,
		scheduledFor: wire.scheduled_for,
		evaluatedAt: wire.evaluated_at,
		outcome: wire.outcome,
		runId: wire.run_id ?? null,
		coveredByRunId: wire.covered_by_run_id ?? null,
		reason: null,
		lateBySeconds: Math.max(0, Math.round((Date.parse(wire.evaluated_at) - Date.parse(wire.scheduled_for)) / 1000)),
		catchUp: false
	};
}

export function toTrigger(wire: Wire['TriggerRecord'], runs: Run[], lastTick: Tick | null): Trigger {
	const d = wire.definition_json;
	const schedule = isSchedule(d) ? d.schedule : null;
	return {
		id: wire.trigger_id,
		name: wire.name,
		type: schedule ? 'schedule' : 'sensor',
		kind: triggerKindOf(d),
		description: d.description,
		jobKey: wire.job_key,
		cadence: cadenceOf(d),
		cron: schedule?.kind === 'cron' ? schedule.expression : null,
		timezone: schedule?.kind === 'cron' ? schedule.timezone : null,
		defaultState: d.default_status ?? 'running',
		state: wire.status,
		nextDueAt: wire.next_due_at ?? null,
		lastTick,
		// Group children copy the group's trigger, so only top-level runs are the trigger's own.
		lastRun: newest(runs, (r) => r.triggerId === wire.trigger_id && !r.parentRunId),
		sourceFile: null,
		definitionChanged: false,
		warning: null
	};
}

/** How many upcoming due times a calendar schedule lists. */
const UPCOMING_LIMIT = 5;

/**
 * The server's next due time, and for calendars the stored times from it on, which the scheduler
 * takes in order. Later cron times would need cron evaluation, which stays on the server.
 */
function upcomingOf(d: Wire['TriggerDeclaration'], nextDueAt: string | null): string[] | null {
	if (!isSchedule(d)) return null;
	if (d.schedule.kind === 'cron') return nextDueAt ? [nextDueAt] : [];
	if (!nextDueAt) return [];
	const next = Date.parse(nextDueAt);
	return d.schedule.times.filter((t) => Date.parse(t) >= next).sort((a, b) => Date.parse(a) - Date.parse(b)).slice(0, UPCOMING_LIMIT);
}

/** A trigger's detail; parts with no server source are null and their widgets stay hidden. */
const CHANGE_KINDS: Record<Wire['TriggerChange']['kind'], TriggerStateEventKind> = {
	enable: 'enabled',
	pause: 'paused',
	reset_default: 'reset_to_default',
	set_cursor: 'cursor_set',
	reset_cursor: 'cursor_reset'
};

/** One operator change, newest first as the server lists them; a person shows by name. */
export function toTriggerStateEvent(wire: Wire['PersonTriggerChange']): TriggerStateEvent {
	const name = wire.actor_name ?? wire.actor;
	return {
		id: wire.event_id,
		at: wire.occurred_at,
		kind: CHANGE_KINDS[wire.change.kind],
		actor: { kind: wire.actor_name ? 'user' : 'system', name, displayName: name },
		reason: wire.reason.trim() || null,
		value: wire.change.kind === 'set_cursor' ? wire.change.time : null
	};
}

export function triggerDetailOf(trigger: Trigger, wire: Wire['TriggerRecord'], ticks: Tick[], history: TriggerStateEvent[]): TriggerDetail {
	const d = wire.definition_json;
	const calendar = isSchedule(d) && d.schedule.kind === 'calendar' ? d.schedule : null;
	const stored = calendar ? calendar.times.map(Date.parse) : [];
	return {
		...trigger,
		declaration: null,
		targets: null,
		catchUp: null,
		upcoming: upcomingOf(d, wire.next_due_at ?? null),
		calendar: calendar && {
			horizon: null,
			storedUntil: stored.length ? new Date(Math.max(...stored)).toISOString() : null,
			lastRefreshedAt: null,
			nextRefreshAt: calendar.refresh_at,
			lowHorizonWarningBelow: null,
			low: false
		},
		sensor: isSchedule(d)
			? null
			: {
					condition: trigger.cadence,
					nodeConditions: [],
					minimumIntervalSeconds: d.minimum_interval_seconds,
					timeoutSeconds: null,
					cursor: null,
					cursorUpdatedAt: null,
					cursorKnown: false,
					currentFingerprint: null,
					lastEvaluatedFingerprint: null
				},
		ticks,
		evaluations: null,
		runRequests: null,
		stateHistory: history
	};
}

export function toJob(wire: Wire['JobRecord'], triggers: Trigger[], runs: Run[]): Job {
	return {
		key: wire.job_key,
		kind: wire.kind === 'group' ? 'group' : 'job',
		description: wire.description,
		select: wire.select ?? null,
		members: null,
		nodeCount: wire.selection.length,
		overlap: null,
		onFailure: null,
		concurrencyGroups: wire.concurrency_groups,
		priority: null,
		owner: null,
		sourceFile: wire.declared_at ? `${wire.declared_at.path}:${wire.declared_at.line}` : null,
		triggerIds: triggers.filter((t) => t.jobKey === wire.job_key).map((t) => t.id),
		lastRun: newest(runs, (r) => r.jobKey === wire.job_key)
	};
}

/** A setting as shown: text and flags as stored, anything else as JSON. */
const shownSetting = (value: unknown): string | boolean => (typeof value === 'string' || typeof value === 'boolean' ? value : JSON.stringify(value));

/** Members in the server's run order; a member's step follows the latest step it waits for. */
function groupMembersOf(members: Wire['JobMember'][]): JobGroupMember[] {
	const stageOf = new Map<string, number>();
	return members.map((m) => {
		const stage = Math.max(-1, ...m.waits_for.map((w) => stageOf.get(w) ?? -1)) + 1;
		stageOf.set(m.job_key, stage);
		// The server reports the waits, not whether lineage or `after` caused each one.
		return { jobKey: m.job_key, upstream: m.waits_for.map((jobKey) => ({ jobKey, source: null })), stage };
	});
}

export function jobDetailOf(job: Job, wire: Wire['JobDetailRecord']): JobDetail {
	return {
		...job,
		members: wire.kind === 'group' ? wire.members.map((m) => m.job_key) : null,
		// Published snapshots record where a job is declared but not its code.
		declaration: null,
		exclude: wire.exclude?.split(/\s+/).filter(Boolean) ?? [],
		settings: Object.fromEntries(Object.entries(wire.settings).map(([name, value]) => [name, shownSetting(value)])),
		resolvedNodes: wire.selection,
		concurrency: null,
		targets: wire.targets.length ? wire.targets : null,
		groupMembers: wire.kind === 'group' ? groupMembersOf(wire.members) : null,
		criticalNodes: wire.critical_nodes,
		memberOf: wire.member_of,
		cursorType: wire.cursor_type === 'integer' || wire.cursor_type === 'timestamp' ? wire.cursor_type : null
	};
}

/**
 * A backfill's bounds as typed: whole numbers on both ends make an integer window, anything else
 * goes as timestamps for the server to validate. Only integers JSON would round are refused here.
 */
export function runWindowOf(window: CursorWindow): Wire['RunWindow'] {
	const start = (window.start ?? '').trim();
	const end = (window.end ?? '').trim();
	if (![start, end].every((v) => /^-?\d+$/.test(v))) return { kind: 'timestamp', start, end };
	const [from, to] = [start, end].map(Number);
	if (![from, to].every(Number.isSafeInteger)) throw new ApiError(`Whole numbers beyond ${Number.MAX_SAFE_INTEGER} can't be sent exactly`, 400);
	return { kind: 'integer', start: from, end: to };
}
