import type { components } from '$control/schema.d.ts';
import { isActiveRun } from '$lib/status';
import type { Capability, Principal } from './types';
import type { AdhocRunCheck, LaunchVerb, NodeCounts, PlanDocument, PlanResult, QueueEntry, QueueWait, Run, RunDetail, RunListQuery, RunNode, RunNodeStatus, RunRow, RunRowPage, RunRowQuery, RunTabCounts, RunEvent, RunEventLevel, RunOutputLine, RunStatement, RunCostRow, CostGroup, CostSummary, NodeBuildCost } from './types/operate';
import { RECALL_EVENT_TYPES, SCHEDULER_FACTS } from './types/operate';

type Wire = components['schemas'];

/** The server lists at most this many runs per request; tabs, filters and pages apply to them. */
export const RUN_WINDOW = 1000;

const PERSON = 'principal:';

/** The location's answer to a plan request: the plan, `sqb plan`'s own error, or a superseded plan. */
export function planResultOf(outcome: Wire['LocationWorkOutcome']): PlanResult {
	if (outcome.kind === 'superseded') return { plan: null, error: 'The project changed on the location while it was planning; plan again' };
	if (outcome.kind !== 'plan') return { plan: null, error: 'The location answered with something other than a plan' };
	const document = outcome.document as Partial<PlanDocument> & { error?: { code?: string; message?: string; help?: string | null } };
	// The help line carries the fix, as `sqb plan` prints it under the error.
	if (document.error) {
		const { code, message, help } = document.error;
		return { plan: null, error: [code, message].filter(Boolean).join(': ') + (help ? `\nhelp: ${help}` : '') };
	}
	const list = <T>(value: T[] | undefined): T[] => value ?? [];
	return {
		plan: {
			selected_count: document.selected_count ?? 0,
			models: list(document.models),
			seeds: list(document.seeds),
			source_loads: list(document.source_loads),
			functions: list(document.functions),
			python_nodes: list(document.python_nodes),
			warnings: list(document.warnings),
			migrations: list(document.migrations),
			column_migrations: list(document.column_migrations),
			table_type_conversions: list(document.table_type_conversions),
			retention: list(document.retention),
			synced_copy: document.synced_copy ?? null,
			changed: document.changed ?? null
		},
		error: null
	};
}

/** The server's dry run of an ad-hoc command. */
export function toAdhocCheck(wire: Wire['AdhocCheck']): AdhocRunCheck {
	return {
		verb: (wire.verb ?? null) as LaunchVerb | null,
		selection: wire.selection,
		writeSet: wire.write_set,
		settings: wire.settings,
		permissions: wire.permissions.map((p) => ({ action: p.action as Capability, allowed: p.allowed, reason: p.reason ?? null })),
		error: wire.error ?? null,
		launchable: wire.launchable
	};
}

/** Who requested a run: a person by display name and principal ID, otherwise the trigger or system actor. */
export function requesterOf(wire: Wire['RunRecord']): Principal {
	if (wire.requested_by.startsWith(PERSON)) {
		const id = wire.requested_by.slice(PERSON.length);
		return { kind: 'user', name: id, displayName: wire.requester_name ?? id };
	}
	const actor = wire.requested_by;
	const kind = actor === 'system:scheduler' ? 'scheduler' : actor === 'system:sensor' ? 'sensor' : 'system';
	const name = wire.trigger_name ?? actor;
	return { kind, name, displayName: name };
}

function countsOf(wire: Wire['RunRecord']): NodeCounts {
	const c = wire.node_counts_json;
	const n = (key: string) => c[key] ?? 0;
	return {
		success: n('success'),
		warn: n('warn'),
		failed: n('failed') + n('cancelled'),
		skipped: n('skipped'),
		running: n('running'),
		pending: n('pending')
	};
}

export function toRun(wire: Wire['RunRecord'], locationId: string, childRunCount: number): Run {
	return {
		id: wire.run_id,
		kind: wire.kind,
		origin: wire.origin,
		status: wire.status,
		statusReason: wire.status_reason ?? null,
		jobKey: wire.job_key ?? null,
		parentRunId: wire.parent_run_id ?? null,
		rootRunId: wire.root_run_id,
		childRunCount,
		triggerId: wire.trigger_id ?? null,
		triggerName: wire.trigger_name ?? null,
		tickId: wire.tick_id ?? null,
		tickScheduledFor: null,
		runKey: null,
		rerunOfRunId: wire.rerun_of_run_id ?? null,
		narrowing: wire.narrowing ?? null,
		window: wire.window ? { start: String(wire.window.start), end: String(wire.window.end) } : null,
		requestedBy: requesterOf(wire),
		priority: wire.priority,
		rewriteHistory: wire.rewrite_history,
		locationId,
		target: wire.target,
		snapshotId: wire.snapshot_id,
		gitRevision: wire.git_revision ?? null,
		queuedAt: wire.queued_at,
		launchedAt: wire.launched_at ?? null,
		startedAt: wire.started_at ?? null,
		endedAt: wire.ended_at ?? null,
		nodeCounts: countsOf(wire),
		cost: null
	};
}

/** Runs keyed by id, each with its number of child runs in the listing. */
/** The queue in claim order; a run the read sent without queue state (it started meanwhile) is left out. */
export function queueEntriesOf(wires: Wire['RunRecord'][], locationId: string): QueueEntry[] {
	const runs = toRuns(wires, locationId);
	return wires.flatMap((wire, i) => (wire.queue ? [{ run: runs[i], position: wire.queue.position, waits: shownWaits(wire.queue.waits.map(toQueueWait)) }] : []));
}

/** A running run of the same job also holds the job's write claims; name it once. */
function shownWaits(waits: QueueWait[]): QueueWait[] {
	const sameJob = new Set(waits.flatMap((w) => (w.kind === 'job_running' ? [w.runId] : [])));
	return waits.filter((w) => w.kind !== 'write_claim' || !w.runId || !sameJob.has(w.runId));
}

function toQueueWait(wait: Wire['QueueWait']): QueueWait {
	switch (wait.kind) {
		case 'write_claim':
			return { kind: wait.kind, runId: wait.run_id ?? null, nodeKey: wait.node_key, otherNodes: wait.other_nodes };
		case 'concurrency_limit':
			return { kind: wait.kind, limit: wait.limit, holders: wait.holders, elsewhere: wait.elsewhere };
		case 'earlier_run':
			return { kind: wait.kind, runId: wait.run_id ?? null, limit: wait.limit };
		case 'upstream_job':
			return { kind: wait.kind, runId: wait.run_id, jobKey: wait.job_key };
		case 'job_running':
		case 'group_parent':
			return { kind: wait.kind, runId: wait.run_id };
		case 'retry_delay':
			return { kind: wait.kind, attempt: wait.attempt, dueAt: wait.due_at };
	}
}

export function toRuns(wires: Wire['RunRecord'][], locationId: string): Run[] {
	const children = new Map<string, number>();
	for (const w of wires) if (w.parent_run_id) children.set(w.parent_run_id, (children.get(w.parent_run_id) ?? 0) + 1);
	return wires.map((w) => toRun(w, locationId, children.get(w.run_id) ?? 0));
}

/** Listed runs under a run-list query, newest first. */
export function runsMatching(runs: Run[], query: RunListQuery): Run[] {
	const matching = runs.filter(
		(r) =>
			(!query.jobKey || r.jobKey === query.jobKey) &&
			(!query.triggerId || r.triggerId === query.triggerId) &&
			(!query.parentRunId || r.parentRunId === query.parentRunId) &&
			(!query.topLevelOnly || !r.parentRunId) &&
			(!query.statuses?.length || query.statuses.includes(r.status)) &&
			(!query.kinds?.length || query.kinds.includes(r.kind)) &&
			(!query.origins?.length || query.origins.includes(r.origin)) &&
			(!query.since || r.queuedAt >= query.since) &&
			(!query.until || r.queuedAt < query.until)
	);
	return query.limit === undefined ? matching : matching.slice(0, query.limit);
}

function rowOf(run: Run, wire: Wire['RunRecord']): RunRow {
	const c = run.nodeCounts;
	const total = c.success + c.warn + c.failed + c.skipped + c.running + c.pending;
	return {
		run,
		progress: { done: total - c.running - c.pending, total },
		waits: [],
		lastContactAt: null,
		lostContactLimitSeconds: 0,
		cancelRequestedAt: isActiveRun(run.status) ? (wire.cancel_requested_at ?? null) : null,
		selection: run.kind === 'adhoc' ? wire.selection_json : [],
		rerunFromFailureCount: (wire.node_counts_json.failed ?? 0) + c.skipped
	};
}

function inTab(run: Run, tab: RunRowQuery['tab']): boolean {
	if (tab === 'active') return isActiveRun(run.status);
	if (tab === 'queued') return run.status === 'queued';
	if (tab === 'failed') return run.status === 'failed' || run.status === 'partial' || run.status === 'presumed_failed';
	if (tab === 'cancelled') return run.status === 'cancelled';
	return true;
}

/** Whether a run passes a runs-list query, apart from its tab. Node-key filters need nodes and match nothing here. */
function matches(run: Run, wire: Wire['RunRecord'], query: RunRowQuery): boolean {
	const f = query.filter ?? {};
	const lower = (xs?: string[]) => (xs?.length ? xs.map((x) => x.toLowerCase()) : null);
	if (query.parentRunId) {
		if (run.parentRunId !== query.parentRunId) return false;
	} else if (!query.includeChildren && run.parentRunId) return false;
	if (query.since && run.queuedAt < query.since) return false;
	if (query.until && run.queuedAt >= query.until) return false;
	if (f.kinds?.length && !f.kinds.includes(run.kind)) return false;
	if (f.origins?.length && !f.origins.includes(run.origin)) return false;
	if (f.statuses?.length && !f.statuses.includes(run.status)) return false;
	const ids = lower(f.ids)?.map((id) => id.replace(/^run-/, ''));
	if (ids && !ids.some((id) => run.id.toLowerCase().startsWith(id))) return false;
	const jobs = lower(f.jobKeys);
	if (jobs && !jobs.includes((run.jobKey ?? '').toLowerCase())) return false;
	const trigger = (run.triggerName ?? '').toLowerCase();
	const schedules = lower(f.schedules);
	if (schedules && !(run.origin === 'schedule' && schedules.includes(trigger))) return false;
	const sensors = lower(f.sensors);
	if (sensors && !(run.origin === 'sensor' && sensors.includes(trigger))) return false;
	const by = lower(f.requestedBy);
	if (by && !by.includes(run.requestedBy.name.toLowerCase()) && !by.includes(run.requestedBy.displayName.toLowerCase())) return false;
	if (f.nodeKeys?.length && !f.nodeKeys.some((k) => wire.selection_json.includes(k))) return false;
	const text = f.text?.trim().toLowerCase() ?? '';
	if (text && !`${run.id} ${run.jobKey ?? ''} ${run.triggerName ?? ''} ${run.requestedBy.name} ${run.requestedBy.displayName}`.toLowerCase().includes(text)) return false;
	return true;
}

/** One page of the listing under a runs-list query; the cursor is an offset into the newest runs. */
export function runRowPage(wires: Wire['RunRecord'][], locationId: string, query: RunRowQuery): RunRowPage {
	const runs = toRuns(wires, locationId);
	const matching = runs.flatMap((run, i) => (matches(run, wires[i], query) && inTab(run, query.tab) ? [rowOf(run, wires[i])] : []));
	const offset = Number(query.cursor ?? 0) || 0;
	const limit = query.limit ?? 50;
	return {
		rows: matching.slice(offset, offset + limit),
		nextCursor: offset + limit < matching.length ? String(offset + limit) : null
	};
}

/** A rows cursor: the offset into the matching rows, then `+N` when N windows older than the newest are loaded. */
export function parseRunCursor(cursor: string | null | undefined): { offset: number; older: number } {
	const [offset, older] = (cursor ?? '0').split('+');
	return { offset: Number(offset) || 0, older: Number(older) || 0 };
}

/** A page over the loaded windows, newest first; once they run out and the oldest was full, the next page loads one more window. */
export function runRowPageOver(windows: Wire['RunRecord'][][], locationId: string, query: RunRowQuery): RunRowPage {
	const { offset, older } = parseRunCursor(query.cursor);
	const page = runRowPage(windows.flat(), locationId, { ...query, cursor: String(offset) });
	const more = windows.at(-1)?.length === RUN_WINDOW;
	const next = page.nextCursor ?? (more ? String(offset + page.rows.length) : null);
	const loaded = page.nextCursor ? older : older + 1;
	return { rows: page.rows, nextCursor: next !== null && loaded ? `${next}+${loaded}` : next };
}

export function runTabCountsOf(wires: Wire['RunRecord'][], locationId: string, query: RunRowQuery): RunTabCounts {
	const runs = toRuns(wires, locationId).filter((run, i) => matches(run, wires[i], query));
	return {
		active: runs.filter((r) => isActiveRun(r.status)).length,
		queued: runs.filter((r) => r.status === 'queued').length
	};
}

function nodeStatus(status: string): RunNodeStatus {
	return (['pending', 'running', 'success', 'warn', 'failed', 'skipped', 'cancelled'] as const).find((s) => s === status) ?? 'not_run';
}

export function toRunNode(wire: Wire['RunNodeRecord'], run: Run): RunNode {
	return {
		nodeKey: wire.node_key,
		kind: null,
		runId: run.id,
		jobKey: run.jobKey,
		status: nodeStatus(wire.status),
		attempt: 1,
		startedAt: wire.started_at ?? null,
		endedAt: wire.ended_at ?? null,
		rowsAffected: wire.rows_affected ?? null,
		errorCode: wire.error_code ?? null,
		errorMessage: null,
		errorLocation: null,
		errorDetail: null,
		skippedBecauseOf: wire.skipped_because_of ? { nodeKey: wire.skipped_because_of, runId: run.id, jobKey: run.jobKey } : null,
		critical: false
	};
}

/**
 * A run's detail from the listing and the nodes of the run and its child runs; parts with no server source stay empty.
 * Rerun from failure covers failed and skipped nodes only, as the server does; cancelled nodes need a full rerun.
 */
export function runDetailOf(run: Run, wire: Wire['RunRecord'], listed: Run[], nodes: RunNode[]): RunDetail {
	const byId = new Map(listed.map((r) => [r.id, r]));
	return {
		run,
		parent: run.parentRunId ? (byId.get(run.parentRunId) ?? null) : null,
		children: listed.filter((r) => r.parentRunId === run.id).sort((a, b) => a.queuedAt.localeCompare(b.queuedAt)),
		nodes,
		edges: [],
		invocations: [],
		liveness: null,
		cancel: null,
		provenance:
			wire.snapshot_generation != null
				? {
						snapshotId: wire.snapshot_id,
						snapshotGeneration: wire.snapshot_generation,
						gitRevision: wire.git_revision ?? null,
						gitDirty: wire.git_dirty ?? false,
						imageId: wire.image_id ?? null,
						sqbVersion: wire.sqb_version ?? '',
						selection: wire.selection_json,
						exclude: [],
						command: null
					}
				: null,
		selection: wire.selection_json,
		waits: [],
		rerunFromFailureNodes: nodes.filter((n) => n.status === 'failed' || n.status === 'skipped').map((n) => n.nodeKey),
		rerunOf: run.rerunOfRunId ? (byId.get(run.rerunOfRunId) ?? null) : null,
		reruns: listed.filter((r) => r.rerunOfRunId === run.id)
	};
}

/**
 * A run's nodes with its job runs' nodes. A group run's own rows repeat its job runs' nodes, so they
 * stay only for nodes no job run has, such as those of members that never started.
 */
export function groupRunNodes(own: RunNode[], children: RunNode[]): RunNode[] {
	const covered = new Set(children.map((n) => n.nodeKey));
	return [...own.filter((n) => !covered.has(n.nodeKey)), ...children];
}

const KNOWN_EVENTS: ReadonlySet<string> = new Set(RECALL_EVENT_TYPES);
/** Scheduler facts are stored as `run_<fact>` events in the scheduler invocation. */
const SCHEDULER_EVENTS: ReadonlySet<string> = new Set(SCHEDULER_FACTS.map((f) => `run_${f}`));

/** The node key a Recall resource ID names: `[invocation, key]`, or null for run-level rows. */
function recallNodeKey(resourceId: string | null | undefined): string | null {
	if (!resourceId) return null;
	try {
		const parsed: unknown = JSON.parse(resourceId);
		return Array.isArray(parsed) && typeof parsed[1] === 'string' ? parsed[1] : null;
	} catch {
		return null;
	}
}

function levelOf(severity: string | null | undefined): RunEventLevel {
	if (severity === 'warning' || severity === 'warn') return 'warn';
	if (severity === 'error' || severity === 'critical') return 'error';
	return severity === 'debug' ? 'debug' : 'info';
}

function durationText(ms: number | null | undefined): string {
	if (ms === null || ms === undefined) return '';
	return ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(1)} s`;
}

/**
 * A run event from its metadata. The server leaves payloads out because they can carry SQL and
 * error text, so the message is built from the type, counts, duration and error code only.
 * Event kinds the run page does not show (operations, node projections) are dropped.
 */
export function toRunEvent(wire: Wire['RunEventRecord'], runId: string): RunEvent | null {
	const scheduler = SCHEDULER_EVENTS.has(wire.event_type);
	if (!scheduler && !KNOWN_EVENTS.has(wire.event_type)) return null;
	const type = (scheduler ? wire.event_type.slice('run_'.length) : wire.event_type) as RunEvent['type'];
	const rows = wire.affected_rows ?? wire.row_count;
	const failure = [wire.error_type, wire.error_code].filter(Boolean).join(': ');
	const facts = [durationText(wire.duration_ms), rows === null || rows === undefined ? '' : `${rows} ${rows === 1 ? 'row' : 'rows'}`].filter(Boolean).join(' · ');
	const message = type === 'requested' ? 'Requested' : type === 'cancel_requested' ? 'Cancel requested' : wire.event_type.endsWith('_failed') ? failure || facts || 'no error code recorded' : type === 'audit_completed' ? (wire.outcome ?? '') : facts;
	return {
		id: wire.event_id,
		storageId: wire.storage_id,
		runId,
		producer: wire.invocation_id.startsWith('scheduler/') ? 'scheduler' : 'sqb',
		type,
		occurredAt: wire.occurred_at,
		level: levelOf(wire.severity),
		invocationId: wire.invocation_id,
		nodeKey: recallNodeKey(wire.resource_id),
		statementId: wire.statement_id ?? null,
		attempt: null,
		message
	};
}

/** Statement metadata in start order; error text is never sent, only its type and code. */
export function toRunStatements(wire: Wire['RunStatementRecord'][], runId: string): RunStatement[] {
	return wire.map((s, i) => ({
		id: s.statement_id,
		runId,
		invocationId: s.invocation_id,
		sequence: i + 1,
		nodeKey: recallNodeKey(s.resource_id) ?? s.resource_name ?? '',
		kind: s.statement_kind ?? s.purpose ?? 'statement',
		status: s.outcome === 'failed' ? 'failed' : s.outcome === 'cancelled' ? 'cancelled' : s.ended_at || s.outcome ? 'completed' : 'running',
		startedAt: s.started_at ?? '',
		endedAt: s.ended_at ?? null,
		rowsAffected: s.affected_rows ?? s.row_count ?? null,
		rowsSoFar: null,
		queryId: s.query_id ?? null,
		sqlRecorded: s.sql_available,
		error: [s.error_type, s.error_code].filter(Boolean).join(': ') || null
	}));
}

/** Captured lines and error text in time order; error text reads as stderr of its invocation. */
export function toRunOutput(wire: Wire['RunOutput']): RunOutputLine[] {
	const lines: RunOutputLine[] = wire.lines.map((l) => ({ invocationId: l.invocation_id, stream: l.stream === 'stderr' ? 'stderr' : 'stdout', at: l.occurred_at, text: l.text, error: false }));
	const errors: RunOutputLine[] = wire.errors.map((e) => ({ invocationId: e.invocation_id, stream: 'stderr', at: e.occurred_at ?? '', text: e.message, error: true }));
	return [...lines, ...errors].sort((a, b) => a.at.localeCompare(b.at));
}

const COST_STATUSES: RunCostRow['status'][] = ['complete', 'partial', 'pending', 'collection_failed', 'unavailable'];

const costStatus = (status: string): RunCostRow['status'] => COST_STATUSES.find((s) => s === status) ?? 'unavailable';

const toCostGroup = (g: Wire['CostGroup']): CostGroup => ({ key: g.key ?? null, credits: g.credits, usd: g.usd ?? null, seconds: g.seconds, runs: g.runs });

export function toCostSummary(wire: Wire['CostSummaryRecord']): CostSummary {
	return {
		since: wire.since,
		credits: wire.credits,
		usd: wire.usd ?? null,
		runs: wire.runs,
		incompleteRuns: wire.incomplete_runs,
		byJob: wire.by_job.map(toCostGroup),
		byWarehouseSize: wire.by_warehouse_size.map(toCostGroup),
		byNode: wire.by_node.map(toCostGroup)
	};
}

export function toNodeBuildCosts(wire: Wire['NodeCostRecord'][]): NodeBuildCost[] {
	return wire.map((c) => ({
		buildRunId: c.run_id,
		runId: c.orchestration_run_id ?? null,
		startedAt: c.run_started_at,
		status: costStatus(c.status),
		credits: c.credits,
		usd: c.usd ?? null,
		seconds: c.seconds,
		queryCount: c.query_count
	}));
}

/** Latest cost observations as numbers; a status the UI does not know reads as unavailable. */
export function toRunCostRows(wire: Wire['WarehouseResourceCostRecord'][]): RunCostRow[] {
	return wire.map((r) => ({
		buildRunId: r.run_id,
		resourceType: r.resource_type,
		resourceName: r.resource_name,
		// Catalog node keys are `kind:name`, and a resource's type is its node kind.
		nodeKey: r.resource_type === '_total' ? null : `${r.resource_type}:${r.resource_name}`,
		warehouseName: r.warehouse_name,
		warehouseSize: r.warehouse_size,
		queryCount: r.query_count,
		seconds: Number(r.attributed_seconds),
		bytesScanned: r.bytes_scanned,
		credits: Number(r.estimated_credits),
		usd: r.estimated_usd == null ? null : Number(r.estimated_usd),
		status: costStatus(r.status),
		statementsExpected: r.statements_expected,
		statementsObserved: r.statements_observed
	}));
}
