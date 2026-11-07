import type { components } from '$control/schema.d.ts';
import { isActiveRun } from '$lib/status';
import type { Principal } from './types';
import type { NodeCounts, Run, RunDetail, RunNode, RunNodeStatus, RunRow, RunRowPage, RunRowQuery, RunTabCounts } from './types/operate';

type Wire = components['schemas'];

/** The server lists at most this many runs per request; tabs, filters and pages apply to them. */
export const RUN_WINDOW = 1000;

const PERSON = 'principal:';

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
		requestedBy: requesterOf(wire),
		priority: wire.priority,
		rewriteHistory: wire.rewrite_history,
		locationId,
		target: wire.target,
		snapshotId: wire.snapshot_id,
		gitRevision: null,
		queuedAt: wire.queued_at,
		launchedAt: wire.launched_at ?? null,
		startedAt: wire.started_at ?? null,
		endedAt: wire.ended_at ?? null,
		nodeCounts: countsOf(wire),
		cost: null
	};
}

/** Runs keyed by id, each with its number of child runs in the listing. */
export function toRuns(wires: Wire['RunRecord'][], locationId: string): Run[] {
	const children = new Map<string, number>();
	for (const w of wires) if (w.parent_run_id) children.set(w.parent_run_id, (children.get(w.parent_run_id) ?? 0) + 1);
	return wires.map((w) => toRun(w, locationId, children.get(w.run_id) ?? 0));
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
		provenance: null,
		selection: wire.selection_json,
		waits: [],
		rerunFromFailureNodes: nodes.filter((n) => n.status === 'failed' || n.status === 'skipped').map((n) => n.nodeKey),
		rerunOf: run.rerunOfRunId ? (byId.get(run.rerunOfRunId) ?? null) : null,
		reruns: listed.filter((r) => r.rerunOfRunId === run.id)
	};
}
