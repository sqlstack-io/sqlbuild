// Neutral example data for the OPERATE area: a fictional shop's jobs, job
// groups, schedules, sensors and their run history. Everything is relative to
// `now`, and schedule crons are generated so their due times line up with the
// recorded ticks.

import type {
	CancelProgress,
	CatchUpPolicy,
	ConcurrencyGroup,
	JobDetail,
	LaunchVerb,
	Principal,
	Run,
	RunOrigin,
	SensorEvaluation,
	SensorRunRequest,
	Tick,
	TriggerDetail,
	TriggerKind,
	TriggerState,
	TriggerStateEvent
} from '../types';
import { CI_ACCOUNT, DEMO_USER, OPS_USER, SCHEDULER, SENSOR_DAEMON } from './principals';
import { resolveSelection, writeSetOf } from './operate-graph';
import type { RunEnd, RunRecord, RunScript } from './run-detail';

const MIN = 60_000;
export const IMAGE_ID = 'sha256:4be1c0d97e2a51f3c6a8b0e9d4f7a2c1e5b8d3f6a9c2e7b4d1f8a5c3e6b9d2f4';
export const PREVIOUS_IMAGE_ID = 'sha256:9a3f6c1e8d2b5a7f4c9e1d6b3a8f5c2e7d4b1a9f6c3e8d5b2a7f4c1e9d6b3a8f';
export const SNAPSHOT_GENERATION = 42;

// ---- jobs and groups ----

type JobDef = Omit<JobDetail, 'nodeCount' | 'resolvedNodes' | 'lastRun' | 'triggerIds' | 'concurrency' | 'memberOf' | 'cursorType' | 'groupMembers'> & {
	after: Record<string, string[]>;
};

export const CONCURRENCY_LIMITS: Record<string, number> = { warehouse_heavy: 1, 'target:prod': 4 };

const decl = (sourceFile: string, line: number, code: string) => ({ sourceFile, line, code });

export const JOB_DEFS: JobDef[] = [
	{
		key: 'load_sources',
		kind: 'job',
		description: 'Ingest partner order files into raw__orders',
		select: 'tag:ingest',
		exclude: [],
		members: [],
		overlap: 'queue',
		onFailure: 'skip_dependents',
		concurrencyGroups: [],
		priority: 5,
		sourceFile: 'orchestration/jobs.py',
		settings: {},
		targets: null,
		owner: 'ingest',
		criticalNodes: [],
		after: {},
		declaration: decl(
			'orchestration/jobs.py',
			14,
			`load_sources = job(
    "load_sources",
    description="Ingest partner order files into raw__orders",
    select="tag:ingest",
    overlap="queue",
    priority=5,
    owner="ingest",
)`
		)
	},
	{
		key: 'core_build',
		kind: 'job',
		description: 'Staging and marts models',
		select: 'group:staging group:marts',
		exclude: [],
		members: [],
		overlap: 'coalesce',
		onFailure: 'skip_dependents',
		concurrencyGroups: ['warehouse_heavy'],
		priority: 0,
		sourceFile: 'orchestration/jobs.py',
		settings: { '--concurrency': '8' },
		targets: null,
		owner: 'data-platform',
		criticalNodes: ['fact_orders'],
		after: {},
		declaration: decl(
			'orchestration/jobs.py',
			24,
			`core_build = job(
    "core_build",
    description="Staging and marts models",
    select=["group:staging", "group:marts"],
    overlap="coalesce",
    on_failure="skip_dependents",
    concurrency_groups=[warehouse_heavy],
    settings={"--concurrency": 8},
    owner="data-platform",
    schedule=schedule(cron="{cron:core_build}", timezone="UTC"),
)`
		)
	},
	{
		key: 'marts_only',
		kind: 'job',
		description: 'Marts models only, reading the current staging layer',
		select: 'group:marts',
		exclude: [],
		members: [],
		overlap: 'coalesce',
		onFailure: 'skip_dependents',
		concurrencyGroups: [],
		priority: 0,
		sourceFile: 'orchestration/jobs.py',
		settings: {},
		targets: ['prod', 'staging'],
		owner: 'analytics',
		criticalNodes: [],
		after: {},
		declaration: decl(
			'orchestration/jobs.py',
			37,
			`marts_only = job(
    "marts_only",
    description="Marts models only, reading the current staging layer",
    select="group:marts",
    targets=["prod", "staging"],
    owner="analytics",
)`
		)
	},
	{
		key: 'activity_microbatch',
		kind: 'job',
		description: 'Incremental activity models, four microbatches at a time',
		select: 'group:activity',
		exclude: [],
		members: [],
		overlap: 'coalesce',
		onFailure: 'skip_dependents',
		concurrencyGroups: [],
		priority: 0,
		sourceFile: 'orchestration/activity.py',
		settings: { '--max-microbatches': '4' },
		targets: null,
		owner: 'analytics',
		criticalNodes: [],
		after: {},
		declaration: decl(
			'orchestration/activity.py',
			8,
			`activity_microbatch = job(
    "activity_microbatch",
    description="Incremental activity models, four microbatches at a time",
    select="group:activity",
    settings={"--max-microbatches": 4},
    owner="analytics",
    schedule=schedule(cron="{cron:activity_microbatch}", timezone="UTC",
                      catch_up=catch_up.all(max=3)),
)`
		)
	},
	{
		key: 'revenue_refresh',
		kind: 'job',
		description: 'Daily revenue, its report and the CSV export',
		select: 'daily_revenue revenue_report export_revenue_csv',
		exclude: [],
		members: [],
		overlap: 'coalesce',
		onFailure: 'skip_dependents',
		concurrencyGroups: [],
		priority: 0,
		sourceFile: 'orchestration/finance.py',
		settings: { '--fail-fast': true },
		targets: ['prod'],
		owner: 'finance-data',
		criticalNodes: [],
		after: {},
		declaration: decl(
			'orchestration/finance.py',
			11,
			`revenue_refresh = job(
    "revenue_refresh",
    description="Daily revenue, its report and the CSV export",
    select=["daily_revenue", "revenue_report", "export_revenue_csv"],
    settings={"--fail-fast": True},
    targets=["prod"],
    owner="finance-data",
    schedule=schedule(cron="{cron:revenue_refresh}", timezone="UTC"),
)`
		)
	},
	{
		key: 'customer_segments',
		kind: 'job',
		description: 'Customer status segments; hourly and in the close-of-business group',
		select: 'customer_status',
		exclude: [],
		members: [],
		overlap: 'allow',
		onFailure: 'skip_dependents',
		concurrencyGroups: [],
		priority: 0,
		sourceFile: 'orchestration/finance.py',
		settings: {},
		targets: null,
		owner: 'analytics',
		criticalNodes: [],
		after: {},
		declaration: decl(
			'orchestration/finance.py',
			23,
			`customer_segments = job(
    "customer_segments",
    description="Customer status segments; hourly and in the close-of-business group",
    select="customer_status",
    overlap="allow",
    owner="analytics",
)`
		)
	},
	{
		key: 'reporting_exports',
		kind: 'job',
		description: 'Reporting tables and the revenue export',
		select: 'group:reporting group:exports',
		exclude: [],
		members: [],
		overlap: 'queue',
		onFailure: 'skip_dependents',
		concurrencyGroups: ['warehouse_heavy'],
		priority: 10,
		sourceFile: 'orchestration/reporting.py',
		settings: {},
		targets: ['prod'],
		owner: 'analytics',
		criticalNodes: [],
		after: {},
		declaration: decl(
			'orchestration/reporting.py',
			6,
			`reporting_exports = job(
    "reporting_exports",
    description="Reporting tables and the revenue export",
    select=["group:reporting", "group:exports"],
    overlap="queue",
    concurrency_groups=[warehouse_heavy],
    priority=10,
    targets=["prod"],
)`
		)
	},
	{
		key: 'nightly_all',
		kind: 'group',
		description: 'Sources, the core build, activity models, then reporting',
		select: null,
		exclude: [],
		members: ['load_sources', 'core_build', 'activity_microbatch', 'reporting_exports'],
		overlap: 'coalesce',
		onFailure: 'skip_dependents',
		concurrencyGroups: [],
		priority: 0,
		sourceFile: 'orchestration/groups.py',
		settings: {},
		targets: ['prod'],
		owner: 'data-platform',
		criticalNodes: ['fact_orders'],
		after: {},
		declaration: decl(
			'orchestration/groups.py',
			5,
			`nightly_all = job_group(
    "nightly_all",
    description="Sources, the core build, activity models, then reporting",
    jobs=[load_sources, core_build, activity_microbatch, reporting_exports],
    critical=["fact_orders"],
    targets=["prod"],
    schedule=schedule(cron="{cron:nightly_all}", timezone="UTC"),
)`
		)
	},
	{
		key: 'close_of_business',
		kind: 'group',
		description: 'Revenue first, then activity and customer segments',
		select: null,
		exclude: [],
		members: ['revenue_refresh', 'activity_microbatch', 'customer_segments'],
		overlap: 'coalesce',
		onFailure: 'stop_group',
		concurrencyGroups: ['warehouse_heavy'],
		priority: 20,
		sourceFile: 'orchestration/groups.py',
		settings: {},
		targets: ['prod'],
		owner: 'finance-data',
		criticalNodes: ['daily_revenue'],
		after: { customer_segments: ['revenue_refresh'] },
		declaration: decl(
			'orchestration/groups.py',
			16,
			`close_of_business = job_group(
    "close_of_business",
    description="Revenue first, then activity and customer segments",
    jobs=[revenue_refresh, activity_microbatch, customer_segments],
    # Segments read closed revenue through an external lookup lineage can't see.
    after={customer_segments: [revenue_refresh]},
    on_failure="stop_group",
    critical=["daily_revenue"],
    concurrency_groups=[warehouse_heavy],
    priority=20,
    targets=["prod"],
)`
		)
	}
];

export function jobDef(key: string | null): JobDef | undefined {
	return key ? JOB_DEFS.find((j) => j.key === key) : undefined;
}

export function selectorTerms(job: JobDef): string[] {
	return job.select ? job.select.split(/\s+/).filter(Boolean) : [];
}

/** Write set of a job; for groups, the union of its members. */
export function jobNodes(key: string): string[] {
	const job = jobDef(key);
	if (!job) return [];
	if (job.kind === 'group') return [...new Set(job.members.flatMap(jobNodes))];
	return writeSetOf(selectorTerms(job), job.exclude);
}

export function concurrencyGroupsOf(job: JobDef | undefined): ConcurrencyGroup[] {
	return (job?.concurrencyGroups ?? []).map((key) => ({ key, limit: CONCURRENCY_LIMITS[key] ?? 1, holders: [] }));
}

// ---- triggers ----

type ScheduleSpec = {
	everyMin: number;
	lastTickAgoMin: number;
	historyDays: number;
	/** Nothing has evaluated the due time after the last tick, so the schedule is overdue. */
	overdue?: boolean;
};

export type TriggerDef = {
	name: string;
	kind: TriggerKind;
	description: string;
	jobKey: string | null;
	cadence: string;
	defaultState: TriggerState;
	state: TriggerState;
	sourceFile: string;
	line: number;
	code: string;
	targets: string[] | null;
	catchUp: CatchUpPolicy | null;
	schedule?: ScheduleSpec;
	sensor?: {
		condition: string;
		nodeConditions: string[];
		minimumIntervalSeconds: number;
		timeoutSeconds: number | null;
	};
	definitionChanged?: boolean;
};

export const triggerId = (name: string) => `trg-${name.replace(/_/g, '-')}`;

export const TRIGGER_DEFS: TriggerDef[] = [
	{
		name: 'core_build',
		kind: 'cron_schedule',
		description: 'Core build every four hours',
		jobKey: 'core_build',
		cadence: 'Every 4 hours',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/jobs.py',
		line: 33,
		code: '',
		targets: null,
		catchUp: { mode: 'latest', max: null },
		schedule: { everyMin: 240, lastTickAgoMin: 25, historyDays: 8 }
	},
	{
		name: 'nightly_all',
		kind: 'cron_schedule',
		description: 'The nightly group run',
		jobKey: 'nightly_all',
		cadence: 'Daily',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/groups.py',
		line: 12,
		code: '',
		targets: ['prod'],
		catchUp: { mode: 'latest', max: null },
		schedule: { everyMin: 1440, lastTickAgoMin: 9 * 60 + 40, historyDays: 8 }
	},
	{
		name: 'activity_microbatch',
		kind: 'cron_schedule',
		description: 'Activity microbatches every 15 minutes',
		jobKey: 'activity_microbatch',
		cadence: 'Every 15 minutes',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/activity.py',
		line: 14,
		code: '',
		targets: null,
		catchUp: { mode: 'all', max: 3 },
		schedule: { everyMin: 15, lastTickAgoMin: 4, historyDays: 2 }
	},
	{
		name: 'revenue_refresh',
		kind: 'cron_schedule',
		description: 'Daily revenue refresh',
		jobKey: 'revenue_refresh',
		cadence: 'Daily',
		defaultState: 'running',
		state: 'paused',
		sourceFile: 'orchestration/finance.py',
		line: 18,
		code: '',
		targets: ['prod'],
		catchUp: { mode: 'latest', max: null },
		schedule: { everyMin: 1440, lastTickAgoMin: 27 * 60, historyDays: 8 }
	},
	{
		name: 'segments_hourly',
		kind: 'cron_schedule',
		description: 'Customer segments every hour',
		jobKey: 'customer_segments',
		cadence: 'Hourly',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/schedules.py',
		line: 13,
		code: `segments_hourly = schedule(
    "segments_hourly",
    description="Customer segments every hour",
    job=customer_segments,
    cron="{cron:segments_hourly}",
    timezone="UTC",
)`,
		targets: null,
		catchUp: { mode: 'latest', max: null },
		schedule: { everyMin: 60, lastTickAgoMin: 72, historyDays: 2, overdue: true }
	},
	{
		name: 'marts_hourly',
		kind: 'cron_schedule',
		description: 'Hourly marts refresh, off unless enabled',
		jobKey: 'marts_only',
		cadence: 'Hourly',
		defaultState: 'paused',
		state: 'paused',
		sourceFile: 'orchestration/schedules.py',
		line: 4,
		code: `marts_hourly = schedule(
    "marts_hourly",
    description="Hourly marts refresh, off unless enabled",
    job=marts_only,
    cron="{cron:marts_hourly}",
    timezone="UTC",
    default_status="stopped",
)`,
		targets: null,
		catchUp: { mode: 'latest', max: null },
		schedule: { everyMin: 60, lastTickAgoMin: 17, historyDays: 0 }
	},
	{
		name: 'business_days',
		kind: 'calendar_schedule',
		description: 'One hour after the office closes on business days, skipping public holidays',
		jobKey: 'close_of_business',
		cadence: 'Business days, computed 30 days ahead',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/calendars.py',
		line: 9,
		code: `@calendar_schedule(job=close_of_business, horizon="30d", timezone="UTC",
                   description="One hour after the office closes on business days, "
                               "skipping public holidays")
def business_days(window, holidays):
    return [day.close_at + timedelta(hours=1)
            for day in holidays.business_days(window.start, window.end)]`,
		targets: ['prod'],
		catchUp: { mode: 'latest', max: null },
		schedule: { everyMin: 1440, lastTickAgoMin: 15 * 60 + 5, historyDays: 8 }
	},
	{
		name: 'on_payments_landed',
		kind: 'node_sensor',
		description: 'Refresh the marts when raw__payments has new data',
		jobKey: 'marts_only',
		cadence: 'When raw__payments has new data',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/sensors.py',
		line: 6,
		code: `on_payments_landed = sensor(
    "on_payments_landed",
    description="Refresh the marts when raw__payments has new data",
    when=source_has_new_data("raw__payments"),
    run=marts_only,
    minimum_interval="1m",
)`,
		targets: null,
		catchUp: null,
		sensor: { condition: 'source_has_new_data("raw__payments")', nodeConditions: [], minimumIntervalSeconds: 60, timeoutSeconds: null }
	},
	{
		name: 'after_core_build',
		kind: 'job_sensor',
		description: 'Reporting after a successful core build that rebuilt daily_revenue',
		jobKey: 'reporting_exports',
		cadence: 'When core_build succeeds and daily_revenue executed',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/sensors.py',
		line: 15,
		code: `after_core_build = sensor(
    "after_core_build",
    description="Reporting after a successful core build that rebuilt daily_revenue",
    when=job_succeeded(core_build, nodes=all_executed_successfully(["daily_revenue"])),
    run=reporting_exports,
)`,
		targets: ['prod'],
		catchUp: null,
		sensor: {
			condition: 'job_succeeded(core_build)',
			nodeConditions: ['daily_revenue executed successfully in that run'],
			minimumIntervalSeconds: 0,
			timeoutSeconds: null
		}
	},
	{
		name: 'on_fact_orders_rebuilt',
		kind: 'node_sensor',
		description: 'Activity models whenever fact_orders is rebuilt, in any run',
		jobKey: 'activity_microbatch',
		cadence: 'When fact_orders is rebuilt successfully (any run)',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/sensors.py',
		line: 23,
		code: `on_fact_orders_rebuilt = sensor(
    "on_fact_orders_rebuilt",
    description="Activity models whenever fact_orders is rebuilt, in any run",
    when=node_succeeded("fact_orders"),
    run=activity_microbatch,
)`,
		targets: null,
		catchUp: null,
		sensor: { condition: 'node_succeeded("fact_orders")', nodeConditions: [], minimumIntervalSeconds: 0, timeoutSeconds: null }
	},
	{
		name: 'platform_health_webhook',
		kind: 'platform_sensor',
		description: 'Post platform events to the team chat room',
		jobKey: null,
		cadence: 'On tick_overdue, scheduler_no_leader, location_broken, launch_failed',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/health.py',
		line: 4,
		code: `@sensor(when=platform_event(kinds=[K.TICK_OVERDUE, K.SCHEDULER_NO_LEADER,
                                    K.LOCATION_BROKEN, K.LAUNCH_FAILED]),
        description="Post platform events to the team chat room")
def platform_health_webhook(ctx, chat_webhook):
    for event in ctx.events:
        chat_webhook.post(event.summary())`,
		targets: null,
		catchUp: null,
		sensor: {
			condition: 'platform_event(kinds=[tick_overdue, scheduler_no_leader, location_broken, launch_failed])',
			nodeConditions: [],
			minimumIntervalSeconds: 30,
			timeoutSeconds: 30
		}
	},
	{
		name: 'partner_feed_poll',
		kind: 'custom_sensor',
		description: 'Load each new partner order file once',
		jobKey: 'load_sources',
		cadence: 'Every 5 minutes (minimum interval)',
		defaultState: 'running',
		state: 'running',
		sourceFile: 'orchestration/sensors.py',
		line: 31,
		code: `@sensor(minimum_interval="5m", timeout="60s",
        description="Load each new partner order file once")
def partner_feed_poll(ctx, orders_api):
    files = orders_api.list_files(modified_after=ctx.cursor)
    if not files:
        return SkipReason("No new files")
    for f in files:
        yield RunRequest(job=load_sources, run_key=f.name)
    ctx.cursor = files[-1].modified_at`,
		targets: null,
		catchUp: null,
		definitionChanged: true,
		sensor: { condition: 'partner_feed_poll(ctx, orders_api)', nodeConditions: [], minimumIntervalSeconds: 300, timeoutSeconds: 60 }
	}
];

export function triggerDef(name: string | null): TriggerDef | undefined {
	return name ? TRIGGER_DEFS.find((t) => t.name === name) : undefined;
}

/** A cron expression whose due times include `lastTick` and repeat every `everyMin`. */
export function cronFor(everyMin: number, lastTick: number): string {
	const d = new Date(lastTick);
	const m = d.getUTCMinutes();
	const h = d.getUTCHours();
	if (everyMin < 60) return `${m % everyMin}/${everyMin} * * * *`;
	if (everyMin === 60) return `${m} * * * *`;
	if (everyMin < 1440) return `${m} ${h % (everyMin / 60)}/${everyMin / 60} * * *`;
	return `${m} ${h} * * *`;
}

// ---- run records ----

type Seed = {
	id: string;
	origin: RunOrigin;
	job: string | null;
	trigger?: string;
	tick?: number;
	by: Principal;
	queuedAgo: number;
	startedAgo?: number;
	durationMin?: number;
	fail?: string[];
	warn?: string[];
	errors?: Record<string, string>;
	end?: RunEnd;
	select?: string[];
	exclude?: string[];
	verb?: LaunchVerb;
	flags?: Record<string, string | true>;
	nodes?: string[];
	priority?: number;
	runKey?: string;
	rerunOf?: string;
	parent?: string;
	cancel?: { requestedAgoSec: number; by: Principal; sigtermAgoSec: number | null; done: boolean };
	lastHeartbeatAgoSec?: number;
	running?: boolean;
	launchedNoStart?: boolean;
	image?: string;
	generation?: number;
	reason?: string;
};

function rng(seed: number) {
	return () => {
		seed |= 0;
		seed = (seed + 0x6d2b79f5) | 0;
		let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

export type TickRecord = Tick & { triggerName: string };

export type OperateFixtures = {
	records: RunRecord[];
	ticks: TickRecord[];
	crons: Record<string, string>;
	/** Last recorded tick time per schedule. */
	lastTickAt: Record<string, number>;
	evaluations: Record<string, SensorEvaluation[]>;
	runRequests: Record<string, SensorRunRequest[]>;
	stateHistory: Record<string, TriggerStateEvent[]>;
	cursors: Record<string, { cursor: string | null; updatedAt: number | null }>;
	calendar: { storedUntil: number; lastRefreshedAt: number; times: number[] };
};

export function createOperateFixtures(now: number): OperateFixtures {
	const ago = (minutes: number) => now - minutes * MIN;
	const isoAgo = (minutes: number) => new Date(ago(minutes)).toISOString();
	const floorMin = (t: number) => Math.floor(t / MIN) * MIN;

	const records: RunRecord[] = [];
	const ticks: TickRecord[] = [];
	const lastTickAt: Record<string, number> = {};
	const crons: Record<string, string> = {};
	for (const t of TRIGGER_DEFS) {
		if (!t.schedule) continue;
		lastTickAt[t.name] = floorMin(ago(t.schedule.lastTickAgoMin));
		crons[t.name] = cronFor(t.schedule.everyMin, lastTickAt[t.name]);
	}
	const tickAt = (name: string, index: number) => lastTickAt[name] - index * (triggerDef(name)!.schedule!.everyMin * MIN);
	const tickId = (name: string, at: number) => `tick-${name.replace(/_/g, '-')}-${(at / MIN).toString(36)}`;

	function record(seed: Seed, at?: { queued: number; started: number | null }): RunRecord {
		const job = jobDef(seed.job);
		const trig = triggerDef(seed.trigger ?? null);
		const queued = at?.queued ?? ago(seed.queuedAgo);
		const started = at ? at.started : seed.startedAgo === undefined ? null : ago(seed.startedAgo);
		const plannedMs = (seed.durationMin ?? 5) * MIN;
		const neverStarted = seed.end === 'never_started';
		const ended = seed.running ? null : started != null ? started + plannedMs : neverStarted ? queued + 2.5 * MIN : null;
		const launched = started != null ? started - 4000 : neverStarted || seed.launchedNoStart ? queued + 3000 : null;
		const select = seed.select ?? (job ? selectorTerms(job) : []);
		const exclude = seed.exclude ?? job?.exclude ?? [];
		const flags = seed.flags ?? Object.fromEntries(Object.entries(job?.settings ?? {}).map(([k, v]) => [k, v === true ? true : String(v)]));
		const nodes = job?.kind === 'group' ? [] : (seed.nodes ?? writeSetOf(select, exclude));
		const tick = seed.tick != null && trig ? tickAt(trig.name, seed.tick) : null;
		let cancel: CancelProgress | null = null;
		if (seed.cancel) {
			const requested = seed.cancel.done && ended ? ended - seed.cancel.requestedAgoSec * 1000 : now - seed.cancel.requestedAgoSec * 1000;
			const sigterm = seed.cancel.sigtermAgoSec == null ? null : seed.cancel.done && ended ? requested + 1200 : now - seed.cancel.sigtermAgoSec * 1000;
			cancel = {
				requestedAt: new Date(requested).toISOString(),
				requestedBy: seed.cancel.by,
				sigtermAt: sigterm ? new Date(sigterm).toISOString() : null,
				graceSeconds: 60,
				sigkillAt: null,
				exitCode: seed.cancel.done ? 143 : null,
				terminatedAt: seed.cancel.done && ended ? new Date(ended).toISOString() : null
			};
		}
		const script: RunScript = {
			verb: seed.verb ?? 'build',
			select,
			exclude,
			flags,
			nodes,
			fail: seed.fail ?? [],
			warn: seed.warn ?? [],
			errors: seed.errors ?? {},
			critical: job?.criticalNodes ?? [],
			plannedMs,
			end: seed.end ?? 'complete',
			cancel,
			lastHeartbeatAt: seed.lastHeartbeatAgoSec != null ? now - seed.lastHeartbeatAgoSec * 1000 : null,
			imageId: seed.image ?? IMAGE_ID,
			snapshotGeneration: seed.generation ?? SNAPSHOT_GENERATION,
			concurrencyGroups: job?.concurrencyGroups ?? []
		};
		const run: Run = {
			id: seed.id,
			kind: seed.job === null ? 'adhoc' : job?.kind === 'group' ? 'group' : 'job',
			origin: seed.origin,
			// Terminal statuses are derived from node outcomes by the store.
			status: ended ? 'success' : started != null ? 'running' : launched != null ? 'launched' : 'queued',
			statusReason: seed.reason ?? null,
			jobKey: seed.job,
			parentRunId: seed.parent ?? null,
			rootRunId: seed.parent ?? seed.id,
			childRunCount: job?.kind === 'group' ? job.members.length : 0,
			triggerId: trig ? triggerId(trig.name) : null,
			triggerName: trig?.name ?? null,
			tickId: tick != null && trig ? tickId(trig.name, tick) : null,
			tickScheduledFor: tick != null ? new Date(tick).toISOString() : null,
			runKey: seed.runKey ?? null,
			rerunOfRunId: seed.rerunOf ?? null,
			requestedBy: seed.by,
			priority: seed.priority ?? job?.priority ?? 0,
			rewriteHistory: '--full-refresh' in flags || Object.keys(flags).some((f) => f.includes('cursor')) || '--reload' in flags,
			locationId: '',
			target: 'prod',
			snapshotId: 'snap-7d2e41a',
			gitRevision: (seed.generation ?? SNAPSHOT_GENERATION) === SNAPSHOT_GENERATION ? '7d2e41a' : 'c31f0b9',
			queuedAt: new Date(queued).toISOString(),
			launchedAt: launched != null ? new Date(launched).toISOString() : null,
			startedAt: started != null ? new Date(started).toISOString() : null,
			endedAt: ended ? new Date(ended).toISOString() : null,
			nodeCounts: { success: 0, warn: 0, failed: 0, skipped: 0, running: 0, pending: 0 },
			cost: null
		};
		const rec = { run, script };
		records.push(rec);
		return rec;
	}

	/** A group parent plus one child run per member, staged by the group's derived order. */
	function groupRun(seed: Seed, childSeeds: Partial<Record<string, Partial<Seed>>>, stages: string[][]): void {
		const parentQueued = seed.queuedAgo;
		const parentStart = seed.startedAgo ?? parentQueued;
		let offset = 0;
		const children: RunRecord[] = [];
		stages.forEach((stage) => {
			let longest = 0;
			for (const member of stage) {
				const extra = childSeeds[member] ?? {};
				const dur = extra.durationMin ?? 4;
				longest = Math.max(longest, dur);
				children.push(
					record({
						id: `${seed.id}-${member.slice(0, 3)}`,
						origin: seed.origin,
						job: member,
						by: seed.by,
						parent: seed.id,
						trigger: seed.trigger,
						tick: seed.tick,
						queuedAgo: parentStart - 0.1,
						startedAgo: parentStart - offset - 0.2,
						durationMin: dur,
						...extra
					})
				);
			}
			offset += longest + 0.4;
		});
		record({ ...seed, startedAgo: parentStart, durationMin: offset + 0.3 });
	}

	// ---- fixture runs: the ones the overview, checks and queue point at ----

	const at = (name: string, index: number, delaySec = 8) => {
		const t = tickAt(name, index);
		return (now - t) / MIN - delaySec / 60;
	};

	record({ id: 'run-9d21', origin: 'manual', job: 'core_build', by: OPS_USER, queuedAgo: 3.2, startedAgo: 3, durationMin: 9, running: true, warn: ['stg_payments'] });
	record({ id: 'run-6b02', origin: 'sensor', job: 'load_sources', trigger: 'partner_feed_poll', runKey: 'orders_part_0412.csv', by: SENSOR_DAEMON, queuedAgo: 7.5, startedAgo: 7.2, durationMin: 12, running: true, cancel: { requestedAgoSec: 20, by: OPS_USER, sigtermAgoSec: 19, done: false } });
	record({ id: 'run-6a90', origin: 'schedule', job: 'activity_microbatch', trigger: 'activity_microbatch', tick: 2, by: SCHEDULER, queuedAgo: at('activity_microbatch', 2, 0), startedAgo: at('activity_microbatch', 2, 6), durationMin: 60, running: true, lastHeartbeatAgoSec: 95 });
	record({ id: 'run-5f31', origin: 'schedule', job: 'activity_microbatch', trigger: 'activity_microbatch', tick: 1, by: SCHEDULER, queuedAgo: at('activity_microbatch', 1, 0) });
	record({ id: 'run-5e10', origin: 'manual', job: 'revenue_refresh', by: DEMO_USER, queuedAgo: 1 });
	record({ id: 'run-5f77', origin: 'manual', job: 'reporting_exports', by: CI_ACCOUNT, queuedAgo: 2.4, priority: 10 });
	record({ id: 'run-5f80', origin: 'manual', job: 'reporting_exports', by: DEMO_USER, queuedAgo: 0.6, priority: 0 });
	record({ id: 'run-3c88', origin: 'rerun', job: 'core_build', by: OPS_USER, queuedAgo: 1.5, rerunOf: 'run-4f2a', select: ['dim_customers', 'customer_status'], nodes: ['dim_customers', 'customer_status'] });

	record({
		id: 'run-4f2a', origin: 'schedule', job: 'core_build', trigger: 'core_build', tick: 0, by: SCHEDULER,
		queuedAgo: at('core_build', 0, 0), startedAgo: at('core_build', 0, 10), durationMin: 6,
		warn: ['stg_payments'], fail: ['dim_customers'], errors: { dim_customers: 'Numeric value out of range in lifetime_value' }
	});
	record({ id: 'run-2b70', origin: 'schedule', job: 'core_build', trigger: 'core_build', tick: 1, by: SCHEDULER, queuedAgo: at('core_build', 1, 0), startedAgo: at('core_build', 1, 9), durationMin: 7 });
	record({ id: 'run-2b71', origin: 'sensor', job: 'reporting_exports', trigger: 'after_core_build', runKey: 'run-2b70', by: SENSOR_DAEMON, queuedAgo: at('core_build', 1, 9) - 7.2, startedAgo: at('core_build', 1, 9) - 7.4, durationMin: 3 });
	record({ id: 'run-0c7d', origin: 'rerun', job: 'core_build', by: CI_ACCOUNT, rerunOf: 'run-2b70', queuedAgo: 600, startedAgo: 599.8, durationMin: 3, end: 'cancelled', cancel: { requestedAgoSec: 75, by: CI_ACCOUNT, sigtermAgoSec: 74, done: true }, reason: 'Cancelled by svc-ci' });
	record({ id: 'run-91cd', origin: 'manual', job: 'marts_only', by: DEMO_USER, queuedAgo: 97, startedAgo: 96.8, durationMin: 5 });
	record({ id: 'run-3b22', origin: 'manual', job: null, by: DEMO_USER, queuedAgo: 120, startedAgo: 119.8, durationMin: 2, select: ['stg_customers', 'dim_customers'] });
	record({ id: 'run-88e1', origin: 'sensor', job: 'marts_only', trigger: 'on_payments_landed', runKey: 'raw__payments@81204', by: SENSOR_DAEMON, queuedAgo: 206, startedAgo: 205.8, durationMin: 4 });
	record({ id: 'run-1a05', origin: 'schedule', job: 'activity_microbatch', trigger: 'activity_microbatch', tick: 3, by: SCHEDULER, queuedAgo: at('activity_microbatch', 3, 0), startedAgo: at('activity_microbatch', 3, 7), durationMin: 3, warn: ['hourly_order_activity'] });
	record({ id: 'run-c40e', origin: 'manual', job: 'marts_only', by: CI_ACCOUNT, queuedAgo: 1080, startedAgo: 1079.8, durationMin: 4 });
	record({ id: 'run-e3f8', origin: 'schedule', job: 'activity_microbatch', trigger: 'activity_microbatch', tick: 101, by: SCHEDULER, queuedAgo: at('activity_microbatch', 101, 0), startedAgo: at('activity_microbatch', 101, 6), durationMin: 11, end: 'presumed_failed', lastHeartbeatAgoSec: (at('activity_microbatch', 101, 6) - 0.6) * 60, reason: 'Lost contact: no heartbeat for 10 minutes, so the run was stopped and marked failed' });
	record({ id: 'run-a7c1', origin: 'schedule', job: 'revenue_refresh', trigger: 'revenue_refresh', tick: 0, by: SCHEDULER, queuedAgo: at('revenue_refresh', 0, 0), startedAgo: at('revenue_refresh', 0, 8), durationMin: 2, fail: ['daily_revenue'], errors: { daily_revenue: 'Warehouse error: the warehouse was suspended while the statement ran' } });
	record({ id: 'run-f1a9', origin: 'sensor', job: 'load_sources', trigger: 'partner_feed_poll', runKey: 'orders_part_0398.csv', by: SENSOR_DAEMON, queuedAgo: 50 * 60, end: 'never_started', reason: 'Couldn’t start: the run didn’t report in within 2 minutes of launch (the image couldn’t be downloaded)', image: PREVIOUS_IMAGE_ID, generation: 41 });

	const nightlyStages = [['load_sources'], ['core_build'], ['activity_microbatch'], ['reporting_exports']];
	groupRun(
		{ id: 'run-b7a0', origin: 'schedule', job: 'nightly_all', trigger: 'nightly_all', tick: 0, by: SCHEDULER, queuedAgo: at('nightly_all', 0, 0), startedAgo: at('nightly_all', 0, 9) },
		{
			load_sources: { durationMin: 3 },
			core_build: { durationMin: 7, fail: ['daily_revenue'], errors: { daily_revenue: 'Division by zero in revenue_per_order' } },
			activity_microbatch: { durationMin: 5 },
			reporting_exports: { durationMin: 1 }
		},
		nightlyStages
	);
	groupRun(
		{ id: 'run-77b3', origin: 'schedule', job: 'nightly_all', trigger: 'nightly_all', tick: 1, by: SCHEDULER, queuedAgo: at('nightly_all', 1, 0), startedAgo: at('nightly_all', 1, 9) },
		{ load_sources: { durationMin: 3 }, core_build: { durationMin: 8 }, activity_microbatch: { durationMin: 6 }, reporting_exports: { durationMin: 3 } },
		nightlyStages
	);

	// ---- generated schedule history ----

	const fixtureTicks = new Map<string, Set<number>>();
	for (const r of records) {
		if (!r.run.tickScheduledFor || !r.run.triggerName || r.run.parentRunId) continue;
		const set = fixtureTicks.get(r.run.triggerName) ?? new Set();
		set.add(Date.parse(r.run.tickScheduledFor));
		fixtureTicks.set(r.run.triggerName, set);
	}
	const byTick = (name: string, t: number) =>
		records.find((r) => r.run.triggerName === name && !r.run.parentRunId && r.run.tickScheduledFor && Date.parse(r.run.tickScheduledFor) === t);
	const tick = (name: string, t: number, outcome: Tick['outcome'], extra: Partial<Tick> = {}) =>
		ticks.push({
			id: tickId(name, t),
			triggerName: name,
			scheduledFor: new Date(t).toISOString(),
			evaluatedAt: new Date(t + 400).toISOString(),
			outcome,
			runId: null,
			coveredByRunId: null,
			reason: null,
			lateBySeconds: 0,
			catchUp: false,
			...extra
		});

	const pick = (rand: () => number, job: string): Partial<Seed> => {
		const nodes = jobNodes(job);
		const r = rand();
		if (r < 0.06 && nodes.includes('stg_payments')) return { warn: ['stg_payments'] };
		if (r < 0.06 && nodes.length) return { warn: [nodes[0]] };
		if (r < 0.085 && nodes.length > 2) return { fail: [nodes[nodes.length - 2]], errors: { [nodes[nodes.length - 2]]: 'Statement timed out after max_duration' } };
		return {};
	};

	const downtime = { from: ago(30 * 60), to: ago(27 * 60) };
	const usedIds = new Set(records.map((r) => r.run.id));
	const genId = (rand: () => number) => {
		let id: string;
		do id = `run-${Math.floor(rand() * 0xfffff).toString(16).padStart(5, '0')}`;
		while (usedIds.has(id));
		usedIds.add(id);
		return id;
	};

	TRIGGER_DEFS.forEach((t, index) => {
		const spec = t.schedule;
		if (!spec || !t.jobKey || spec.historyDays === 0) return;
		const rand = rng(index * 7919 + 101);
		const job = jobDef(t.jobKey)!;
		const typical = job.kind === 'group' ? 0 : t.jobKey === 'core_build' ? 7 : t.jobKey === 'activity_microbatch' ? 3 : 4;
		const count = Math.floor((spec.historyDays * 1440) / spec.everyMin);
		const skip = fixtureTicks.get(t.name) ?? new Set();
		const missed: number[] = [];
		for (let i = count; i >= 0; i--) {
			const due = tickAt(t.name, i);
			if (t.kind === 'calendar_schedule' && [0, 6].includes(new Date(due).getUTCDay())) continue;
			if (due >= downtime.from && due < downtime.to && t.name === 'activity_microbatch') {
				missed.push(due);
				continue;
			}
			if (missed.length && t.name === 'activity_microbatch') {
				// Catch-up after downtime: `all` with max 3; older missed times are skipped.
				const cap = t.catchUp?.max ?? 1;
				const caught = missed.slice(-cap);
				for (const m of missed.slice(0, -cap)) tick(t.name, m, 'skipped', { evaluatedAt: new Date(downtime.to + 2000).toISOString(), lateBySeconds: Math.round((downtime.to - m) / 1000), catchUp: true, reason: `Beyond the catch-up limit of ${cap} missed times` });
				const runId = genId(rand);
				const startAgo = (now - downtime.to) / MIN - 0.1;
				record({ id: runId, origin: 'schedule', job: t.jobKey, trigger: t.name, by: SCHEDULER, queuedAgo: startAgo + 0.05, startedAgo: startAgo, durationMin: 6, reason: 'Made up after the scheduler was down' }, { queued: downtime.to + 1000, started: downtime.to + 5000 });
				const rec = records[records.length - 1];
				rec.run.tickScheduledFor = new Date(caught[0]).toISOString();
				rec.run.tickId = tickId(t.name, caught[0]);
				tick(t.name, caught[0], 'requested', { runId, evaluatedAt: new Date(downtime.to + 1000).toISOString(), lateBySeconds: Math.round((downtime.to - caught[0]) / 1000), catchUp: true, reason: 'Missed while no server was running the scheduler' });
				for (const c of caught.slice(1)) tick(t.name, c, 'covered', { coveredByRunId: runId, evaluatedAt: new Date(downtime.to + 1000).toISOString(), lateBySeconds: Math.round((downtime.to - c) / 1000), catchUp: true, reason: 'Missed time merged into the make-up run (overlap = coalesce)' });
				missed.length = 0;
			}
			if (skip.has(due)) {
				const fixture = byTick(t.name, due)!;
				tick(t.name, due, 'requested', { runId: fixture.run.id });
				continue;
			}
			if (t.name === 'activity_microbatch' && i === 0) {
				tick(t.name, due, 'covered', { coveredByRunId: 'run-5f31', reason: 'A run is already waiting (overlap = coalesce)' });
				continue;
			}
			if (t.name === 'nightly_all' && i === 4) {
				tick(t.name, due, 'failed', { reason: 'The code location was unavailable, so no run started for this time' });
				continue;
			}
			const id = genId(rand);
			const ageMin = (now - due) / MIN;
			if (job.kind === 'group') {
				const failing = rand() < 0.15;
				const stages = t.jobKey === 'nightly_all' ? nightlyStages : [['revenue_refresh'], ['activity_microbatch', 'customer_segments']];
				const childSeeds: Partial<Record<string, Partial<Seed>>> = Object.fromEntries(stages.flat().map((m) => [m, { durationMin: 2 + rand() * 5 }]));
				if (failing) childSeeds.activity_microbatch = { ...childSeeds.activity_microbatch, fail: ['daily_activity_rollup'], errors: { daily_activity_rollup: 'Statement timed out after max_duration' } };
				groupRun({ id, origin: 'schedule', job: t.jobKey, trigger: t.name, by: SCHEDULER, queuedAgo: ageMin, startedAgo: ageMin - 0.15 }, childSeeds, stages);
			} else {
				record({ id, origin: 'schedule', job: t.jobKey, trigger: t.name, by: SCHEDULER, queuedAgo: ageMin, startedAgo: ageMin - 0.15, durationMin: typical * (0.7 + rand() * 0.6), ...pick(rand, t.jobKey) });
			}
			const rec = records.find((r) => r.run.id === id)!;
			rec.run.tickScheduledFor = new Date(due).toISOString();
			rec.run.tickId = tickId(t.name, due);
			if (job.kind === 'group') for (const child of records.filter((r) => r.run.parentRunId === id)) {
				child.run.tickScheduledFor = rec.run.tickScheduledFor;
				child.run.tickId = rec.run.tickId;
			}
			tick(t.name, due, 'requested', { runId: id });
		}
	});

	// The waiting coalesced run also covers the tick that requested it.
	const waitingTick = ticks.find((t) => t.runId === 'run-5f31');
	if (!waitingTick) tick('activity_microbatch', tickAt('activity_microbatch', 1), 'requested', { runId: 'run-5f31' });

	// ---- sensors ----

	const evaluations: Record<string, SensorEvaluation[]> = {};
	const runRequests: Record<string, SensorRunRequest[]> = {};
	const fingerprint = (name: string, version = 1) => `fp:${name.length.toString(16)}${version}a${(name.charCodeAt(0) * 7919).toString(16)}`;
	const evalOf = (name: string, seq: number, at: number, outcome: SensorEvaluation['outcome'], extra: Partial<SensorEvaluation> = {}): SensorEvaluation => ({
		id: `eval-${name.replace(/_/g, '-')}-${seq}`,
		sequence: seq,
		outcome,
		requestedAt: new Date(at).toISOString(),
		finishedAt: outcome === 'requested' ? null : new Date(at + 1400).toISOString(),
		cursorBefore: null,
		cursorAfter: null,
		runRequests: 0,
		skipReason: null,
		error: null,
		definitionFingerprint: fingerprint(name),
		...extra
	});

	{
		const name = 'on_payments_landed';
		const list: SensorEvaluation[] = [];
		for (let i = 40; i >= 0; i--) {
			list.push(evalOf(name, 900 - i, ago(i * 1.05 + 0.3), 'completed', { cursorBefore: 'freshness_event:81204', cursorAfter: 'freshness_event:81204', skipReason: 'No new data in raw__payments since the cursor' }));
		}
		list.push(evalOf(name, 880, ago(206.1), 'completed', { cursorBefore: 'freshness_event:81160', cursorAfter: 'freshness_event:81204', runRequests: 1 }));
		evaluations[name] = list.sort((a, b) => b.requestedAt.localeCompare(a.requestedAt));
		runRequests[name] = [{ runKey: 'raw__payments@81204', runId: 'run-88e1', evaluationId: `eval-on-payments-landed-880`, requestedAt: isoAgo(206), runStatus: 'success' }];
	}
	{
		const name = 'after_core_build';
		const builds = records
			.filter((r) => r.run.jobKey === 'core_build' && r.run.origin === 'schedule' && !r.run.parentRunId && r.run.endedAt)
			.sort((a, b) => a.run.queuedAt.localeCompare(b.run.queuedAt));
		const list: SensorEvaluation[] = [];
		const reqs: SensorRunRequest[] = [];
		const rand = rng(4242);
		builds.forEach((b, i) => {
			const t = Date.parse(b.run.endedAt!) + 2000;
			const ok = !b.script.fail.length;
			let runId = b.run.id === 'run-2b70' ? 'run-2b71' : null;
			if (ok && !runId) {
				runId = genId(rand);
				const startAgo = (now - t) / MIN - 0.1;
				record({ id: runId, origin: 'sensor', job: 'reporting_exports', trigger: name, runKey: b.run.id, by: SENSOR_DAEMON, queuedAgo: startAgo + 0.05, startedAgo: startAgo, durationMin: 3 });
			}
			const evaluationId = `eval-after-core-build-${300 + i}`;
			list.push(
				evalOf(name, 300 + i, t, 'completed', {
					cursorBefore: `run_event:${4000 + i * 37}`,
					cursorAfter: `run_event:${4037 + i * 37}`,
					runRequests: ok ? 1 : 0,
					skipReason: ok ? null : `${b.run.id} was partial; on-success conditions don't fire on partial runs`
				})
			);
			if (ok && runId) reqs.push({ runKey: b.run.id, runId, evaluationId, requestedAt: new Date(t).toISOString(), runStatus: 'success' });
		});
		evaluations[name] = list.reverse();
		runRequests[name] = reqs.reverse();
	}
	{
		const name = 'on_fact_orders_rebuilt';
		const t = Date.parse(records.find((r) => r.run.id === 'run-4f2a')!.run.endedAt!) + 1500;
		evaluations[name] = [
			evalOf(name, 512, t, 'completed', { cursorBefore: 'run_event:9311', cursorAfter: 'run_event:9338', runRequests: 1 }),
			evalOf(name, 511, t - 97 * MIN, 'completed', { cursorBefore: 'run_event:9290', cursorAfter: 'run_event:9311', runRequests: 1 })
		];
		runRequests[name] = [
			{ runKey: 'run-4f2a', runId: 'run-5f31', evaluationId: 'eval-on-fact-orders-rebuilt-512', requestedAt: new Date(t).toISOString(), runStatus: 'queued' },
			{ runKey: 'run-91cd', runId: 'run-91cd', evaluationId: 'eval-on-fact-orders-rebuilt-511', requestedAt: new Date(t - 97 * MIN).toISOString(), runStatus: 'success' }
		];
	}
	{
		const name = 'platform_health_webhook';
		evaluations[name] = [
			evalOf(name, 77, ago(11.5), 'completed', { cursorBefore: 'platform_event:1201', cursorAfter: 'platform_event:1202', skipReason: null }),
			evalOf(name, 76, ago(11.9), 'failed', { cursorBefore: 'platform_event:1201', cursorAfter: null, error: 'chat_webhook returned HTTP 503; the cursor was not advanced' }),
			evalOf(name, 75, ago(48), 'completed', { cursorBefore: 'platform_event:1200', cursorAfter: 'platform_event:1201' }),
			evalOf(name, 74, ago(300), 'completed', { cursorBefore: 'platform_event:1198', cursorAfter: 'platform_event:1200' })
		];
		runRequests[name] = [];
	}
	{
		const name = 'partner_feed_poll';
		const oldFp = fingerprint(name, 1);
		const stamp = (minutes: number) => new Date(ago(minutes)).toISOString().slice(0, 19) + 'Z';
		evaluations[name] = [
			{ ...evalOf(name, 1290, ago(0.4), 'requested', { cursorBefore: stamp(8) }), definitionFingerprint: oldFp },
			evalOf(name, 1289, ago(5.5), 'completed', { cursorBefore: stamp(70), cursorAfter: stamp(8), runRequests: 1, definitionFingerprint: oldFp }),
			evalOf(name, 1288, ago(10.6), 'timed_out', { cursorBefore: stamp(70), error: 'No result within 60s; the worker was killed and restarted', definitionFingerprint: oldFp }),
			evalOf(name, 1287, ago(15.7), 'completed', { cursorBefore: stamp(70), cursorAfter: stamp(70), skipReason: 'No new files', definitionFingerprint: oldFp }),
			evalOf(name, 1286, ago(20.8), 'abandoned', { cursorBefore: stamp(70), error: 'The location worker restarted during the evaluation', definitionFingerprint: oldFp }),
			evalOf(name, 1285, ago(25.9), 'failed', { cursorBefore: stamp(70), error: 'orders_api: connection reset by api.example.com', definitionFingerprint: oldFp }),
			evalOf(name, 1284, ago(71), 'completed', { cursorBefore: stamp(3000), cursorAfter: stamp(70), runRequests: 1, definitionFingerprint: oldFp })
		];
		runRequests[name] = [
			{ runKey: 'orders_part_0412.csv', runId: 'run-6b02', evaluationId: 'eval-partner-feed-poll-1289', requestedAt: isoAgo(7.6), runStatus: 'running' },
			{ runKey: 'orders_part_0398.csv', runId: 'run-f1a9', evaluationId: null, requestedAt: isoAgo(50 * 60), runStatus: 'failed' }
		];
	}

	const cursors: OperateFixtures['cursors'] = {
		on_payments_landed: { cursor: 'freshness_event:81204', updatedAt: ago(0.3) },
		after_core_build: { cursor: evaluations.after_core_build[0]?.cursorAfter ?? null, updatedAt: ago(19) },
		on_fact_orders_rebuilt: { cursor: 'run_event:9338', updatedAt: ago(18) },
		platform_health_webhook: { cursor: 'platform_event:1202', updatedAt: ago(11.5) },
		partner_feed_poll: { cursor: new Date(ago(8)).toISOString().slice(0, 19) + 'Z', updatedAt: ago(5.5) }
	};

	const stateHistory: Record<string, TriggerStateEvent[]> = {
		revenue_refresh: [
			{ id: 'tse-1', at: isoAgo(26 * 60), kind: 'paused', actor: DEMO_USER, reason: 'Pausing while the revenue source is migrated', value: null }
		],
		core_build: [
			{ id: 'tse-2', at: isoAgo(6 * 1440 + 30), kind: 'reset_to_default', actor: CI_ACCOUNT, reason: 'Deploy finished', value: 'running' },
			{ id: 'tse-3', at: isoAgo(6 * 1440 + 55), kind: 'paused', actor: CI_ACCOUNT, reason: 'Deploy in progress', value: null }
		],
		on_fact_orders_rebuilt: [
			{ id: 'tse-4', at: isoAgo(2 * 1440), kind: 'enabled', actor: OPS_USER, reason: 'Backfill done', value: null },
			{ id: 'tse-5', at: isoAgo(3 * 1440), kind: 'paused', actor: OPS_USER, reason: 'Backfilling activity models by hand', value: null }
		],
		partner_feed_poll: [
			{ id: 'tse-6', at: isoAgo(4 * 1440), kind: 'cursor_reset', actor: OPS_USER, reason: 'Reprocess files after the partner re-sent them', value: null }
		]
	};

	const calendarTimes: number[] = [];
	const lastBiz = lastTickAt.business_days;
	for (let d = 1; d <= 6; d++) {
		const t = lastBiz + d * 1440 * MIN;
		if (![0, 6].includes(new Date(t).getUTCDay())) calendarTimes.push(t);
	}

	return {
		records,
		ticks,
		crons,
		lastTickAt,
		evaluations,
		runRequests,
		stateHistory,
		cursors,
		calendar: { storedUntil: lastBiz + 5 * 1440 * MIN + 12 * 60 * MIN, lastRefreshedAt: ago(25 * 60), times: calendarTimes.filter((t) => t <= lastBiz + 5.5 * 1440 * MIN) }
	};
}

export function resolveJobSelection(job: JobDef) {
	return resolveSelection(selectorTerms(job), job.exclude);
}

export type { JobDef, TriggerDetail };
