// Plain one-line summaries of run events; the raw type stays available as a tooltip.

import type { RunEvent, RunEventLevel, RunNode, RunOutputLine } from '$lib/api';
import { eventTone } from '$lib/operate/labels';
import type { EventCategory } from '../types';

export const EVENT_CATEGORIES: { value: EventCategory; label: string }[] = [
	{ value: 'run', label: 'Run' },
	{ value: 'nodes', label: 'Nodes' },
	{ value: 'queries', label: 'Queries' },
	{ value: 'output', label: 'Output' }
];

export function eventCategory(e: RunEvent): EventCategory {
	if (e.type.startsWith('statement_')) return 'queries';
	if (e.type.startsWith('resource_attempt_')) return 'nodes';
	if (e.type === 'audit_completed') return 'checks';
	return 'run';
}

export function eventSummary(e: RunEvent): string {
	switch (e.type) {
		case 'requested':
			return e.message;
		case 'queued':
			return 'Queued';
		case 'claimed':
			return 'Claimed write lock';
		case 'launched':
			// What really started it: a local worker process, or a container (CHI-1591).
			return e.message.startsWith('Worker started') || e.message.startsWith('Started ') ? e.message : 'Container started';
		case 'cancel_requested':
			return e.message;
		case 'presumed_failed':
			return 'Lost contact · failed';
		case 'terminated':
			// A run that never started says so, in the same words everywhere (CHI-1591).
			if (/^Terminated: skipped/.test(e.message)) return 'Not started';
			return `Finished${e.message.match(/^Terminated: (\w+)/) ? `: ${e.message.match(/^Terminated: (\w+)/)![1].replace('_', ' ')}` : ''}`;
		case 'invocation_started':
			return 'sqb started';
		case 'invocation_completed':
			return `sqb finished: ${e.message}`;
		case 'invocation_failed':
			return `sqb finished with failures: ${e.message}`;
		case 'resource_attempt_started':
			return 'Started';
		case 'resource_attempt_completed':
			return e.message;
		case 'resource_attempt_failed':
			return e.message.replace(/^[A-Za-z0-9_]+: /, 'Failed: ');
		case 'resource_attempt_skipped':
			return e.message;
		case 'statement_submitted':
			return 'Query sent to the warehouse';
		case 'statement_started':
			return 'Query running';
		case 'statement_completed':
			return `Query finished · ${e.message}`;
		case 'statement_failed':
			return `Query failed: ${e.message}`;
		case 'audit_completed':
			return `Checks: ${e.message}`;
		case 'member_retrying':
		case 'member_recovered':
			return e.message;
	}
}

/** One row of the run log: an event, or a block the run printed. */
export type LogEntry = {
	id: string;
	runId: string;
	at: string;
	nodeKey: string | null;
	level: RunEventLevel;
	category: EventCategory;
	text: string;
	/** The raw event type and message, for the tooltip. */
	title: string;
	tone: string;
	stream: 'stdout' | 'stderr' | null;
	/** Query rows link to their statement's SQL. */
	statementId: string | null;
	/** A node's finished queries, folded under its outcome row. */
	queries: LogEntry[];
	/** Shown even with printed output hidden: a failing run's last printed block, which ends with its failures. */
	pinned: boolean;
};

const QUERY_DONE = new Set(['statement_completed', 'statement_failed']);
const NODE_ENDS = new Set(['resource_attempt_completed', 'resource_attempt_failed', 'resource_attempt_skipped']);

function entryOf(e: RunEvent): LogEntry {
	const text = eventSummary(e);
	return {
		id: e.id,
		runId: e.runId,
		at: e.occurredAt,
		nodeKey: e.nodeKey,
		level: e.level,
		category: eventCategory(e),
		text,
		title: `${e.producer === 'scheduler' ? 'Scheduler' : 'sqb'} event ${e.type}${e.message !== text ? `\n${e.message}` : ''}`,
		tone: eventTone(e),
		stream: null,
		statementId: e.statementId,
		queries: [],
		pinned: false
	};
}

/**
 * The run's story in time order: run and node milestones, checks, failures, and printed output.
 * A node's finished queries fold under its outcome row, and its start row goes once it has one;
 * a failed query keeps its own row. Run-level bookkeeping queries and query progress are left to
 * the Statements tab.
 */
export function logEntries(events: readonly RunEvent[], output: readonly (RunOutputLine & { runId: string })[], outcomes?: NodeOutcomes): LogEntry[] {
	if (outcomes) return withOutcomes(events, output, outcomes);
	const nodeOf = (e: RunEvent) => `${e.runId}|${e.nodeKey}`;
	const queries = new Map<string, LogEntry[]>();
	const ended = new Set<string>();
	for (const e of events) {
		if (!e.nodeKey) continue;
		if (QUERY_DONE.has(e.type)) queries.set(nodeOf(e), [...(queries.get(nodeOf(e)) ?? []), entryOf(e)]);
		if (NODE_ENDS.has(e.type)) ended.add(nodeOf(e));
	}

	const fromEvents = events.flatMap((e): LogEntry[] => {
		if (e.type.startsWith('statement_')) return e.type === 'statement_failed' ? [entryOf(e)] : [];
		if (e.type === 'resource_attempt_started' && ended.has(nodeOf(e))) return [];
		const entry = entryOf(e);
		if (!NODE_ENDS.has(e.type)) return [entry];
		const text = e.type === 'resource_attempt_completed' ? ['Finished', entry.text].filter(Boolean).join(' · ') : entry.text;
		return [{ ...entry, text, queries: queries.get(nodeOf(e)) ?? [] }];
	});

	const failing = new Set(fromEvents.filter((e) => e.level === 'error').map((e) => e.runId));
	const lastStderr = new Map<string, number>();
	output.forEach((l, i) => l.stream === 'stderr' && failing.has(l.runId) && lastStderr.set(l.runId, i));
	const fromOutput = output.map(
		(l, i): LogEntry => ({
			id: `out-${l.runId}-${i}`,
			runId: l.runId,
			at: l.at,
			nodeKey: null,
			level: l.error ? 'error' : 'info',
			category: 'output',
			text: l.text,
			title: l.stream,
			tone: l.error ? 'var(--sb-error)' : l.stream === 'stderr' ? 'var(--sb-warning)' : 'var(--sb-text-faint)',
			stream: l.stream,
			statementId: null,
			queries: [],
			pinned: l.error || lastStderr.get(l.runId) === i
		})
	);
	// A stable sort keeps each source's own order for equal times.
	return [...fromEvents, ...fromOutput].sort((a, b) => a.at.localeCompare(b.at));
}

// ---- one line per node outcome (CHI-1591) ----

export type NodeOutcomes = {
	nodes: readonly RunNode[];
	/** Why a skipped node was skipped, chain included, as the timeline says it. */
	skipCause: (n: RunNode) => string | null;
	now: number;
};

const rowsText = (n: number) => n.toLocaleString('en-GB');
const secsText = (ms: number) => (ms < 60_000 ? `${(ms / 1000).toFixed(1)}s` : `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`);
const BUILT: Record<string, string> = { table: 'rebuilt table', view: 'rebuilt view', incremental: 'incremental merge', seed: 'loaded seed', loader: 'loaded', task: 'ran task' };
const BUILDING: Record<string, string> = { table: 'building table', view: 'building view', incremental: 'incremental merge', seed: 'loading seed', loader: 'loading', task: 'running task' };

/** A check result as the node's line names it: `check unique(sku) warned: 3 failing rows`. */
function checkText(e: RunEvent): string {
	const m = e.message.match(/^(.*?): (.*?)(?: \(severity \w+\))?$/);
	return `check ${m?.[1] ?? e.message} ${e.level === 'error' ? 'failed' : 'warned'}${m ? `: ${m[2]}` : ''}`;
}

/** What a node did, on one line: its outcome, rows, time and checks, with a warning or failed check named on it. */
export function outcomeText(n: RunNode, checks: readonly RunEvent[], skip: string | null, now: number): { text: string; level: RunEventLevel; tone: string } {
	const took = n.startedAt ? secsText((n.endedAt ? Date.parse(n.endedAt) : now) - Date.parse(n.startedAt)) : null;
	const rows = n.rowsAffected == null ? null : `${n.kind === 'incremental' ? '+' : ''}${rowsText(n.rowsAffected)} rows`;
	const join = (parts: (string | null | false)[]) => parts.filter(Boolean).join(' · ');
	switch (n.status) {
		case 'success':
		case 'warn': {
			const bad = checks.filter((c) => c.level !== 'info');
			const passed = checks.length - bad.length;
			const checkPart = bad.length ? bad.map(checkText).join('; ') : passed ? `${passed} ${passed === 1 ? 'check' : 'checks'} passed` : null;
			const warned = n.status === 'warn' || bad.length > 0;
			return { text: join([`${warned ? '⚠' : '✓'} ${BUILT[n.kind ?? 'table'] ?? `built ${n.kind}`}`, rows, took, checkPart]), level: warned ? 'warn' : 'info', tone: warned ? 'var(--sb-warning)' : 'var(--sb-success)' };
		}
		case 'failed': {
			const where = n.errorLocation ? (n.errorLocation.line == null ? n.errorLocation.file : `${n.errorLocation.file}:${n.errorLocation.line}`) : null;
			return { text: join(['✗ failed', n.errorMessage ?? n.errorCode, where]), level: 'error', tone: 'var(--sb-error)' };
		}
		case 'skipped':
			return { text: join(['⊘ skipped', skip?.replace(/^skipped: /, '') ?? null]), level: 'info', tone: 'var(--sb-text-faint)' };
		case 'running':
			return { text: join([`▸ ${BUILDING[n.kind ?? 'table'] ?? `building ${n.kind}`}`, rows && `${rows} so far`, took && `${took} so far`]), level: 'info', tone: 'var(--sb-running)' };
		case 'pending':
			return { text: '· waiting', level: 'info', tone: 'var(--sb-text-faint)' };
		default:
			return { text: '— not run', level: 'info', tone: 'var(--sb-text-faint)' };
	}
}

/**
 * The log with one line per node: where the node started, saying what it did once done (and what it is
 * doing while it builds), with its checks folded in. Run-level events stay as they are; what sqb printed
 * for a node carries the node, so filtering by node keeps it.
 */
function withOutcomes(events: readonly RunEvent[], output: readonly (RunOutputLine & { runId: string })[], { nodes, skipCause, now }: NodeOutcomes): LogEntry[] {
	const id = (runId: string, key: string) => `${runId}|${key}`;
	const known = new Map(nodes.map((n) => [id(n.runId, n.nodeKey), n]));
	const byNode = new Map<string, RunEvent[]>();
	for (const e of events) if (e.nodeKey && known.has(id(e.runId, e.nodeKey))) byNode.set(id(e.runId, e.nodeKey), [...(byNode.get(id(e.runId, e.nodeKey)) ?? []), e]);

	const runLines = events.filter((e) => !(e.nodeKey && known.has(id(e.runId, e.nodeKey))) && !e.type.startsWith('statement_')).map(entryOf);
	const failedQueries = events.filter((e) => e.type === 'statement_failed').map(entryOf);
	const nodeLines = nodes.flatMap((n): LogEntry[] => {
		const evs = byNode.get(id(n.runId, n.nodeKey)) ?? [];
		const at = n.startedAt ?? n.endedAt ?? evs[0]?.occurredAt;
		if (!at || n.status === 'pending') return [];
		const checks = evs.filter((e) => e.type === 'audit_completed');
		const o = outcomeText(n, checks, skipCause(n), now);
		return [
			{
				id: `node-${n.runId}-${n.nodeKey}`,
				runId: n.runId,
				at,
				nodeKey: n.nodeKey,
				level: o.level,
				category: 'nodes',
				text: o.text,
				title: evs.map((e) => `${e.type}: ${e.message}`).join('\n'),
				tone: o.tone,
				stream: null,
				statementId: null,
				queries: evs.filter((e) => QUERY_DONE.has(e.type)).map(entryOf),
				pinned: false
			}
		];
	});
	const failing = new Set(nodes.filter((n) => n.status === 'failed').map((n) => n.runId));
	const lastStderr = new Map<string, number>();
	output.forEach((l, i) => l.stream === 'stderr' && failing.has(l.runId) && lastStderr.set(l.runId, i));
	const fromOutput = output.map(
		(l, i): LogEntry => ({
			id: `out-${l.runId}-${i}`,
			runId: l.runId,
			at: l.at,
			nodeKey: l.nodeKey ?? null,
			level: l.error ? 'error' : 'info',
			category: 'output',
			text: l.text,
			title: l.stream,
			tone: l.error ? 'var(--sb-error)' : l.stream === 'stderr' ? 'var(--sb-warning)' : 'var(--sb-text-faint)',
			stream: l.stream,
			statementId: null,
			queries: [],
			pinned: l.error || lastStderr.get(l.runId) === i
		})
	);
	return [...runLines, ...nodeLines, ...failedQueries, ...fromOutput].sort((a, b) => a.at.localeCompare(b.at));
}
