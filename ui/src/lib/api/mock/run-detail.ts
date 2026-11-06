// Derives a mock run's execution detail (nodes, Recall events, statements,
// output) from its record and the clock, the way the server projects it from
// facts. Pure: the same record and clock always give the same detail.

import type {
	CancelProgress,
	LaunchVerb,
	NodeCounts,
	Run,
	RunEvent,
	RunEventLevel,
	RunInvocation,
	RunLiveness,
	RunNode,
	RunNodeStatus,
	RunOutputLine,
	RunStatement,
	RunStatus,
	SkippedBecauseOf
} from '../types';
import { ancestors, snapshotNode, sortTopologically } from './operate-graph';

export type RunEnd = 'complete' | 'cancelled' | 'presumed_failed' | 'never_started';

export type RunScript = {
	verb: LaunchVerb;
	select: string[];
	exclude: string[];
	flags: Record<string, string | true>;
	/** Nodes this run executes; empty for group parents. */
	nodes: string[];
	fail: string[];
	warn: string[];
	errors: Record<string, string>;
	critical: string[];
	plannedMs: number;
	/** How the run ends, once terminal. */
	end: RunEnd;
	cancel: CancelProgress | null;
	/** Last heartbeat; node progress stops here for unresponsive and lost runs. */
	lastHeartbeatAt: number | null;
	imageId: string;
	snapshotGeneration: number;
	/** Concurrency groups the run takes a slot in. */
	concurrencyGroups: string[];
};

export type RunRecord = { run: Run; script: RunScript };

const ACTIVE: ReadonlySet<RunStatus> = new Set(['queued', 'launched', 'running', 'unresponsive']);
export const isActive = (status: RunStatus) => ACTIVE.has(status);
export const isTerminal = (status: RunStatus) => !ACTIVE.has(status);

function hash(text: string): number {
	let h = 2166136261;
	for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
	return h >>> 0;
}

const iso = (ms: number) => new Date(ms).toISOString();
const ms = (value: string | null) => (value ? Date.parse(value) : null);

type Timed = { key: string; start: number; end: number };

/** Natural schedule in relative units: each node starts when its in-run parents finish. */
function schedule(nodes: string[]): { timed: Timed[]; total: number } {
	const order = sortTopologically(nodes);
	const set = new Set(order);
	const endOf = new Map<string, number>();
	const timed: Timed[] = [];
	for (const key of order) {
		const n = snapshotNode(key);
		const parents = (n?.upstream ?? []).filter((p) => set.has(p));
		const start = Math.max(0, ...parents.map((p) => endOf.get(p) ?? 0));
		const weight = n?.kind === 'incremental' ? 2.2 : n?.kind === 'table' ? 1.6 : n?.kind === 'loader' ? 2.5 : 1;
		const end = start + (1 + (hash(key) % 7)) * weight;
		endOf.set(key, end);
		timed.push({ key, start, end });
	}
	return { timed, total: Math.max(1, ...timed.map((t) => t.end)) };
}

/** Failed nodes in the run's group siblings that have finished, for cross-job skips. */
function siblingFailures(record: RunRecord, siblings: RunRecord[]): Map<string, { runId: string; jobKey: string | null }> {
	const out = new Map<string, { runId: string; jobKey: string | null }>();
	for (const s of siblings) {
		if (s.run.id === record.run.id || !isTerminal(s.run.status) || !s.run.startedAt) continue;
		for (const key of s.script.fail) out.set(key, { runId: s.run.id, jobKey: s.run.jobKey });
	}
	return out;
}

/** The instant node progress is read at for this run. */
function clockOf(record: RunRecord, now: number): number {
	const { run, script } = record;
	let clock = now;
	if (run.endedAt) clock = Math.min(clock, Date.parse(run.endedAt));
	if (script.lastHeartbeatAt != null && run.status !== 'running') clock = Math.min(clock, script.lastHeartbeatAt);
	if (script.cancel?.sigtermAt) clock = Math.min(clock, Date.parse(script.cancel.sigtermAt));
	return clock;
}

export function computeNodes(record: RunRecord, siblings: RunRecord[], now: number): RunNode[] {
	const { run, script } = record;
	if (!script.nodes.length) return [];
	const started = ms(run.startedAt);
	const terminal = isTerminal(run.status);
	const base = (key: string, status: RunNodeStatus): RunNode => ({
		nodeKey: key,
		kind: snapshotNode(key)?.kind ?? 'table',
		runId: run.id,
		jobKey: run.jobKey,
		status,
		attempt: 1,
		startedAt: null,
		endedAt: null,
		rowsAffected: null,
		errorCode: null,
		errorMessage: null,
		errorLocation: null,
		errorDetail: null,
		skippedBecauseOf: null,
		critical: script.critical.includes(key)
	});
	if (started == null) {
		const status: RunNodeStatus = terminal ? (run.status === 'cancelled' ? 'cancelled' : 'not_run') : 'pending';
		return sortTopologically(script.nodes).map((k) => base(k, status));
	}

	const { timed, total } = schedule(script.nodes);
	const t0 = started + 1500;
	const unit = Math.max(200, script.plannedMs - 3000) / total;
	const clock = clockOf(record, now);
	const failed = new Map<string, SkippedBecauseOf & { at: number }>();
	for (const [key, src] of siblingFailures(record, siblings)) failed.set(key, { nodeKey: key, ...src, at: started });

	return timed.map(({ key, start, end }) => {
		const s = t0 + start * unit;
		const e = t0 + end * unit;
		const ups = ancestors(key);
		const because = [...failed.values()].filter((f) => ups.has(f.nodeKey)).sort((a, b) => b.at - a.at)[0];
		if (because) {
			return {
				...base(key, 'skipped'),
				skippedBecauseOf: { nodeKey: because.nodeKey, runId: because.runId, jobKey: because.jobKey },
				endedAt: iso(Math.max(because.at, started))
			};
		}
		const finish = (): RunNode => {
			const fails = script.fail.includes(key);
			const status: RunNodeStatus = fails ? 'failed' : script.warn.includes(key) ? 'warn' : 'success';
			if (fails) failed.set(key, { nodeKey: key, runId: run.id, jobKey: run.jobKey, at: e });
			const message = fails ? (script.errors[key] ?? 'Statement failed in the warehouse') : null;
			return {
				...base(key, status),
				startedAt: iso(s),
				endedAt: iso(e),
				rowsAffected: fails || snapshotNode(key)?.kind === 'view' ? null : 120 + (hash(key + run.id) % 48000),
				errorCode: fails ? 'E2103' : null,
				errorMessage: message,
				...(fails ? failureDetail(key, run, message ?? '') : {})
			};
		};
		if (terminal && script.end === 'complete') return finish();
		if (e <= clock) return finish();
		if (s <= clock) {
			if (!terminal) return { ...base(key, 'running'), startedAt: iso(s) };
			if (script.end === 'presumed_failed')
				return {
					...base(key, 'failed'),
					startedAt: iso(s),
					endedAt: run.endedAt,
					errorCode: 'worker_lost',
					errorMessage: 'Lost contact with the run while this node was building',
					errorDetail: 'The run stopped sending heartbeats while this node was building and was marked failed.\nThe warehouse statement may still have finished; rerunning is safe because the write is idempotent.'
				};
			return { ...base(key, 'cancelled'), startedAt: iso(s), endedAt: run.endedAt };
		}
		if (!terminal) return base(key, 'pending');
		return base(key, script.end === 'cancelled' ? 'cancelled' : 'not_run');
	});
}

/** Mock failure location and full text: the model file, and a line inside its main statement. */
function failureDetail(key: string, run: Run, message: string): Pick<RunNode, 'errorLocation' | 'errorDetail'> {
	const file = snapshotNode(key)?.path ?? `models/${key}.sql`;
	const line = 4 + (hash(key) % 30);
	const queryId = queryIdOf(statementIdOf(run, key));
	return {
		errorLocation: { file, line },
		errorDetail: [
			`error[E2103]: ${message}`,
			`  --> ${file}:${line}`,
			`   |`,
			`${String(line).padStart(3)} |   ${key.includes('revenue') ? 'sum(amount) / count(distinct order_id) as revenue_per_order' : 'cast(lifetime_value as number(10, 2)) as lifetime_value'}`,
			`   |   ${'^'.repeat(24)}`,
			``,
			`Warehouse query ${queryId} failed after ${1 + (hash(key) % 40)}s.`,
			`Statement: ${STATEMENT_KIND[snapshotNode(key)?.kind ?? 'table'] ?? 'create_table_as'} ${key}`
		].join('\n')
	};
}

export function statementIdOf(run: Run, nodeKey: string): string {
	return `stmt-${run.id.replace(/^run-/, '')}-${nodeKey}`;
}

/** A warehouse-style query ID, stable per statement. */
export function queryIdOf(statementId: string): string {
	const h = (n: number) => hash(`${statementId}:${n}`).toString(16).padStart(8, '0');
	return `${h(1)}-0000-${h(2).slice(0, 4)}-${h(3).slice(0, 4)}-${h(4)}${h(5).slice(0, 4)}`;
}

export function countNodes(nodes: RunNode[]): NodeCounts {
	const c: NodeCounts = { success: 0, warn: 0, failed: 0, skipped: 0, running: 0, pending: 0 };
	for (const n of nodes) {
		if (n.status === 'success') c.success++;
		else if (n.status === 'warn') c.warn++;
		else if (n.status === 'failed') c.failed++;
		else if (n.status === 'skipped' || n.status === 'cancelled' || n.status === 'not_run') c.skipped++;
		else if (n.status === 'running') c.running++;
		else c.pending++;
	}
	return c;
}

/** Terminal status from node outcomes: partial when something failed and everything else completed. */
export function deriveTerminalStatus(end: RunEnd, nodes: RunNode[]): RunStatus {
	if (end === 'never_started') return 'failed';
	if (end === 'cancelled') return 'cancelled';
	if (end === 'presumed_failed') return 'presumed_failed';
	const c = countNodes(nodes);
	const blocked = c.failed + nodes.filter((n) => n.skippedBecauseOf).length;
	if (blocked) return c.success + c.warn > 0 ? 'partial' : 'failed';
	return c.warn ? 'warn' : 'success';
}

export function statusReason(status: RunStatus, nodes: RunNode[]): string | null {
	const c = countNodes(nodes);
	if (status === 'partial')
		return c.failed
			? `${c.failed} node${c.failed === 1 ? '' : 's'} failed and ${c.skipped} that depend on ${c.failed === 1 ? 'it' : 'them'} were skipped; the other ${c.success + c.warn} finished`
			: `${c.skipped} node${c.skipped === 1 ? '' : 's'} skipped because something they depend on failed; the other ${c.success + c.warn} finished`;
	if (status === 'failed' && c.failed) {
		const first = nodes.find((n) => n.status === 'failed');
		return first ? `${first.nodeKey} failed: ${first.errorMessage}` : null;
	}
	if (status === 'failed' && c.skipped) {
		const why = nodes.find((n) => n.skippedBecauseOf)?.skippedBecauseOf;
		return why ? `Every node was skipped: ${why.nodeKey} failed in run ${why.runId.replace(/^run-/, '')}` : null;
	}
	return null;
}

// ---- Recall events, statements and output ----

const LEVEL: Record<RunEvent['type'], RunEventLevel> = {
	invocation_started: 'info',
	invocation_completed: 'info',
	invocation_failed: 'error',
	resource_attempt_started: 'debug',
	resource_attempt_completed: 'info',
	resource_attempt_failed: 'error',
	resource_attempt_skipped: 'warn',
	statement_submitted: 'debug',
	statement_started: 'debug',
	statement_completed: 'debug',
	statement_failed: 'error',
	audit_completed: 'info',
	requested: 'info',
	queued: 'info',
	claimed: 'info',
	launched: 'info',
	cancel_requested: 'warn',
	presumed_failed: 'error',
	terminated: 'info'
};

export function invocationId(run: Run, attempt = 1): string {
	return `inv-${run.id.replace(/^run-/, '')}-${attempt}`;
}

export function commandOf(record: RunRecord): string {
	const { script, run } = record;
	const parts = [`sqb ${script.verb}`];
	if (script.select.length) parts.push(`--select ${script.select.join(' ')}`);
	if (script.exclude.length) parts.push(`--exclude ${script.exclude.join(' ')}`);
	for (const [flag, value] of Object.entries(script.flags)) parts.push(value === true ? flag : `${flag} ${value}`);
	parts.push(`--target ${run.target}`);
	return parts.join(' ');
}

export function invocations(record: RunRecord): RunInvocation[] {
	const { run } = record;
	if (!run.startedAt || !record.script.nodes.length) return [];
	const exitCode = !run.endedAt
		? null
		: run.status === 'cancelled'
			? 143
			: run.status === 'presumed_failed'
				? 137
				: run.status === 'success' || run.status === 'warn'
					? 0
					: 1;
	return [
		{
			invocationId: invocationId(run),
			runId: run.id,
			attempt: 1,
			command: commandOf(record),
			startedAt: run.startedAt,
			endedAt: run.endedAt,
			exitCode
		}
	];
}

const STATEMENT_KIND: Partial<Record<string, string>> = {
	view: 'create_view',
	table: 'create_table_as',
	incremental: 'merge',
	seed: 'load_seed',
	loader: 'loader_insert'
};

export function sqlFor(key: string, kind: string, target: string): string {
	const rel = `${target === 'dev' ? 'dev_demo' : 'analytics'}.${key}`;
	const n = snapshotNode(key);
	const from = (n?.upstream ?? []).map((u) => `analytics.${u}`);
	const src = from[0] ?? 'staging.incoming_rows';
	if (kind === 'create_view') return `create or replace view ${rel} as\nselect *\nfrom ${src}`;
	if (kind === 'merge')
		return `merge into ${rel} as t\nusing (\n  select * from ${src}\n  where updated_at >= :start_cursor and updated_at < :end_cursor\n) as s\non t.id = s.id\nwhen matched then update set *\nwhen not matched then insert *`;
	if (kind === 'load_seed') return `insert into ${rel}\nselect * from read_csv('seeds/${key}.csv')`;
	if (kind === 'loader_insert') return `insert into ${rel}\nselect * from staging.${key}__batch`;
	return `create or replace table ${rel}__sqb_stage as\nselect *\nfrom ${from.join('\njoin ') || src};\n\nalter table ${rel}__sqb_stage swap with ${rel}`;
}

export type RunExecution = { events: RunEvent[]; statements: RunStatement[]; output: RunOutputLine[] };

/** Mock: SQL capture misses a few statements, as when capture was off or retention dropped the text. */
export function sqlRecorded(statementId: string): boolean {
	return hash(statementId) % 11 !== 0;
}

export function execution(record: RunRecord, nodes: RunNode[], now?: number): RunExecution {
	const { run, script } = record;
	type Draft = Omit<RunEvent, 'id' | 'storageId' | 'level' | 'runId' | 'producer'> & { producer?: RunEvent['producer'] };
	const drafts: Draft[] = [];
	const statements: RunStatement[] = [];
	const output: RunOutputLine[] = [];
	const inv = run.startedAt && script.nodes.length ? invocationId(run) : null;
	const fact = (type: Draft['type'], at: string | number, message: string) =>
		drafts.push({ producer: 'scheduler', type, occurredAt: typeof at === 'number' ? iso(at) : at, invocationId: null, nodeKey: null, statementId: null, attempt: null, message });
	const ev = (type: Draft['type'], at: number, nodeKey: string | null, message: string, statementId: string | null = null) =>
		drafts.push({ type, occurredAt: iso(at), invocationId: inv, nodeKey, statementId, attempt: nodeKey ? 1 : null, message });
	const out = (stream: RunOutputLine['stream'], at: number, text: string) => inv && output.push({ invocationId: inv, stream, at: iso(at), text });

	const queued = Date.parse(run.queuedAt);
	fact('requested', queued - 200, `Requested by ${run.requestedBy.displayName}${run.triggerName ? ` via ${run.triggerName}` : ''}`);
	fact('queued', queued, `Queued at priority ${run.priority}`);
	if (run.launchedAt) {
		const launched = Date.parse(run.launchedAt);
		fact('claimed', launched - 300, script.nodes.length ? `Claimed ${script.nodes.length} write-set nodes${script.concurrencyGroups.length ? ` and a slot in ${script.concurrencyGroups.join(', ')}` : ''}` : 'Claimed');
		fact('launched', launched, script.nodes.length ? `Container started from image ${script.imageId.slice(0, 19)}` : `Started ${run.childRunCount} child runs in order`);
	}
	if (script.cancel) fact('cancel_requested', script.cancel.requestedAt, `Cancel requested by ${script.cancel.requestedBy.displayName}`);

	if (inv && run.startedAt) {
		const start = Date.parse(run.startedAt);
		ev('invocation_started', start, null, commandOf(record));
		out('stdout', start, `$ ${commandOf(record)}`);
		out('stdout', start + 400, `Compiled ${script.nodes.length} nodes against snapshot generation ${script.snapshotGeneration}`);
		for (const n of nodes) {
			const k = snapshotNode(n.nodeKey)?.kind ?? 'table';
			if (n.status === 'skipped' && n.endedAt) {
				const why = n.skippedBecauseOf;
				const msg = why ? `Skipped: ${why.nodeKey} failed${why.runId !== run.id ? ` in run ${why.runId.replace(/^run-/, '')}` : ''}` : 'Skipped';
				ev('resource_attempt_skipped', Date.parse(n.endedAt), n.nodeKey, msg);
				out('stdout', Date.parse(n.endedAt), `SKIP  ${n.nodeKey}  ${msg.toLowerCase()}`);
				continue;
			}
			if (!n.startedAt) continue;
			const s = Date.parse(n.startedAt);
			const e = n.endedAt ? Date.parse(n.endedAt) : null;
			ev('resource_attempt_started', s, n.nodeKey, `Started ${k} ${n.nodeKey}`);
			const kind = STATEMENT_KIND[k];
			if (kind) {
				const statementId = statementIdOf(run, n.nodeKey);
				const stEnd = e != null ? e - 120 : null;
				const failed = n.status === 'failed';
				const running = n.status === 'running';
				statements.push({
					id: statementId,
					runId: run.id,
					invocationId: inv,
					sequence: statements.length + 1,
					nodeKey: n.nodeKey,
					kind,
					status: running ? 'running' : n.status === 'cancelled' ? 'cancelled' : failed ? 'failed' : 'completed',
					startedAt: iso(s + 60),
					endedAt: stEnd == null ? null : iso(stEnd),
					rowsAffected: n.rowsAffected,
					rowsSoFar: running ? Math.round(((now ?? s) - s) / 1000) * (37 + (hash(n.nodeKey) % 400)) : null,
					queryId: run.target === 'dev' ? null : queryIdOf(statementId),
					sqlRecorded: sqlRecorded(statementId),
					error: failed ? (n.errorDetail ?? n.errorMessage) : null
				});
				ev('statement_submitted', s + 40, n.nodeKey, `${kind} submitted`, statementId);
				ev('statement_started', s + 60, n.nodeKey, `${kind} running`, statementId);
				if (stEnd != null) {
					if (failed) ev('statement_failed', stEnd, n.nodeKey, n.errorMessage ?? 'Statement failed', statementId);
					else if (n.status !== 'cancelled') ev('statement_completed', stEnd, n.nodeKey, n.rowsAffected != null ? `${n.rowsAffected.toLocaleString('en-GB')} rows` : 'Completed', statementId);
				}
			}
			if (e == null) continue;
			if (n.status === 'warn') {
				ev('audit_completed', e - 60, n.nodeKey, 'not_null(payment_id): 2 failing rows (severity warn)');
				out('stdout', e - 60, `WARN  ${n.nodeKey}  audit not_null(payment_id): 2 rows`);
			} else if (n.status === 'success' && k !== 'task' && k !== 'loader') {
				ev('audit_completed', e - 60, n.nodeKey, 'All audits passed');
			}
			const dur = ((e - s) / 1000).toFixed(2);
			if (n.status === 'failed') {
				ev('resource_attempt_failed', e, n.nodeKey, `${n.errorCode}: ${n.errorMessage}`);
				out('stdout', e, `FAIL  ${n.nodeKey}  ${dur}s`);
				out('stderr', e, `error[${n.errorCode}] ${n.nodeKey}: ${n.errorMessage}`);
			} else if (n.status === 'cancelled') {
				ev('resource_attempt_failed', e, n.nodeKey, 'Cancelled: its query was stopped by the cancel');
				out('stderr', e, `${n.nodeKey}: statement cancelled`);
			} else {
				ev('resource_attempt_completed', e, n.nodeKey, `${n.status === 'warn' ? 'Completed with warnings' : 'Completed'} in ${dur}s`);
				out('stdout', e, `OK    ${n.nodeKey}  ${k}  ${dur}s`);
			}
		}
		if (run.endedAt) {
			const end = Date.parse(run.endedAt) - 400;
			const c = countNodes(nodes);
			const summary = `${c.success} ok · ${c.warn} warn · ${c.failed} failed · ${c.skipped} skipped`;
			const ok = run.status === 'success' || run.status === 'warn';
			ev(ok ? 'invocation_completed' : 'invocation_failed', end, null, summary);
			out(ok ? 'stdout' : 'stderr', end, `${run.status === 'cancelled' ? 'cancelled' : ok ? 'done' : 'failed'}  ${summary}`);
		}
	}

	if (run.status === 'presumed_failed' && run.endedAt) fact('presumed_failed', Date.parse(run.endedAt) - 100, run.statusReason ?? 'No heartbeat; worker killed');
	if (run.endedAt) fact('terminated', run.endedAt, `Terminated: ${run.status}${run.statusReason ? ` (${run.statusReason})` : ''}`);

	const events = drafts
		.sort((a, b) => a.occurredAt.localeCompare(b.occurredAt))
		.map((d, i): RunEvent => ({
			id: `${run.id}-e${i + 1}`,
			storageId: i + 1,
			runId: run.id,
			producer: d.producer ?? 'sqb',
			type: d.type,
			occurredAt: d.occurredAt,
			level: LEVEL[d.type],
			invocationId: d.invocationId,
			nodeKey: d.nodeKey,
			statementId: d.statementId,
			attempt: d.attempt,
			message: d.message
		}));
	return { events, statements, output: output.sort((a, b) => a.at.localeCompare(b.at)) };
}

export function liveness(record: RunRecord, now: number): RunLiveness | null {
	const { run, script } = record;
	if (!script.nodes.length || !run.launchedAt) return null;
	const base = {
		workerInstance: `worker-${run.id.slice(-4)}`,
		containerId: `c${(hash(run.id) % 0xffffffff).toString(16).padStart(8, '0')}`,
		unresponsiveAfterSeconds: 45,
		presumedFailedAfterSeconds: 600,
		startDeadlineSeconds: 120
	};
	const heartbeat = script.lastHeartbeatAt ?? (run.endedAt ? Date.parse(run.endedAt) - 1000 : now - 4000);
	if (!run.startedAt) {
		if (isTerminal(run.status)) return { ...base, state: 'never_started', lastHeartbeatAt: null, exitCode: null };
		return { ...base, state: 'healthy', lastHeartbeatAt: null, exitCode: null };
	}
	const exitCode = invocations(record)[0]?.exitCode ?? null;
	if (run.status === 'presumed_failed') return { ...base, state: 'presumed_failed', lastHeartbeatAt: iso(heartbeat), exitCode };
	if (run.status === 'unresponsive') return { ...base, state: 'unresponsive', lastHeartbeatAt: iso(heartbeat), exitCode: null };
	if (isTerminal(run.status)) return { ...base, state: 'exited', lastHeartbeatAt: iso(heartbeat), exitCode };
	return { ...base, state: 'healthy', lastHeartbeatAt: iso(heartbeat), exitCode: null };
}
