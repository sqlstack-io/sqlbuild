import type { components } from '$control/schema.d.ts';
import type { Job, JobDetail, Run, RunRef, Tick, Trigger, TriggerDetail, TriggerKind } from './types/operate';

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
		lastRun: newest(runs, (r) => r.triggerId === wire.trigger_id),
		sourceFile: null,
		definitionChanged: false,
		warning: null
	};
}

/** A trigger's detail; parts with no server source are null and their widgets stay hidden. */
export function triggerDetailOf(trigger: Trigger, wire: Wire['TriggerRecord'], ticks: Tick[]): TriggerDetail {
	const d = wire.definition_json;
	return {
		...trigger,
		declaration: null,
		targets: null,
		catchUp: null,
		upcoming: null,
		calendar: null,
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
		stateHistory: null
	};
}

export function toJob(wire: Wire['JobRecord'], triggers: Trigger[], runs: Run[]): Job {
	return {
		key: wire.job_key,
		kind: wire.kind === 'group' ? 'group' : 'job',
		description: wire.description,
		// The server stores the resolved node list, not the selector text.
		select: null,
		members: null,
		nodeCount: wire.selection.length,
		overlap: null,
		onFailure: null,
		concurrencyGroups: wire.concurrency_groups,
		priority: null,
		owner: null,
		sourceFile: null,
		triggerIds: triggers.filter((t) => t.jobKey === wire.job_key).map((t) => t.id),
		lastRun: newest(runs, (r) => r.jobKey === wire.job_key)
	};
}

export function jobDetailOf(job: Job, wire: Wire['JobRecord']): JobDetail {
	return {
		...job,
		declaration: null,
		exclude: [],
		settings: null,
		resolvedNodes: wire.selection,
		concurrency: null,
		targets: null,
		groupMembers: null,
		criticalNodes: [],
		memberOf: null,
		cursorType: null
	};
}
