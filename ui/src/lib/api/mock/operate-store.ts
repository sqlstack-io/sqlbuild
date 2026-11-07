// In-memory stand-in for the server's OPERATE endpoints: runs, the queue and
// its claims, jobs, triggers and sensors. A small loop advances live runs so
// cancels, launches and claims behave like they do on the server.

import { flagSpec } from '$lib/run/command-table';
import { ApiError } from '../client';
import type { LocalChangeBus } from '../signals';
import type {
	AdhocRunCheck,
	AdhocRunRequest,
	BuiltInRole,
	BulkRunResult,
	Capability,
	JobDetail,
	JobGroupMember,
	JobLaunchOptions,
	OverlapOutcome,
	Principal,
	QueueEntry,
	QueueWait,
	RerunMode,
	Run,
	RunDetail,
	RunEventPage,
	RunListQuery,
	RunNode,
	RunOutputLine,
	RunRow,
	RunRowPage,
	RunRowQuery,
	RunStatement,
	RunTabCounts,
	Scope,
	SnapshotNode,
	StatementSql,
	Tick,
	Timeline,
	TimelineBar,
	TimelineTickMarker,
	Trigger,
	TriggerDetail,
	TriggerState,
	TriggerStateEvent
} from '../types';
import {
	CONCURRENCY_LIMITS,
	IMAGE_ID,
	JOB_DEFS,
	SNAPSHOT_GENERATION,
	TRIGGER_DEFS,
	concurrencyGroupsOf,
	createOperateFixtures,
	jobDef,
	jobNodes,
	resolveJobSelection,
	selectorTerms,
	triggerDef,
	triggerId,
	type OperateFixtures,
	type TickRecord,
	type TriggerDef
} from './operate';
import { SCHEDULER } from './principals';
import { ancestors, resolveSelection, SNAPSHOT_NODES, snapshotNode, sortTopologically, edgesWithin, writeSetOf } from './operate-graph';
import {
	commandOf,
	computeNodes,
	countNodes,
	deriveTerminalStatus,
	execution,
	invocations,
	isActive,
	isTerminal,
	liveness,
	sqlFor,
	statusReason,
	type RunRecord
} from './run-detail';

const MIN = 60_000;

function plainTick(t: TickRecord): Tick {
	const { triggerName, ...tick } = t;
	void triggerName;
	return tick;
}
const HOLDING = new Set(['launched', 'running', 'unresponsive']);
const UNRESPONSIVE_MS = 45_000;
const PRESUMED_FAILED_MS = 600_000;
const CANCEL_EXIT_MS = 30_000;

/** Node scope of the `run.adhoc` grant per mock role; null means every node. */
const ADHOC_SCOPE: Record<BuiltInRole, string[] | null> = {
	viewer: [],
	operator: [],
	developer: ['group:staging', 'group:marts', 'group:activity', 'group:reporting'],
	admin: null
};

type TriggerRuntime = {
	state: TriggerState;
	history: TriggerStateEvent[];
	cursor: string | null;
	cursorUpdatedAt: number | null;
};

export type OperateContext = {
	changes: LocalChangeBus;
	role: () => BuiltInRole;
	can: (capability: Capability) => boolean;
	user: () => Principal;
	adapter: (scope: Scope) => string;
	/** Name of the location's one target. */
	target: (scope: Scope) => string;
	reply: <T>(value: T) => Promise<T>;
};

export class MockOperate {
	readonly #ctx: OperateContext;
	readonly #fx: OperateFixtures;
	readonly #records: RunRecord[];
	readonly #triggers = new Map<string, Map<string, TriggerRuntime>>();
	#seq = 0;

	constructor(ctx: OperateContext, now = Date.now()) {
		this.#ctx = ctx;
		this.#fx = createOperateFixtures(now);
		this.#records = this.#fx.records;
		for (const rec of this.#records) this.#settle(rec, now);
		if (typeof window !== 'undefined') setInterval(() => this.#advance(), 1500);
	}

	// ---- reads: runs ----

	listRuns(scope: Scope, query: RunListQuery = {}): Promise<Run[]> {
		let recs = this.#records;
		if (query.jobKey) recs = recs.filter((r) => r.run.jobKey === query.jobKey);
		if (query.parentRunId) recs = recs.filter((r) => r.run.parentRunId === query.parentRunId);
		else if (query.topLevelOnly) recs = recs.filter((r) => !r.run.parentRunId);
		if (query.triggerId) recs = recs.filter((r) => r.run.triggerId === query.triggerId);
		if (query.since) recs = recs.filter((r) => r.run.queuedAt >= query.since!);
		if (query.until) recs = recs.filter((r) => r.run.queuedAt < query.until!);
		let runs = recs.map((r) => this.#view(r, scope));
		if (query.statuses?.length) runs = runs.filter((r) => query.statuses!.includes(r.status));
		if (query.kinds?.length) runs = runs.filter((r) => query.kinds!.includes(r.kind));
		if (query.origins?.length) runs = runs.filter((r) => query.origins!.includes(r.origin));
		runs.sort((a, b) => b.queuedAt.localeCompare(a.queuedAt));
		if (query.limit) runs = runs.slice(0, query.limit);
		return this.#ctx.reply(runs);
	}

	listRunRows(scope: Scope, query: RunRowQuery = {}): Promise<RunRowPage> {
		const now = Date.now();
		const matching = this.#matching(scope, query);
		const offset = query.cursor ? Number(query.cursor.replace(/^c/, '')) || 0 : 0;
		const limit = query.limit ?? 50;
		const page = matching.slice(offset, offset + limit);
		const waits = this.#queue();
		const rows: RunRow[] = page.map(({ rec, view }) => {
			const nodes = this.#nodesOf(rec, now);
			const done = nodes.filter((n) => n.status !== 'pending' && n.status !== 'running').length;
			const cancel = rec.script.cancel;
			return {
				run: view,
				progress: { done, total: nodes.length },
				waits: view.status === 'queued' ? (waits.get(view.id) ?? []) : [],
				lastContactAt: view.status === 'unresponsive' || view.status === 'presumed_failed' ? (liveness(rec, now)?.lastHeartbeatAt ?? null) : null,
				lostContactLimitSeconds: PRESUMED_FAILED_MS / 1000,
				cancelRequestedAt: cancel && !cancel.terminatedAt ? cancel.requestedAt : null,
				selection: view.kind === 'adhoc' ? rec.script.select : [],
				rerunFromFailureCount: nodes.filter((n) => n.status === 'failed' || n.status === 'skipped').length
			};
		});
		const next = offset + limit < matching.length ? `c${offset + limit}` : null;
		return this.#ctx.reply({ rows, nextCursor: next });
	}

	runTabCounts(scope: Scope, query: RunRowQuery = {}): Promise<RunTabCounts> {
		const base = this.#matching(scope, { ...query, tab: 'all' });
		return this.#ctx.reply({
			active: base.filter(({ view }) => isActive(view.status)).length,
			queued: base.filter(({ view }) => view.status === 'queued').length
		});
	}

	cancelMatchingRuns(scope: Scope, query: RunRowQuery): Promise<BulkRunResult> {
		this.#need('run.cancel');
		const targets = this.#matching(scope, query).filter(({ view }) => isActive(view.status));
		const result: BulkRunResult = { done: [], failed: [] };
		for (const { rec } of targets) {
			try {
				void this.cancelRun(scope, rec.run.id);
				result.done.push(rec.run.id);
			} catch (e) {
				result.failed.push({ runId: rec.run.id, error: e instanceof Error ? e.message : String(e) });
			}
		}
		return this.#ctx.reply(result);
	}

	/** Records matching a runs-list query, newest first. */
	#matching(scope: Scope, query: RunRowQuery): { rec: RunRecord; view: Run }[] {
		const f = query.filter ?? {};
		const lower = (xs?: string[]) => (xs?.length ? xs.map((x) => x.toLowerCase()) : null);
		const ids = lower(f.ids)?.map((id) => id.replace(/^run-/, ''));
		const jobs = lower(f.jobKeys);
		const schedules = lower(f.schedules);
		const sensors = lower(f.sensors);
		const by = lower(f.requestedBy);
		const nodeKeys = lower(f.nodeKeys);
		const text = f.text?.trim().toLowerCase() ?? '';
		const tab = query.tab ?? 'all';
		const out: { rec: RunRecord; view: Run }[] = [];
		for (const rec of this.#records) {
			const r = rec.run;
			if (query.parentRunId) {
				if (r.parentRunId !== query.parentRunId) continue;
			} else if (!query.includeChildren && r.parentRunId) continue;
			if (query.since && r.queuedAt < query.since) continue;
			if (query.until && r.queuedAt >= query.until) continue;
			if (f.kinds?.length && !f.kinds.includes(r.kind)) continue;
			if (f.origins?.length && !f.origins.includes(r.origin)) continue;
			if (ids && !ids.some((id) => r.id.replace(/^run-/, '').toLowerCase().startsWith(id))) continue;
			if (jobs && !jobs.includes((r.jobKey ?? '').toLowerCase())) continue;
			const trigger = triggerDef(r.triggerName);
			const isSchedule = trigger?.kind === 'cron_schedule' || trigger?.kind === 'calendar_schedule';
			if (schedules && !(isSchedule && schedules.includes((r.triggerName ?? '').toLowerCase()))) continue;
			if (sensors && !(trigger && !isSchedule && sensors.includes((r.triggerName ?? '').toLowerCase()))) continue;
			if (by && !by.includes(r.requestedBy.name.toLowerCase())) continue;
			if (nodeKeys) {
				const nodes = new Set([rec, ...this.#children(rec)].flatMap((x) => x.script.nodes.map((n) => n.toLowerCase())));
				if (!nodeKeys.some((n) => nodes.has(n))) continue;
			}
			if (text && !`${r.id} ${r.jobKey ?? ''} ${r.triggerName ?? ''} ${r.requestedBy.name} ${r.requestedBy.displayName} ${r.runKey ?? ''}`.toLowerCase().includes(text)) continue;
			const view = this.#view(rec, scope);
			if (f.statuses?.length && !f.statuses.includes(view.status)) continue;
			if (tab === 'active' && !isActive(view.status)) continue;
			if (tab === 'queued' && view.status !== 'queued') continue;
			if (tab === 'failed' && !['failed', 'partial', 'presumed_failed'].includes(view.status)) continue;
			if (tab === 'cancelled' && view.status !== 'cancelled') continue;
			out.push({ rec, view });
		}
		return out.sort((a, b) => b.view.queuedAt.localeCompare(a.view.queuedAt) || a.view.id.localeCompare(b.view.id));
	}

	getRun(scope: Scope, runId: string): Promise<Run | null> {
		const rec = this.#byId(runId);
		return this.#ctx.reply(rec ? this.#view(rec, scope) : null);
	}

	getRunDetail(scope: Scope, runId: string): Promise<RunDetail | null> {
		const rec = this.#byId(runId);
		if (!rec) return this.#ctx.reply(null);
		const now = Date.now();
		const run = this.#view(rec, scope);
		const children = this.#children(rec).map((c) => this.#view(c, scope));
		const parentRec = rec.run.parentRunId ? this.#byId(rec.run.parentRunId) : undefined;
		const nodes = this.#nodesOf(rec, now);
		const nodeKeys = nodes.map((n) => n.nodeKey);
		const ownRecords = [rec, ...this.#children(rec)];
		const detail: RunDetail = {
			run,
			parent: parentRec ? this.#view(parentRec, scope) : null,
			children,
			nodes,
			edges: edgesWithin(nodeKeys),
			invocations: ownRecords.flatMap((r) => invocations(r)),
			liveness: liveness(rec, now),
			cancel: rec.script.cancel ? structuredClone(rec.script.cancel) : null,
			provenance: {
				snapshotId: rec.run.snapshotId,
				snapshotGeneration: rec.script.snapshotGeneration,
				gitRevision: rec.run.gitRevision ?? '',
				gitDirty: false,
				imageId: rec.script.imageId,
				sqbVersion: rec.script.snapshotGeneration === SNAPSHOT_GENERATION ? '0.185.0' : '0.184.2',
				selection: rec.script.select,
				exclude: rec.script.exclude,
				command: rec.script.nodes.length ? commandOf(rec) : `group ${rec.run.jobKey}: ${this.#children(rec).length} child runs`
			},
			selection: rec.script.select,
			waits: rec.run.status === 'queued' ? this.#queue().get(rec.run.id) ?? [] : [],
			rerunFromFailureNodes: nodes.filter((n) => n.status === 'failed' || n.status === 'skipped').map((n) => n.nodeKey),
			rerunOf: rec.run.rerunOfRunId ? ((r) => (r ? this.#view(r, scope) : null))(this.#byId(rec.run.rerunOfRunId)) : null,
			reruns: this.#records
				.filter((r) => r.run.rerunOfRunId === rec.run.id)
				.map((r) => this.#view(r, scope))
				.sort((a, b) => b.queuedAt.localeCompare(a.queuedAt))
		};
		return this.#ctx.reply(detail);
	}

	listRunEvents(scope: Scope, runId: string, query: { after?: number; limit?: number } = {}): Promise<RunEventPage> {
		const rec = this.#require(runId);
		const now = Date.now();
		const all = [rec, ...this.#children(rec)]
			.flatMap((r) => execution(r, this.#nodesOf(r, now)).events)
			.sort((a, b) => a.occurredAt.localeCompare(b.occurredAt) || a.storageId - b.storageId)
			.map((e, i) => ({ ...e, storageId: i + 1 }));
		const after = query.after ?? 0;
		const limit = query.limit ?? 500;
		const page = all.filter((e) => e.storageId > after).slice(0, limit);
		const more = all.length > after + page.length;
		return this.#ctx.reply({ events: page, nextCursor: page.length ? (more || isActive(rec.run.status) ? page[page.length - 1].storageId : null) : after || null });
	}

	listRunStatements(scope: Scope, runId: string): Promise<RunStatement[]> {
		const rec = this.#require(runId);
		const now = Date.now();
		return this.#ctx.reply([rec, ...this.#children(rec)].flatMap((r) => execution(r, this.#nodesOf(r, now), now).statements));
	}

	getStatementSql(scope: Scope, runId: string, statementId: string): Promise<StatementSql> {
		this.#need('view.sql');
		const rec = this.#require(runId);
		const now = Date.now();
		const statement = [rec, ...this.#children(rec)]
			.flatMap((r) => execution(r, this.#nodesOf(r, now), now).statements)
			.find((st) => st.id === statementId);
		if (!statement) throw new ApiError(`Unknown statement ${statementId}`, 404);
		return this.#ctx.reply({ statementId, sql: statement.sqlRecorded ? sqlFor(statement.nodeKey, statement.kind, this.#ctx.target(scope)) : null });
	}

	listRunOutput(scope: Scope, runId: string): Promise<RunOutputLine[]> {
		const rec = this.#require(runId);
		const now = Date.now();
		return this.#ctx.reply([rec, ...this.#children(rec)].flatMap((r) => execution(r, this.#nodesOf(r, now)).output));
	}

	listQueue(scope: Scope): Promise<QueueEntry[]> {
		const waits = this.#queue();
		const queued = this.#ordered();
		return this.#ctx.reply(
			queued.map((rec, i) => ({
				run: this.#view(rec, scope),
				position: i + 1,
				waits: waits.get(rec.run.id) ?? [],
				overlap: this.#overlap(rec, queued)
			}))
		);
	}

	timeline(scope: Scope, window: { since: string; until: string }): Promise<Timeline> {
		const now = Date.now();
		const bars: TimelineBar[] = [];
		for (const rec of this.#records) {
			const r = rec.run;
			if (r.parentRunId || !r.jobKey || !r.startedAt) continue;
			const end = r.endedAt ?? new Date(now).toISOString();
			if (end <= window.since || r.startedAt >= window.until) continue;
			const view = this.#view(rec, scope);
			bars.push({ runId: r.id, jobKey: r.jobKey, kind: r.kind, origin: r.origin, status: view.status, triggerName: r.triggerName, start: r.startedAt, end });
		}
		// Only schedules and calendars have due times to project; sensors fire on events.
		const until = Date.parse(window.until);
		for (const t of this.#triggerViews(scope)) {
			if (t.type !== 'schedule' || t.state !== 'running' || !t.jobKey) continue;
			const typical = this.#typicalMs(t.jobKey);
			for (const due of this.#upcoming(t.name, until)) {
				if (due < Date.parse(window.since)) continue;
				bars.push({
					runId: null,
					jobKey: t.jobKey,
					kind: jobDef(t.jobKey)?.kind === 'group' ? 'group' : 'job',
					origin: 'schedule',
					status: 'projected',
					triggerName: t.name,
					start: new Date(due).toISOString(),
					end: new Date(due + typical).toISOString()
				});
			}
		}
		const markers: TimelineTickMarker[] = this.#fx.ticks
			.filter((t) => (t.outcome === 'covered' || t.outcome === 'skipped') && t.scheduledFor >= window.since && t.scheduledFor < window.until)
			.map((t) => ({
				tickId: t.id,
				triggerId: triggerId(t.triggerName),
				triggerName: t.triggerName,
				jobKey: triggerDef(t.triggerName)?.jobKey ?? t.triggerName,
				scheduledFor: t.scheduledFor,
				outcome: t.outcome as 'covered' | 'skipped',
				coveredByRunId: t.coveredByRunId,
				reason: t.reason
			}));
		const jobKeys = [...new Set([...JOB_DEFS.map((j) => j.key)])];
		return this.#ctx.reply({ now: new Date(now).toISOString(), jobKeys, bars: bars.sort((a, b) => a.start.localeCompare(b.start)), markers });
	}

	listSnapshotNodes(scope: Scope): Promise<SnapshotNode[]> {
		void scope;
		return this.#ctx.reply(structuredClone(SNAPSHOT_NODES));
	}

	// ---- actions: runs ----

	cancelRun(scope: Scope, runId: string): Promise<Run> {
		this.#need('run.cancel');
		const rec = this.#require(runId);
		if (isTerminal(rec.run.status)) throw new ApiError(`${runId} has already finished`, 409);
		if (rec.script.cancel) return this.#ctx.reply(this.#view(rec, scope));
		const now = Date.now();
		for (const r of [rec, ...this.#children(rec)]) {
			if (isTerminal(r.run.status)) continue;
			r.script.cancel = {
				requestedAt: new Date(now).toISOString(),
				requestedBy: this.#ctx.user(),
				sigtermAt: null,
				graceSeconds: 60,
				sigkillAt: null,
				exitCode: null,
				terminatedAt: null
			};
			if (r.script.nodes.length && !r.run.startedAt) {
				// Not started yet: nothing to signal, so the run ends straight away.
				r.script.cancel.terminatedAt = r.script.cancel.requestedAt;
				this.#finish(r, now, 'cancelled', `Cancelled by ${this.#ctx.user().displayName} before it started`);
			}
		}
		this.#emit(rec);
		return this.#ctx.reply(this.#view(rec, scope));
	}

	rerunRun(scope: Scope, runId: string, mode: RerunMode, selectedNodes: string[] = []): Promise<Run> {
		this.#need('run.rerun');
		const source = this.#require(runId);
		if (isActive(source.run.status)) throw new ApiError(`${runId} is still active`, 409);
		if (source.run.rewriteHistory) this.#need('run.rewrite_history');
		if (mode === 'selected' && !selectedNodes.length) throw new ApiError('Choose at least one node to rerun', 422);
		const now = Date.now();
		const affected = (rec: RunRecord) =>
			this.#nodesOf(rec, now)
				.filter((n) => (mode === 'selected' ? selectedNodes.includes(n.nodeKey) : n.status === 'failed' || n.status === 'skipped'))
				.map((n) => n.nodeKey);
		const user = this.#ctx.user();
		const id = this.#newId();
		const job = jobDef(source.run.jobKey);
		if (job?.kind === 'group') {
			const members = this.#children(source).filter((c) => mode === 'all' || affected(c).length);
			if (!members.length) throw new ApiError('Nothing to rerun: no node failed or was skipped', 409);
			const parent = this.#create({ id, origin: 'rerun', jobKey: job.key, rerunOf: runId, by: user, select: [], exclude: [], flags: {}, nodes: [], priority: job.priority });
			members.forEach((c, i) => {
				const nodes = mode === 'all' ? c.script.nodes : affected(c);
				this.#create({ id: `${id}-${i + 1}`, origin: 'rerun', jobKey: c.run.jobKey, parent: id, rerunOf: c.run.id, by: user, select: mode === 'all' ? c.script.select : nodes, exclude: mode === 'all' ? c.script.exclude : [], flags: c.script.flags, nodes, priority: job.priority });
			});
			parent.run.childRunCount = members.length;
			this.#ctx.changes.emit('runs');
			return this.#ctx.reply(this.#view(parent, scope));
		}
		const nodes = mode === 'all' ? source.script.nodes : affected(source);
		if (!nodes.length) throw new ApiError('Nothing to rerun: no node failed or was skipped', 409);
		const rec = this.#create({
			id,
			origin: 'rerun',
			jobKey: source.run.jobKey,
			rerunOf: runId,
			by: user,
			select: mode === 'all' ? source.script.select : nodes,
			exclude: mode === 'all' ? source.script.exclude : [],
			flags: source.script.flags,
			nodes,
			priority: source.run.priority
		});
		this.#ctx.changes.emit('runs');
		return this.#ctx.reply(this.#view(rec, scope));
	}

	launchJob(scope: Scope, key: string, options: JobLaunchOptions & { fullRefresh?: boolean } = {}): Promise<Run> {
		this.#need('job.run');
		const job = jobDef(key);
		if (!job) throw new ApiError(`Unknown job ${key}`, 404);
		if (job.targets && !job.targets.includes(this.#ctx.target(scope))) throw new ApiError(`${key} is limited to targets ${job.targets.join(', ')}`, 422);
		const flags: Record<string, string | true> = Object.fromEntries(Object.entries(job.settings).map(([k, v]) => [k, v === true ? true : String(v)]));
		if (options.fullRefresh) flags['--full-refresh'] = true;
		if (options.window) {
			const suffix = options.window.type === 'integer' ? '-int' : '-ts';
			if (options.window.start) flags[`--start-cursor${suffix}`] = options.window.start;
			if (options.window.end) flags[`--end-cursor${suffix}`] = options.window.end;
		}
		const rewrite = !!options.fullRefresh || !!options.window;
		if (rewrite) this.#need('run.rewrite_history');
		const user = this.#ctx.user();
		const id = this.#newId();
		if (job.kind === 'group') {
			const parent = this.#create({ id, origin: 'manual', jobKey: key, by: user, select: [], exclude: [], flags, nodes: [], priority: job.priority });
			job.members.forEach((m, i) => {
				const member = jobDef(m)!;
				this.#create({ id: `${id}-${i + 1}`, origin: 'manual', jobKey: m, parent: id, by: user, select: selectorTerms(member), exclude: member.exclude, flags: { ...flags, ...Object.fromEntries(Object.entries(member.settings).map(([k, v]) => [k, v === true ? true : String(v)])) }, nodes: jobNodes(m), priority: job.priority });
			});
			parent.run.childRunCount = job.members.length;
			this.#ctx.changes.emit('runs');
			return this.#ctx.reply(this.#view(parent, scope));
		}
		const rec = this.#create({ id, origin: 'manual', jobKey: key, by: user, select: selectorTerms(job), exclude: job.exclude, flags, nodes: jobNodes(key), priority: job.priority });
		this.#ctx.changes.emit('runs');
		return this.#ctx.reply(this.#view(rec, scope));
	}

	checkAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<AdhocRunCheck> {
		return this.#ctx.reply(this.#check(scope, request));
	}

	launchAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<Run> {
		const check = this.#check(scope, request);
		if (!check.adhoc.allowed) throw new ApiError(`Refused: run.adhoc doesn't cover ${check.adhoc.outOfScope.join(', ') || 'this run'}`, 403);
		if (check.rewriteHistory.required && !check.rewriteHistory.allowed) throw new ApiError('Refused: requires run.rewrite_history', 403);
		if (!check.ok) throw new ApiError(check.problems[0]?.message ?? 'Invalid request', 422);
		const rec = this.#create({
			id: this.#newId(),
			origin: 'manual',
			jobKey: null,
			by: this.#ctx.user(),
			select: request.select,
			exclude: request.exclude,
			flags: request.flags,
			nodes: check.writeSet.length ? check.writeSet : check.resolvedNodes.filter((n) => snapshotNode(n)?.kind !== 'source'),
			priority: 0,
			verb: request.verb
		});
		this.#ctx.changes.emit('runs');
		return this.#ctx.reply(this.#view(rec, scope));
	}

	// ---- jobs ----

	listJobs(scope: Scope): Promise<JobDetail[]> {
		return this.#ctx.reply(JOB_DEFS.map((j) => this.#jobView(j.key, scope)));
	}

	getJob(scope: Scope, key: string): Promise<JobDetail | null> {
		return this.#ctx.reply(jobDef(key) ? this.#jobView(key, scope) : null);
	}

	// ---- triggers ----

	listTriggers(scope: Scope): Promise<Trigger[]> {
		return this.#ctx.reply(this.#triggerViews(scope));
	}

	getTrigger(scope: Scope, id: string): Promise<TriggerDetail | null> {
		const def = TRIGGER_DEFS.find((t) => triggerId(t.name) === id);
		return this.#ctx.reply(def ? this.#triggerDetail(def, scope) : null);
	}

	setTriggerStates(scope: Scope, ids: string[], state: TriggerState, reason: string | null = null): Promise<Trigger[]> {
		this.#need('trigger.toggle');
		const defs = ids.map((id) => {
			const def = TRIGGER_DEFS.find((t) => triggerId(t.name) === id);
			if (!def) throw new ApiError(`Unknown trigger ${id}`, 404);
			return def;
		});
		for (const def of defs) {
			const rt = this.#runtime(scope, def.name);
			if (rt.state === state) continue;
			rt.state = state;
			rt.history.unshift(this.#stateEvent(state === 'running' ? 'enabled' : 'paused', reason));
		}
		this.#ctx.changes.emit('triggers');
		return this.#ctx.reply(this.#triggerViews(scope).filter((t) => ids.includes(t.id)));
	}

	resetTriggerToDefault(scope: Scope, id: string): Promise<Trigger> {
		this.#need('trigger.toggle');
		const def = TRIGGER_DEFS.find((t) => triggerId(t.name) === id);
		if (!def) throw new ApiError(`Unknown trigger ${id}`, 404);
		const rt = this.#runtime(scope, def.name);
		rt.state = def.defaultState;
		rt.history.unshift(this.#stateEvent('reset_to_default', null, def.defaultState));
		this.#ctx.changes.emit('triggers');
		return this.#ctx.reply(this.#triggerViews(scope).find((t) => t.id === id)!);
	}

	resetSensorCursor(scope: Scope, id: string, reason: string | null = null): Promise<TriggerDetail> {
		this.#need('sensor.reset_cursor');
		const def = TRIGGER_DEFS.find((t) => triggerId(t.name) === id);
		if (!def?.sensor) throw new ApiError(`${id} is not a sensor`, 422);
		const rt = this.#runtime(scope, def.name);
		rt.history.unshift(this.#stateEvent('cursor_reset', reason, rt.cursor));
		rt.cursor = null;
		rt.cursorUpdatedAt = Date.now();
		this.#ctx.changes.emit('triggers');
		return this.#ctx.reply(this.#triggerDetail(def, scope));
	}

	// ---- shell counts ----

	activeRunCount(): number {
		return this.#records.filter((r) => !r.run.parentRunId && isActive(r.run.status)).length;
	}

	pausedTriggerCount(scope: Scope): number {
		return this.#triggerViews(scope).filter((t) => t.state === 'paused').length;
	}

	topLevelRuns(scope: Scope): Run[] {
		return this.#records
			.filter((r) => !r.run.parentRunId)
			.map((r) => this.#view(r, scope))
			.sort((a, b) => b.queuedAt.localeCompare(a.queuedAt));
	}

	// ---- synchronous reads for the BUILD mock (mock/build.ts) ----

	/** Every run that executed `nodeKey`, with that node's result, newest first. */
	nodeHistory(scope: Scope, nodeKey: string): { run: Run; node: RunNode }[] {
		const now = Date.now();
		return this.#records
			.filter((r) => r.script.nodes.includes(nodeKey))
			.flatMap((r) => {
				const node = computeNodes(r, this.#siblings(r), now).find((n) => n.nodeKey === nodeKey);
				return node ? [{ run: this.#view(r, scope), node }] : [];
			})
			.sort((a, b) => (b.node.startedAt ?? b.run.queuedAt).localeCompare(a.node.startedAt ?? a.run.queuedAt));
	}

	/** Jobs and groups whose selection includes `nodeKey`. */
	jobsCovering(nodeKey: string): { key: string; kind: 'job' | 'group'; triggerIds: string[] }[] {
		return JOB_DEFS.filter((j) => (j.kind === 'group' ? jobNodes(j.key) : resolveJobSelection(j).nodes).includes(nodeKey)).map((j) => ({
			key: j.key,
			kind: j.kind,
			triggerIds: TRIGGER_DEFS.filter((t) => t.jobKey === j.key).map((t) => triggerId(t.name))
		}));
	}

	/** Liveness of an active run, for the overview's unresponsive item. */
	lastHeartbeat(runId: string): string | null {
		const rec = this.#byId(runId);
		return rec ? (liveness(rec, Date.now())?.lastHeartbeatAt ?? null) : null;
	}

	// ---- internals: records ----

	#byId(id: string): RunRecord | undefined {
		return this.#records.find((r) => r.run.id === id);
	}

	#require(id: string): RunRecord {
		const rec = this.#byId(id);
		if (!rec) throw new ApiError(`Unknown run ${id}`, 404);
		return rec;
	}

	#children(rec: RunRecord): RunRecord[] {
		return this.#records.filter((r) => r.run.parentRunId === rec.run.id);
	}

	#siblings(rec: RunRecord): RunRecord[] {
		return this.#records.filter((r) => r.run.rootRunId === rec.run.rootRunId);
	}

	#nodesOf(rec: RunRecord, now: number): RunNode[] {
		if (rec.script.nodes.length) return computeNodes(rec, this.#siblings(rec), now);
		return this.#children(rec).flatMap((c) => this.#nodesOf(c, now));
	}

	#view(rec: RunRecord, scope: Scope): Run {
		const nodes = this.#nodesOf(rec, Date.now());
		const run = { ...rec.run, locationId: scope.locationId, target: this.#ctx.target(scope) };
		run.nodeCounts = countNodes(nodes);
		run.cost = null;
		if (run.endedAt && run.startedAt && this.#ctx.adapter(scope) === 'snowflake') {
			const seconds = nodes.reduce((sum, n) => sum + (n.startedAt && n.endedAt ? (Date.parse(n.endedAt) - Date.parse(n.startedAt)) / 1000 : 0), 0);
			if (seconds > 0) run.cost = { value: Math.round(seconds * 9) / 10_000, unit: 'credits', source: 'Busy-compute estimate from statement history' };
		}
		return run;
	}

	/** Brings a record's projected status in line with its facts at `now`. */
	#settle(rec: RunRecord, now: number) {
		const { run, script } = rec;
		if (!script.nodes.length) {
			const kids = this.#children(rec);
			if (!kids.length) return;
			if (kids.every((k) => isTerminal(k.run.status))) {
				if (isTerminal(run.status) && run.endedAt && run.status !== 'success') return;
				const nodes = this.#nodesOf(rec, now);
				const status = script.cancel ? 'cancelled' : deriveTerminalStatus('complete', nodes);
				run.status = status;
				run.endedAt = kids.reduce((m, k) => (k.run.endedAt && k.run.endedAt > m ? k.run.endedAt : m), run.startedAt ?? run.queuedAt);
				run.statusReason = run.statusReason ?? statusReason(status, nodes);
			} else if (kids.some((k) => k.run.startedAt) && run.status !== 'running') {
				run.status = 'running';
				run.startedAt ??= kids.find((k) => k.run.startedAt)!.run.startedAt;
			}
			return;
		}
		if (run.endedAt && run.status === 'success') {
			const nodes = this.#nodesOf(rec, now);
			const status = deriveTerminalStatus(script.end, nodes);
			run.status = status;
			run.statusReason = run.statusReason ?? statusReason(status, nodes);
			return;
		}
		if (run.status === 'running' && script.lastHeartbeatAt != null && now - script.lastHeartbeatAt > UNRESPONSIVE_MS) {
			run.status = 'unresponsive';
			run.statusReason = `No signal for ${Math.round((now - script.lastHeartbeatAt) / 1000)}s`;
		}
	}

	#finish(rec: RunRecord, at: number, status: Run['status'], reason: string | null) {
		rec.run.endedAt = new Date(at).toISOString();
		rec.script.end = status === 'cancelled' ? 'cancelled' : status === 'presumed_failed' ? 'presumed_failed' : 'complete';
		const nodes = this.#nodesOf(rec, at);
		rec.run.status = status === 'success' ? deriveTerminalStatus(rec.script.end, nodes) : status;
		rec.run.statusReason = reason ?? statusReason(rec.run.status, nodes);
		if (rec.run.parentRunId) {
			const parent = this.#byId(rec.run.parentRunId);
			if (parent) this.#settle(parent, at);
		}
	}

	/** One scheduler pass: liveness, cancels, launches and claims. */
	#advance() {
		const now = Date.now();
		const changed = new Set<RunRecord>();
		for (const rec of this.#records) {
			const { run, script } = rec;
			if (isTerminal(run.status) || !script.nodes.length) continue;
			const cancel = script.cancel;
			if (cancel && !cancel.terminatedAt) {
				if (!cancel.sigtermAt && now - Date.parse(cancel.requestedAt) > 1000) {
					cancel.sigtermAt = new Date(now).toISOString();
					changed.add(rec);
				} else if (cancel.sigtermAt && now - Date.parse(cancel.sigtermAt) > CANCEL_EXIT_MS) {
					cancel.exitCode = 143;
					cancel.terminatedAt = new Date(now).toISOString();
					this.#finish(rec, now, 'cancelled', `Cancelled by ${cancel.requestedBy.displayName}`);
					changed.add(rec);
				}
				continue;
			}
			if (run.status === 'launched' && run.launchedAt && now - Date.parse(run.launchedAt) > 2000) {
				run.status = 'running';
				run.startedAt = new Date(now).toISOString();
				changed.add(rec);
			} else if (run.status === 'running') {
				if (script.lastHeartbeatAt != null) {
					this.#settle(rec, now);
					if (rec.run.status !== 'running') changed.add(rec);
				} else if (run.startedAt && now >= Date.parse(run.startedAt) + script.plannedMs) {
					this.#finish(rec, Date.parse(run.startedAt) + script.plannedMs, 'success', null);
					changed.add(rec);
				}
			} else if (run.status === 'unresponsive' && script.lastHeartbeatAt != null && now - script.lastHeartbeatAt > PRESUMED_FAILED_MS) {
				this.#finish(rec, script.lastHeartbeatAt + PRESUMED_FAILED_MS, 'presumed_failed', 'Lost contact: no heartbeat for 10 minutes, so the run was stopped and marked failed');
				changed.add(rec);
			}
		}
		const waits = this.#queue();
		for (const rec of this.#ordered()) {
			if ((waits.get(rec.run.id) ?? []).some((w) => w.kind !== 'priority')) continue;
			rec.run.status = 'launched';
			rec.run.launchedAt = new Date(now).toISOString();
			if (!rec.script.nodes.length) {
				rec.run.status = 'running';
				rec.run.startedAt = rec.run.launchedAt;
			}
			changed.add(rec);
			break; // claims change the picture; re-evaluate on the next pass
		}
		for (const rec of this.#records) {
			if (!rec.script.nodes.length && isActive(rec.run.status) && rec.run.status !== 'queued') {
				const before = rec.run.status;
				this.#settle(rec, now);
				if (rec.run.status !== before) changed.add(rec);
			}
		}
		for (const rec of changed) this.#emit(rec);
		// Live runs append events continuously; the server signals "run X advanced".
		for (const rec of this.#records) {
			if (rec.run.status === 'running' && !changed.has(rec)) this.#ctx.changes.emit(`run:${rec.run.id}`);
		}
	}

	#emit(rec: RunRecord) {
		this.#ctx.changes.emit('runs');
		this.#ctx.changes.emit(`run:${rec.run.id}`);
		if (rec.run.parentRunId) this.#ctx.changes.emit(`run:${rec.run.parentRunId}`);
	}

	#ordered(): RunRecord[] {
		return this.#records
			.filter((r) => r.run.status === 'queued')
			.sort((a, b) => b.run.priority - a.run.priority || a.run.queuedAt.localeCompare(b.run.queuedAt));
	}

	/** Why each queued run waits, evaluated like the claim transaction would. */
	#queue(): Map<string, QueueWait[]> {
		const holders = this.#records.filter((r) => HOLDING.has(r.run.status));
		const out = new Map<string, QueueWait[]>();
		const ahead: RunRecord[] = [];
		for (const rec of this.#ordered()) {
			const waits: QueueWait[] = [];
			const mine = new Set(rec.script.nodes);
			for (const h of holders) {
				const overlap = h.script.nodes.filter((n) => mine.has(n));
				if (overlap.length)
					waits.push({ kind: 'write_claim', runId: h.run.id, jobKey: h.run.jobKey, nodeKey: overlap[0], otherNodes: overlap.length - 1 });
			}
			const fullGroups: string[] = [];
			for (const group of rec.script.concurrencyGroups) {
				const inGroup = holders.filter((h) => h.script.concurrencyGroups.includes(group)).map((h) => h.run.id);
				const limit = CONCURRENCY_LIMITS[group] ?? 1;
				if (inGroup.length >= limit) {
					waits.push({ kind: 'concurrency_group', group, limit, holders: inGroup });
					fullGroups.push(group);
				}
			}
			if (fullGroups.length) {
				const before = ahead.filter((a) => a.script.concurrencyGroups.some((g) => fullGroups.includes(g))).map((a) => a.run.id);
				if (before.length) waits.push({ kind: 'priority', aheadRunIds: before });
			}
			if (rec.run.parentRunId) {
				const parent = this.#byId(rec.run.parentRunId);
				const group = jobDef(parent?.run.jobKey ?? null);
				if (parent && group) {
					const members = this.#groupMembers(group.key);
					const me = members.find((m) => m.jobKey === rec.run.jobKey);
					for (const up of me?.upstream ?? []) {
						const sibling = this.#children(parent).find((c) => c.run.jobKey === up.jobKey);
						if (sibling && isActive(sibling.run.status)) waits.push({ kind: 'upstream_job', runId: sibling.run.id, jobKey: up.jobKey });
					}
				}
				if (parent && parent.run.status === 'queued') waits.push({ kind: 'group_parent', runId: parent.run.id, jobKey: parent.run.jobKey ?? '' });
			}
			out.set(rec.run.id, waits);
			ahead.push(rec);
		}
		return out;
	}

	#overlap(rec: RunRecord, queued: RunRecord[]): OverlapOutcome | null {
		const job = jobDef(rec.run.jobKey);
		if (!job || rec.run.parentRunId) return null;
		const covered = this.#fx.ticks.filter((t) => t.coveredByRunId === rec.run.id).length;
		if (job.overlap === 'coalesce')
			return {
				policy: 'coalesce',
				coveredTicks: covered,
				detail: covered ? `${covered} later request${covered === 1 ? '' : 's'} merged into this run` : 'New requests merge into this run while it waits'
			};
		if (job.overlap === 'queue') {
			const before = queued.filter((q) => q.run.jobKey === job.key && q.run.queuedAt < rec.run.queuedAt);
			return { policy: 'queue', coveredTicks: 0, detail: before.length ? `Goes after ${before.map((b) => b.run.id.replace(/^run-/, '')).join(', ')}` : 'Next in line for this job' };
		}
		return { policy: 'allow', coveredTicks: 0, detail: 'Waits only for runs updating the same data' };
	}

	#newId(): string {
		let id: string;
		do id = `run-${(0xa1000 + ++this.#seq * 0x9e37).toString(16).slice(-5)}`;
		while (this.#byId(id));
		return id;
	}

	#create(spec: {
		id: string;
		origin: Run['origin'];
		jobKey: string | null;
		by: Principal;
		select: string[];
		exclude: string[];
		flags: Record<string, string | true>;
		nodes: string[];
		priority: number;
		parent?: string;
		rerunOf?: string;
		verb?: AdhocRunRequest['verb'];
	}): RunRecord {
		const now = new Date().toISOString();
		const job = jobDef(spec.jobKey);
		const rec: RunRecord = {
			run: {
				id: spec.id,
				kind: spec.jobKey === null ? 'adhoc' : job?.kind === 'group' ? 'group' : 'job',
				origin: spec.origin,
				status: 'queued',
				statusReason: null,
				jobKey: spec.jobKey,
				parentRunId: spec.parent ?? null,
				rootRunId: spec.parent ?? spec.id,
				childRunCount: 0,
				triggerId: null,
				triggerName: null,
				tickId: null,
				tickScheduledFor: null,
				runKey: null,
				rerunOfRunId: spec.rerunOf ?? null,
				requestedBy: spec.by,
				priority: spec.priority,
				rewriteHistory: Object.keys(spec.flags).some((f) => flagSpec(spec.verb ?? 'build', f)?.rewritesHistory),
				locationId: '',
				target: '',
				snapshotId: 'snap-7d2e41a',
				gitRevision: '7d2e41a',
				queuedAt: now,
				launchedAt: null,
				startedAt: null,
				endedAt: null,
				nodeCounts: { success: 0, warn: 0, failed: 0, skipped: 0, running: 0, pending: 0 },
				cost: null
			},
			script: {
				verb: spec.verb ?? 'build',
				select: spec.select,
				exclude: spec.exclude,
				flags: spec.flags,
				nodes: sortTopologically(spec.nodes),
				fail: [],
				warn: [],
				errors: {},
				critical: job?.criticalNodes ?? [],
				plannedMs: Math.max(12_000, spec.nodes.length * 2500),
				end: 'complete',
				cancel: null,
				lastHeartbeatAt: null,
				imageId: IMAGE_ID,
				snapshotGeneration: SNAPSHOT_GENERATION,
				concurrencyGroups: job?.concurrencyGroups ?? []
			}
		};
		this.#records.unshift(rec);
		return rec;
	}

	#need(capability: Capability) {
		if (!this.#ctx.can(capability)) throw new ApiError(`Missing permission: ${capability}`, 403);
	}

	#check(scope: Scope, request: AdhocRunRequest): AdhocRunCheck {
		const problems: AdhocRunCheck['problems'] = [];
		const resolution = resolveSelection(request.select, request.exclude);
		for (const term of resolution.unknown) problems.push({ code: 'unknown_selector', message: `No node or selector matches ${term}` });
		if (!resolution.nodes.length) problems.push({ code: 'empty_selection', message: 'The selection resolves to no nodes' });
		const writable = resolution.nodes.filter((n) => snapshotNode(n)?.kind !== 'source');
		const writeSet =
			request.verb === 'build'
				? writable.filter((n) => !(request.flags['--no-load'] && snapshotNode(n)?.kind === 'loader'))
				: request.verb === 'seed'
					? writable.filter((n) => snapshotNode(n)?.kind === 'seed')
					: request.verb === 'load'
						? writable.filter((n) => snapshotNode(n)?.kind === 'loader')
						: [];
		const reasons: string[] = [];
		for (const [flag, value] of Object.entries(request.flags)) {
			const spec = flagSpec(request.verb, flag);
			if (!spec) {
				problems.push({ code: 'unknown_flag', message: `sqb ${request.verb} has no flag ${flag}` });
				continue;
			}
			if (spec.type !== 'bool' && value === true) problems.push({ code: 'missing_value', message: `${flag} needs a value` });
			if (spec.type === 'int' && value !== true && !/^\d+$/.test(value)) problems.push({ code: 'invalid_value', message: `${flag} must be a whole number` });
			if (spec.type === 'timestamp' && value !== true && Number.isNaN(Date.parse(value))) problems.push({ code: 'invalid_value', message: `${flag} must be a timestamp, e.g. 2026-10-01T00:00:00Z` });
			if (spec.rewritesHistory) reasons.push(flag);
			if (flag.includes('cursor') && !writeSet.some((n) => snapshotNode(n)?.cursor)) problems.push({ code: 'no_cursor', message: `${flag}: no selected node has a cursor` });
		}
		const scopeTerms = ADHOC_SCOPE[this.#ctx.role()];
		const granted = scopeTerms === null ? null : new Set(writeSetOf(scopeTerms.length ? scopeTerms : ['__none__'], []));
		const outOfScope = !this.#ctx.can('run.adhoc') ? writeSet : granted === null ? [] : writeSet.filter((n) => !granted.has(n));
		const allowed = this.#ctx.can('run.adhoc') && outOfScope.length === 0;
		const holders = this.#records.filter((r) => HOLDING.has(r.run.status));
		const claimConflicts = writeSet.flatMap((n) => holders.filter((h) => h.script.nodes.includes(n)).map((h) => ({ runId: h.run.id, nodeKey: n })));
		const rewriteAllowed = this.#ctx.can('run.rewrite_history');
		void scope;
		return {
			resolvedNodes: resolution.nodes,
			writeSet,
			adhoc: { required: 'run.adhoc', allowed, outOfScope },
			rewriteHistory: { required: reasons.length > 0, reasons, allowed: rewriteAllowed },
			claimConflicts,
			problems,
			ok: problems.length === 0 && allowed && (!reasons.length || rewriteAllowed)
		};
	}

	// ---- internals: jobs ----

	#groupMembers(key: string): JobGroupMember[] {
		const job = jobDef(key);
		if (!job || job.kind !== 'group') return [];
		const nodesOf = new Map(job.members.map((m) => [m, new Set(jobNodes(m))]));
		const upstream = new Map<string, JobGroupMember['upstream']>();
		for (const b of job.members) {
			const ups: JobGroupMember['upstream'] = [];
			const needs = new Set([...nodesOf.get(b)!].flatMap((n) => [...ancestors(n)]));
			for (const a of job.members) {
				if (a === b) continue;
				if ([...nodesOf.get(a)!].some((n) => needs.has(n))) ups.push({ jobKey: a, source: 'lineage' });
			}
			for (const a of job.after[b] ?? []) if (!ups.some((u) => u.jobKey === a)) ups.push({ jobKey: a, source: 'after' });
			upstream.set(b, ups);
		}
		const stage = new Map<string, number>();
		const stageOf = (m: string): number => {
			if (stage.has(m)) return stage.get(m)!;
			const s = Math.max(-1, ...(upstream.get(m) ?? []).map((u) => stageOf(u.jobKey))) + 1;
			stage.set(m, s);
			return s;
		};
		return job.members.map((m) => ({ jobKey: m, upstream: upstream.get(m) ?? [], stage: stageOf(m) }));
	}

	#jobView(key: string, scope: Scope): JobDetail {
		const job = jobDef(key)!;
		const latest = this.#records
			.filter((r) => r.run.jobKey === key && !r.run.parentRunId)
			.sort((a, b) => b.run.queuedAt.localeCompare(a.run.queuedAt))[0];
		const view = latest ? this.#view(latest, scope) : null;
		const holders = this.#records.filter((r) => HOLDING.has(r.run.status));
		const resolved = job.kind === 'group' ? jobNodes(key) : resolveJobSelection(job).nodes;
		const { after, ...rest } = job;
		void after;
		return structuredClone({
			...rest,
			nodeCount: resolved.filter((n) => snapshotNode(n)?.kind !== 'source').length,
			resolvedNodes: resolved,
			lastRun: view ? { id: view.id, status: view.status, startedAt: view.startedAt, endedAt: view.endedAt } : null,
			triggerIds: TRIGGER_DEFS.filter((t) => t.jobKey === key).map((t) => triggerId(t.name)),
			concurrency: concurrencyGroupsOf(job).map((g) => ({ ...g, holders: holders.filter((h) => h.script.concurrencyGroups.includes(g.key)).map((h) => h.run.id) })),
			memberOf: JOB_DEFS.filter((g) => g.members.includes(key)).map((g) => g.key),
			cursorType: resolved.map((n) => snapshotNode(n)?.cursor).find((c) => c) ?? null,
			groupMembers: this.#groupMembers(key),
			declaration: { ...job.declaration, code: this.#fillCrons(job.declaration.code) }
		});
	}

	#typicalMs(jobKey: string): number {
		const done = this.#records.filter((r) => r.run.jobKey === jobKey && !r.run.parentRunId && r.run.startedAt && r.run.endedAt).slice(0, 10);
		if (!done.length) return 5 * MIN;
		return done.reduce((s, r) => s + Date.parse(r.run.endedAt!) - Date.parse(r.run.startedAt!), 0) / done.length;
	}

	#fillCrons(code: string): string {
		return code.replace(/\{cron:([a-z_]+)\}/g, (_, name: string) => this.#fx.crons[name] ?? '* * * * *');
	}

	// ---- internals: triggers ----

	#runtime(scope: Scope, name: string): TriggerRuntime {
		const key = scope.locationId;
		const dev = this.#ctx.target(scope) === 'dev';
		let map = this.#triggers.get(key);
		if (!map) {
			map = new Map();
			for (const t of TRIGGER_DEFS) {
				const cursor = this.#fx.cursors[t.name];
				// `sqb dev` starts with every trigger paused.
				map.set(t.name, {
					state: dev ? 'paused' : t.state,
					history: structuredClone(dev ? [] : (this.#fx.stateHistory[t.name] ?? [])),
					cursor: cursor?.cursor ?? null,
					cursorUpdatedAt: cursor?.updatedAt ?? null
				});
			}
			this.#triggers.set(key, map);
		}
		return map.get(name)!;
	}

	#stateEvent(kind: TriggerStateEvent['kind'], reason: string | null, value: string | null = null): TriggerStateEvent {
		return { id: `tse-${Date.now().toString(36)}-${++this.#seq}`, at: new Date().toISOString(), kind, actor: this.#ctx.user(), reason, value };
	}

	#every(name: string): number {
		return (triggerDef(name)?.schedule?.everyMin ?? 60) * MIN;
	}

	#upcoming(name: string, until: number, limit = 400): number[] {
		const def = triggerDef(name)!;
		const now = Date.now();
		if (def.kind === 'calendar_schedule') return this.#fx.calendar.times.filter((t) => t > now && t <= until).slice(0, limit);
		if (!def.schedule) return [];
		const every = this.#every(name);
		const last = this.#fx.lastTickAt[name];
		const out: number[] = [];
		let t = last + every;
		if (!def.schedule.overdue) while (t <= now) t += every;
		for (; t <= until && out.length < limit; t += every) out.push(t);
		return out;
	}

	#ticksOf(name: string): TickRecord[] {
		return this.#fx.ticks.filter((t) => t.triggerName === name).sort((a, b) => b.scheduledFor.localeCompare(a.scheduledFor));
	}

	#triggerViews(scope: Scope): Trigger[] {
		return TRIGGER_DEFS.map((def) => this.#triggerView(def, scope));
	}

	#triggerView(def: TriggerDef, scope: Scope): Trigger {
		const rt = this.#runtime(scope, def.name);
		const id = triggerId(def.name);
		const lastTick = this.#ticksOf(def.name)[0] ?? null;
		const lastRunRec = this.#records
			.filter((r) => r.run.triggerId === id && !r.run.parentRunId)
			.sort((a, b) => b.run.queuedAt.localeCompare(a.run.queuedAt))[0];
		const lastRun = lastRunRec ? this.#view(lastRunRec, scope) : null;
		const isSchedule = def.kind === 'cron_schedule' || def.kind === 'calendar_schedule';
		const next = isSchedule && rt.state === 'running' ? (this.#upcoming(def.name, Date.now() + 40 * 1440 * MIN, 1)[0] ?? null) : null;
		const calendarLow = def.kind === 'calendar_schedule' && this.#fx.calendar.storedUntil - Date.now() < 7 * 1440 * MIN;
		const overdueMin = next != null && next < Date.now() ? Math.floor((Date.now() - next) / MIN) : null;
		return {
			id,
			name: def.name,
			type: isSchedule ? 'schedule' : 'sensor',
			kind: def.kind,
			description: def.description,
			jobKey: def.jobKey,
			cadence: def.cadence,
			cron: def.kind === 'cron_schedule' ? (this.#fx.crons[def.name] ?? null) : null,
			timezone: isSchedule ? 'UTC' : null,
			defaultState: def.defaultState,
			state: rt.state,
			nextDueAt: next ? new Date(next).toISOString() : null,
			lastTick: lastTick ? plainTick(lastTick) : null,
			lastRun: lastRun ? { id: lastRun.id, status: lastRun.status, startedAt: lastRun.startedAt, endedAt: lastRun.endedAt } : null,
			sourceFile: def.sourceFile,
			definitionChanged: !!def.definitionChanged,
			warning: calendarLow
				? `The calendar only has dates until ${new Date(this.#fx.calendar.storedUntil).toISOString().slice(0, 10)}, less than 7 days ahead`
				: overdueMin != null
					? `Should have started a run at ${new Date(next!).toISOString().slice(11, 16)} UTC; ${overdueMin} min late`
					: def.definitionChanged
						? 'Its code changed since it last checked'
						: null
		};
	}

	#triggerDetail(def: TriggerDef, scope: Scope): TriggerDetail {
		const base = this.#triggerView(def, scope);
		const rt = this.#runtime(scope, def.name);
		const isSchedule = base.type === 'schedule';
		const evaluations = structuredClone(this.#fx.evaluations[def.name] ?? []);
		const sensorFp = evaluations[0]?.definitionFingerprint ?? null;
		const currentFp = def.definitionChanged ? `${sensorFp ?? 'fp:0'}-2` : (sensorFp ?? 'fp:0');
		const runRequests = (this.#fx.runRequests[def.name] ?? []).map((r) => {
			const rec = this.#byId(r.runId);
			return { ...r, runStatus: rec ? this.#view(rec, scope).status : r.runStatus };
		});
		const declaration = def.code
			? { sourceFile: def.sourceFile, line: def.line, code: this.#fillCrons(def.code) }
			: (() => {
					const job = jobDef(def.jobKey)!;
					return { sourceFile: job.declaration.sourceFile, line: job.declaration.line, code: this.#fillCrons(job.declaration.code) };
				})();
		return {
			...base,
			declaration,
			targets: def.targets,
			catchUp: def.catchUp,
			upcoming: isSchedule && rt.state === 'running' ? this.#upcoming(def.name, Date.now() + 40 * 1440 * MIN, 8).map((t) => new Date(t).toISOString()) : [],
			calendar:
				def.kind === 'calendar_schedule'
					? {
							horizon: '30d',
							storedUntil: new Date(this.#fx.calendar.storedUntil).toISOString(),
							lastRefreshedAt: new Date(this.#fx.calendar.lastRefreshedAt).toISOString(),
							lowHorizonWarningBelow: '7d',
							low: this.#fx.calendar.storedUntil - Date.now() < 7 * 1440 * MIN
						}
					: null,
			sensor: def.sensor
				? {
						...def.sensor,
						cursor: rt.cursor,
						cursorUpdatedAt: rt.cursorUpdatedAt ? new Date(rt.cursorUpdatedAt).toISOString() : null,
						cursorKnown: true,
						currentFingerprint: currentFp,
						lastEvaluatedFingerprint: sensorFp
					}
				: null,
			ticks: this.#ticksOf(def.name).map(plainTick),
			evaluations,
			runRequests,
			stateHistory: structuredClone(rt.history)
		};
	}
}
