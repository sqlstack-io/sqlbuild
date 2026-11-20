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
import { NODE_SPECS } from './build-fixtures';
import { BIG_NODE, OUTPUT_SOURCES, V2_NODE, V2_TRACEBACK, bigAvailable } from './big-output';

/** The prototype runs (CHI-1591), run by a local worker process and printing sqb's own lines. */
const PROTO = /^run-p(f31|r02|s07|g40)/;
/** Rows a prototype node writes per second while it builds, for its progress. */
const protoRate = (key: string) => 37 + (hash(key) % 400);
/**
 * Honest progress (CHI-1591): only loaders and microbatch models know their rows while they run; a
 * warehouse statement reports none until it finishes, so other nodes show elapsed time only.
 */
const reportsRowsWhileRunning = (key: string) => snapshotNode(key)?.kind === 'loader' || !!NODE_SPECS[key]?.microbatch;
/** Rows a finished node reports: added rows for incrementals and loaders, the table's count from metadata after a rebuild, none for views and tasks. */
const finishedRows = (key: string, ms: number): number | null => {
	const kind = snapshotNode(key)?.kind;
	if (kind === 'view' || kind === 'task') return null;
	if (kind === 'incremental' || kind === 'loader') return Math.round((ms / 1000) * protoRate(key));
	return 800 + (hash(`${key}/rows`) % 60_000);
};

/** `crashed`: the worker stopped before building anything, as when its plan is refused; `crashError` says why. */
export type RunEnd = 'complete' | 'cancelled' | 'presumed_failed' | 'never_started' | 'crashed';

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
	crashError?: string;
	cancel: CancelProgress | null;
	/** Last heartbeat; node progress stops here for unresponsive and lost runs. */
	lastHeartbeatAt: number | null;
	imageId: string;
	snapshotGeneration: number;
	/** Each pool the run takes, with how many of its slots. */
	pools: Record<string, number>;
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

	if (terminal && script.end === 'crashed') return sortTopologically(script.nodes).map((k) => ({ ...base(k, 'skipped'), endedAt: run.endedAt }));

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
		const python = fails && key === V2_NODE;
			return {
				...base(key, status),
				startedAt: iso(s),
				endedAt: iso(e),
				rowsAffected: (fails && !CHECK_FAILS.has(key)) || snapshotNode(key)?.kind === 'view' ? null : PROTO.test(run.id) ? finishedRows(key, e - s) : 120 + (hash(key + run.id) % 48000),
				errorCode: fails ? (CHECK_FAILS.has(key) ? 'check_failed' : python ? 'python_exception' : 'E2103') : null,
				errorMessage: message,
				...(fails ? (CHECK_FAILS.has(key) ? checkFailureDetail(key) : python ? pythonFailureDetail() : failureDetail(key, run, message ?? '')) : {})
			};
		};
		if (terminal && script.end === 'complete') return finish();
		if (e <= clock) return finish();
		if (s <= clock) {
			// A prototype run reports rows so far while a node builds, for the timeline bar and the drawer.
			if (!terminal) return { ...base(key, 'running'), startedAt: iso(s), rowsAffected: PROTO.test(run.id) && reportsRowsWhileRunning(key) ? Math.round(((clock - s) / 1000) * protoRate(key)) : null };
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

// ---- several checks on one node (CHI-1591) ----

/** Checks that fail or warn on a node that warns or fails by a check: severity and the rows they found. */
const CHECK_PROFILE: Record<string, Record<string, readonly ['warn' | 'error', number]>> = {
	stg_inventory: { 'not_null(product_id)': ['warn', 3] },
	fct_shipments: { 'not_null(shipment_id)': ['warn', 4] },
	fct_payment_daily: { 'not_null(payment_id)': ['warn', 2] },
	fct_order_lines: { 'not_null(payment_id)': ['warn', 2], 'unique(order_id)': ['warn', 6] },
	fct_refunds: { 'unique(order_id)': ['error', 14], 'not_null(payment_id)': ['warn', 2], 'accepted_values(status)': ['warn', 5], 'not_null(reason)': ['warn', 9] }
};
/** Nodes that built but failed on an error-severity check. */
const CHECK_FAILS = new Set(['fct_refunds']);

export const auditStatementId = (run: Run, nodeKey: string, i: number) => `${statementIdOf(run, nodeKey)}-audit-${i}`;

/** A node's checks, as `name(column)`, in declaration order. */
export function auditsOf(key: string): string[] {
	return (NODE_SPECS[key]?.columns ?? []).flatMap((c) => (c.audits ?? []).map((a) => `${a}(${c.name})`));
}

/** A check's query as it ran, compiled. */
export function auditSql(nodeKey: string, statementId: string, target: string): string {
	const i = Number(statementId.split('-audit-')[1] ?? 0);
	const audit = auditsOf(nodeKey)[i] ?? 'not_null(id)';
	const [, name, col] = audit.match(/^(\w+)\((\w+)\)$/) ?? [];
	const rel = `${target === 'dev' ? 'dev_demo' : 'analytics'}.${nodeKey}`;
	if (name === 'unique') return `select ${col}, count(*) as n\nfrom ${rel}\nwhere ${col} is not null\ngroup by ${col}\nhaving count(*) > 1`;
	if (name === 'accepted_values') return `select ${col}, count(*) as n\nfrom ${rel}\nwhere ${col} not in ('paid', 'pending', 'refunded', 'void')\ngroup by ${col}`;
	return `select *\nfrom ${rel}\nwhere ${col} is null`;
}

/** The failing rows a check found, as a small sample (the server keeps the first 100). */
export function auditSample(nodeKey: string, statementId: string, count: number): { columns: string[]; rows: (string | number | null)[][]; total: number } {
	const i = Number(statementId.split('-audit-')[1] ?? 0);
	const audit = auditsOf(nodeKey)[i] ?? 'not_null(id)';
	const [, name, col] = audit.match(/^(\w+)\((\w+)\)$/) ?? [];
	const n = Math.min(count, 8);
	const id = (j: number) => 10_400 + ((hash(`${statementId}${j}`) % 9_000) | 0);
	if (name === 'unique') return { columns: [col, 'n'], rows: Array.from({ length: n }, (_, j) => [id(j), 2 + (j % 2)]), total: count };
	if (name === 'accepted_values') return { columns: [col, 'n'], rows: ['paid ', 'PAID', 'refund', 'unknown', 'n/a'].slice(0, n).map((v, j) => [v, 1 + j]), total: count };
	return { columns: ['order_id', col, 'created_at'], rows: Array.from({ length: n }, (_, j) => [id(j), null, `2026-11-${String(10 + j).padStart(2, '0')} 0${j}:14:0${j}`]), total: count };
}

/** A Python node's exception: where it was raised in the node's own code, and the whole traceback. */
function pythonFailureDetail(): Pick<RunNode, 'errorLocation' | 'errorDetail'> {
	return { errorLocation: { file: 'models/ml/score_customers_v2.py', line: 26 }, errorDetail: V2_TRACEBACK.join('\n') };
}

function checkFailureDetail(key: string): Pick<RunNode, 'errorLocation' | 'errorDetail'> {
	const file = (snapshotNode(key)?.path ?? `models/${key}.sql`).replace(/\.sql$/, '.yml');
	return {
		errorLocation: { file, line: 12 },
		errorDetail: ['check unique(order_id) failed: 14 duplicate rows (severity error)', `  --> ${file}:12`, '', 'The table was built, then its error-severity check failed, so the node failed and its downstream was skipped.'].join('\n')
	};
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
	const c: NodeCounts = { success: 0, warn: 0, failed: 0, skipped: 0, running: 0, pending: 0, notRun: 0 };
	for (const n of nodes) {
		if (n.status === 'success') c.success++;
		else if (n.status === 'warn') c.warn++;
		else if (n.status === 'failed') c.failed++;
		else if (n.status === 'skipped' || n.status === 'cancelled' || n.status === 'not_run') {
			c.skipped++;
			if (n.status !== 'skipped') c.notRun!++;
		}
		else if (n.status === 'running') c.running++;
		else c.pending++;
	}
	return c;
}

/** Terminal status from node outcomes: partial when something failed and everything else completed. */
export function deriveTerminalStatus(end: RunEnd, nodes: RunNode[]): RunStatus {
	if (end === 'never_started' || end === 'crashed') return 'failed';
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
	node_log: 'info',
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
	terminated: 'info',
	member_retrying: 'warn',
	member_recovered: 'info'
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
	const out = (stream: RunOutputLine['stream'], at: number, text: string, error = false, nodeKey: string | null = null, omitted?: number) =>
		inv && output.push({ invocationId: inv, stream, at: iso(at), text, error, ...(nodeKey ? { nodeKey } : {}), ...(omitted ? { omitted } : {}) });

	const queued = Date.parse(run.queuedAt);
	fact('requested', queued - 200, `Requested by ${run.requestedBy.displayName}${run.triggerName ? ` via ${run.triggerName}` : ''}`);
	fact('queued', queued, `Queued at priority ${run.priority}`);
	if (run.launchedAt) {
		const launched = Date.parse(run.launchedAt);
		fact('claimed', launched - 300, script.nodes.length ? `Claimed ${script.nodes.length} write-set nodes${Object.keys(script.pools).length ? ` and slots in pool ${Object.keys(script.pools).join(', ')}` : ''}` : 'Claimed');
		fact('launched', launched, PROTO.test(run.id) ? (script.nodes.length ? 'Worker started (local process)' : `Started ${run.childRunCount} member job runs in order`) : script.nodes.length ? `Container started from image ${script.imageId.slice(0, 19)}` : `Started ${run.childRunCount} child runs in order`);
	}
	if (script.cancel) fact('cancel_requested', script.cancel.requestedAt, `Cancel requested by ${script.cancel.requestedBy.displayName}`);

	if (inv && run.startedAt && PROTO.test(run.id)) {
		protoExecution(record, nodes, now ?? Date.now(), { ev, out, statements, inv });
	} else if (inv && run.startedAt) {
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
		if (run.endedAt && script.crashError) out('stderr', Date.parse(run.endedAt) - 500, script.crashError, true);
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
			level:
				d.type === 'audit_completed' && /\(severity error\)$/.test(d.message)
					? 'error'
					: (d.type === 'audit_completed' && /\(severity warn\)$/.test(d.message)) || (d.type === 'node_log' && d.message.startsWith('[warning] '))
						? 'warn'
						: LEVEL[d.type],
			invocationId: d.invocationId,
			nodeKey: d.nodeKey,
			statementId: d.statementId,
			attempt: d.attempt,
			message: d.message
		}));
	return { events, statements, output: output.sort((a, b) => a.at.localeCompare(b.at)) };
}

type Emit = {
	ev: (type: RunEvent['type'], at: number, nodeKey: string | null, message: string, statementId?: string | null) => void;
	out: (stream: RunOutputLine['stream'], at: number, text: string, error?: boolean, nodeKey?: string | null, omitted?: number) => void;
	statements: RunStatement[];
	inv: string;
};

/** What a prototype Python node prints while it runs: its own stdout, and a warning with a traceback on stderr. */
function pythonOutput(key: string, s: number, e: number | null, now: number, out: Emit['out']) {
	const until = e ?? now;
	const at = (ms: number) => s + ms;
	const line = (ms: number, stream: RunOutputLine['stream'], text: string) => at(ms) < until && out(stream, at(ms), text, false, key);
	if (key === 'raw_events') {
		line(400, 'stdout', 'Fetching events since 2026-11-20T09:00:00Z');
		for (let page = 1, t = 6_000; t < (e ?? now) - s; page++, t += 9_000) {
			line(t, 'stdout', `Page ${page}: ${(11_000 + (hash(`${key}${page}`) % 2_000)).toLocaleString('en-GB')} events`);
			if (page === 3)
				line(
					t + 2_500,
					'stderr',
					[
						'WARNING shop.events: page 4 failed, retrying in 5s',
						'Traceback (most recent call last):',
						'  File "/app/models/ingest/raw_events.py", line 18, in raw_events',
						'    rows = fetch_page(since=ctx.current_cursor_value, page=page)',
						'  File "/app/shop/events_api.py", line 41, in fetch_page',
						'    resp.raise_for_status()',
						'requests.exceptions.HTTPError: 503 Server Error: Service Unavailable for url: https://events.example.com/v1/events?page=4'
					].join('\n')
				);
		}
	}
	const src = OUTPUT_SOURCES[key];
	if (src) {
		// The live view is capped (CHI-1597): a marker for what came before, then the latest lines; the rest is in the viewer.
		const n = { nodeKey: key, startedAt: new Date(s).toISOString(), status: e == null ? 'running' : 'success' } as RunNode;
		const big = bigAvailable(n, until);
		if (!big) return;
		// Capped only past about 1,000 lines: then the last 40, and the rest in the viewer.
		const LIVE = big.available > 1_000 ? 40 : big.available;
		const first = Math.max(1, big.available - LIVE + 1);
		if (first > 1) out('stdout', until - 1_000, `${(first - 1).toLocaleString('en-GB')} earlier lines`, false, key, first - 1);
		for (let i = first; i <= big.available; i++) {
			const l = src.line(i);
			out(l.stream, until - 1_000 + (i - first) * 20, l.text, false, key);
		}
		return;
	}
	if (key === 'score_customers') {
		line(500, 'stdout', 'Loaded 4,812 customers');
		line(
			1_400,
			'stderr',
			[
				'/app/models/ml/score_customers.py:11: FutureWarning: DataFrame.fillna with \'method\' is deprecated and will raise in a future version. Use obj.ffill() or obj.bfill() instead.',
				'  features = features.fillna(method="ffill")'
			].join('\n')
		);
		if (e != null) line(e - s - 300, 'stdout', 'Scored 4,812 customers');
	}
}

/** The heartbeat's backoff: 30 s, then 1 min, 2 min, and every 5 min after that. */
function heartbeatTimes(start: number, until: number): number[] {
	const gaps = [30_000, 60_000, 120_000];
	const out: number[] = [];
	for (let t = start, i = 0; ; i++) {
		t += gaps[i] ?? 300_000;
		if (t >= until) return out;
		out.push(t);
	}
}

/**
 * A prototype run's events and what sqb printed. Each check is its own event (the page folds them into
 * their node's line); sqb prints a progress line for a building node every 30 seconds, and every line
 * it prints for a node carries that node.
 */
function protoExecution(record: RunRecord, nodes: RunNode[], now: number, { ev, out, statements, inv }: Emit) {
	const { run, script } = record;
	const start = Date.parse(run.startedAt!);
	const secs = (ms: number) => `${(ms / 1000).toFixed(1)}s`;
	const rows = (n: number) => n.toLocaleString('en-GB');
	ev('invocation_started', start, null, commandOf(record));
	// sqb prints its progress on stdout, and warnings and errors on stderr.
	out('stdout', start + 120, `sqb ${commandOf(record).replace(/^sqb /, '')}`);
	out('stdout', start + 450, `✓ Project compiled  ${script.nodes.length} models  0.4s`);
	out('stdout', start + 900, '✓ Warehouse connected  snowflake  0.1s');
	out('stdout', start + 1100, `Building ${script.nodes.length} nodes, 4 at a time`);
	const audits = auditsOf;
	for (const n of nodes) {
		const k = snapshotNode(n.nodeKey)?.kind ?? 'table';
		const key = n.nodeKey;
		if (n.status === 'skipped' && n.endedAt) {
			const why = n.skippedBecauseOf;
			// Every skip names its cause, chain included: the skipped parent it waited on, and the failure behind it.
			const via = why ? nodes.find((p) => p.status === 'skipped' && p.skippedBecauseOf?.nodeKey === why.nodeKey && (snapshotNode(key)?.upstream ?? []).includes(p.nodeKey)) : undefined;
			const across = why && why.runId !== run.id ? ` in ${why.jobKey ?? why.runId}` : '';
			const cause = !why ? 'an upstream node did not run' : via ? `${via.nodeKey} was skipped (after ${why.nodeKey} failed${across})` : `${why.nodeKey} failed${across}`;
			ev('resource_attempt_skipped', Date.parse(n.endedAt), key, `Skipped: ${cause}`);
			out('stdout', Date.parse(n.endedAt), `  – ${key}  skipped: ${cause}`, false, key);
			continue;
		}
		if (!n.startedAt) continue;
		const s = Date.parse(n.startedAt);
		const e = n.endedAt ? Date.parse(n.endedAt) : null;
		ev('resource_attempt_started', s, key, `Started ${k} ${key}`);
		out('stdout', s, `  ▸ ${key}  ${k}`, false, key);
		pythonOutput(key, s, e, now, out);
		if (key === BIG_NODE) {
			// Its ctx.log lines: few and deliberate, kept as events (CHI-1597).
			ev('node_log', s + 1_500, key, 'Running notebook churn_features.ipynb (48 cells)');
			if ((e ?? now) - s > 3_000) ev('node_log', s + ((e ?? now) - s) / 2, key, '[warning] cell 18 retried after worker loss');
			if (e != null) ev('node_log', e - 200, key, 'Notebook finished: 48 cells, 211,604 rows written');
		}
		const kind = STATEMENT_KIND[k];
		if (kind) {
			// The node's warehouse statement, with its query ID, for the Statements tab, the cost and the drawer.
			const statementId = statementIdOf(run, key);
			const stEnd = e != null ? e - 120 : null;
			// A node failed by a check built fine: its build query completed.
			const failed = n.status === 'failed' && !CHECK_FAILS.has(key);
			statements.push({
				id: statementId,
				runId: run.id,
				invocationId: inv,
				sequence: statements.length + 1,
				nodeKey: key,
				kind,
				status: n.status === 'running' ? 'running' : failed ? 'failed' : 'completed',
				startedAt: iso(s + 60),
				endedAt: stEnd == null ? null : iso(stEnd),
				rowsAffected: n.status === 'running' ? null : n.rowsAffected,
				rowsSoFar: n.status === 'running' && reportsRowsWhileRunning(key) ? n.rowsAffected : null,
				queryId: queryIdOf(statementId),
				sqlRecorded: true,
				error: failed ? (n.errorDetail ?? n.errorMessage) : null
			});
			ev('statement_submitted', s + 40, key, `${kind} submitted`, statementId);
			ev('statement_started', s + 60, key, `${kind} running`, statementId);
			if (stEnd != null) {
				if (failed) ev('statement_failed', stEnd, key, n.errorMessage ?? 'Statement failed', statementId);
				else ev('statement_completed', stEnd, key, n.rowsAffected != null ? `${rows(n.rowsAffected)} rows` : 'Completed', statementId);
			}
		}
		if (e == null) continue;
		const took = secs(e - s);
		const checkFailed = CHECK_FAILS.has(key) && n.status === 'failed';
		if (n.status === 'failed' && !checkFailed) {
			ev('resource_attempt_failed', e, key, `${n.errorCode}: ${n.errorMessage}`);
			out('stderr', e, `  ✗ ${key}  ${k}  ${took}  ${n.errorMessage}`, true, key);
			continue;
		}
		const added = k === 'incremental' && n.rowsAffected != null ? `  +${rows(n.rowsAffected)} rows` : n.rowsAffected != null ? `  ${rows(n.rowsAffected)} rows` : '';
		if (!checkFailed) ev('resource_attempt_completed', e, key, `${n.status === 'warn' ? 'Completed with warnings' : 'Completed'} in ${took}${added ? ` ·${added.replace(/^ +/, ' ')}` : ''}`);
		out('stdout', e, `  ✓ ${key}  ${k}  ${took}${added}`, false, key);
		// Each check is its own query: an event with its statement, so its SQL, query ID and failing rows can be read.
		const profile = n.status === 'warn' || checkFailed ? (CHECK_PROFILE[key] ?? null) : null;
		audits(key).forEach((audit, i) => {
			const problem = profile ? profile[audit] : n.status === 'warn' && i === 0 ? (['warn', 2 + (hash(key) % 5)] as const) : undefined;
			const at = e + 40 + i * 120;
			const statementId = auditStatementId(run, key, i);
			statements.push({
				id: statementId,
				runId: run.id,
				invocationId: inv,
				sequence: statements.length + 1,
				nodeKey: key,
				kind: 'audit',
				status: 'completed',
				startedAt: iso(at - 90 - (hash(statementId) % 300)),
				endedAt: iso(at),
				rowsAffected: problem ? problem[1] : 0,
				rowsSoFar: null,
				queryId: queryIdOf(statementId),
				sqlRecorded: true,
				error: null
			});
			const noun = audit.startsWith('unique') ? 'duplicate rows' : 'failing rows';
			ev('audit_completed', at, key, problem ? `${audit}: ${problem[1]} ${noun} (severity ${problem[0]})` : `${audit} passed`, statementId);
			if (problem) out('stderr', at, `    ${problem[0] === 'error' ? '✗' : '!'} ${key}  check ${audit}: ${problem[1]} ${noun} (${problem[0]})`, problem[0] === 'error', key);
		});
		if (checkFailed) {
			const at = e + 60 + audits(key).length * 120;
			ev('resource_attempt_failed', at, key, `check_failed: ${n.errorMessage}`);
			out('stderr', at, `  ✗ ${key}  ${k}  ${took}  ${n.errorMessage}`, true, key);
		}
	}
	// One combined heartbeat line for whatever is still running, backing off: 30 s, 1 min, 2 min, then every 5 min.
	const firstStart = Math.min(...nodes.map((n) => (n.startedAt ? Date.parse(n.startedAt) : Infinity)));
	if (Number.isFinite(firstStart)) {
		const until = run.endedAt ? Date.parse(run.endedAt) : now;
		for (const t of heartbeatTimes(firstStart, until)) {
			const busy = nodes.filter((n) => n.startedAt && Date.parse(n.startedAt) <= t && (!n.endedAt || Date.parse(n.endedAt) > t));
			if (!busy.length) continue;
			const span = (ms: number) => (ms >= 60_000 ? `${Math.floor(ms / 60_000)}m${String(Math.floor((ms % 60_000) / 1000)).padStart(2, '0')}s` : `${Math.floor(ms / 1000)}s`);
			const parts = busy.map((n) => {
				const ms = t - Date.parse(n.startedAt!);
				return `${n.nodeKey} ${span(ms)}${reportsRowsWhileRunning(n.nodeKey) ? ` (${rows(Math.round((ms / 1000) * protoRate(n.nodeKey)))} rows)` : ''}`;
			});
			out('stdout', t, `… still running (${busy.length})  ${parts.join(' · ')}`);
		}
	}
	if (run.endedAt) {
		const end = Date.parse(run.endedAt) - 400;
		const c = countNodes(nodes);
		const summary = `${c.success + c.warn} built · ${c.failed} failed · ${c.skipped} skipped`;
		const ok = c.failed === 0;
		ev(ok ? 'invocation_completed' : 'invocation_failed', end, null, `${summary} · ${secs(end - start)}`);
		out(ok ? 'stdout' : 'stderr', end, `${ok ? '✓ Completed' : '✗ Completed with errors'}  ${summary}  ${secs(end - start)}`);
	}
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
