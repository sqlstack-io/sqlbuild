// Plain one-line summaries of run events; the raw type stays available as a tooltip.

import type { RunEvent, RunEventLevel, RunNode, RunOutputLine } from '$lib/api';
import { eventTone } from '$lib/operate/labels';
import { fullStamp } from './stamps';
import { CHECK_FAILED_CODE } from './failures';
import { checksOf, checksText, worstCheck, type CheckResult } from './checks';
import type { EventCategory } from '../types';

export const EVENT_CATEGORIES: { value: EventCategory; label: string }[] = [
	{ value: 'run', label: 'Run' },
	{ value: 'nodes', label: 'Nodes' },
	{ value: 'queries', label: 'Queries' },
	{ value: 'output', label: 'Output' }
];

export function eventCategory(e: RunEvent): EventCategory {
	if (e.type.startsWith('statement_')) return 'queries';
	if (e.type.startsWith('resource_attempt_') || e.type === 'node_log') return 'nodes';
	if (e.type === 'audit_completed') return 'checks';
	return 'run';
}

export function eventSummary(e: RunEvent): string {
	switch (e.type) {
		case 'requested':
			return e.message;
		case 'node_log':
			// A Python node's `ctx.log` line, as it wrote it.
			return e.message.replace(/^\[warning\] /, '');
		case 'queued':
			return 'Queued';
		case 'claimed':
			return 'Claimed write lock';
		case 'launched':
			return e.message || 'Worker started';
		case 'cancel_requested':
			return e.message;
		case 'presumed_failed':
			return 'Lost contact · failed';
		case 'terminated':
			// A run that never started says so, in the same words everywhere.
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
	/** A short tag in plain words (built, merged, failed, skipped, check, run) and its colour. */
	tag?: { text: string; tone: string };
	/** A node line's node: its failure, file and statements, for what its details open to. */
	node?: RunNode;
	/** A node line's checks, problems first, for the table its details open to. */
	checks?: CheckResult[];
	/** A capped live view's marker: this many earlier lines of the node's output are in the viewer. */
	omitted?: number;
	/** Structured details the row opens to in place, as key and value; `long` is text behind "show more". */
	details?: [string, string][];
	long?: string;
};

const TAG_TONE = { ok: 'var(--sb-success)', err: 'var(--sb-error)', warn: 'var(--sb-warning)', run: 'var(--sb-running)', quiet: 'var(--sb-text-faint)' };
/** The tag an event's row carries: plain words, never the event type. */
function eventTag(e: RunEvent): LogEntry['tag'] {
	if (e.type === 'resource_attempt_completed') return { text: 'built', tone: TAG_TONE.ok };
	if (e.type === 'resource_attempt_failed' || e.type === 'statement_failed' || e.type === 'invocation_failed') return { text: 'failed', tone: TAG_TONE.err };
	if (e.type === 'resource_attempt_skipped') return { text: 'skipped', tone: TAG_TONE.quiet };
	if (e.type === 'audit_completed') return { text: 'check', tone: e.level === 'info' ? TAG_TONE.ok : TAG_TONE.warn };
	if (e.type === 'node_log') return { text: 'log', tone: e.level === 'info' ? TAG_TONE.quiet : TAG_TONE.warn };
	return { text: 'run', tone: TAG_TONE.quiet };
}

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
		pinned: false,
		tag: eventTag(e),
		details: e.message && e.message !== text ? [['Message', e.message]] : undefined
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

// ---- one line per node outcome ----

export type NodeOutcomes = {
	nodes: readonly RunNode[];
	/** Why a skipped node was skipped, chain included, as the timeline says it. */
	skipCause: (n: RunNode) => string | null;
	now: number;
};

/** A node line's tag: built or merged, a warned check, failed, skipped, building or not run. */
/**
 * A node line's tag names its build (built, merged, failed…), coloured by its worst outcome, checks
 * included; never `check`, which is for checks run on their own, as in a `sqb test` run.
 */
function nodeTag(n: RunNode, checks: readonly RunEvent[]): LogEntry['tag'] {
	if (n.status === 'success' || n.status === 'warn') {
		const warned = n.status === 'warn' || checks.some((c) => c.level !== 'info');
		return { text: n.kind === 'incremental' ? 'merged' : 'built', tone: warned ? TAG_TONE.warn : TAG_TONE.ok };
	}
	if (n.status === 'failed') return { text: 'failed', tone: TAG_TONE.err };
	if (n.status === 'skipped') return { text: 'skipped', tone: TAG_TONE.quiet };
	if (n.status === 'running') return { text: 'building', tone: TAG_TONE.run };
	return { text: 'not run', tone: TAG_TONE.quiet };
}

const secsText = (ms: number) => (ms < 60_000 ? `${(ms / 1000).toFixed(1)}s` : `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`);
const BUILT: Record<string, string> = { table: 'rebuilt table', view: 'rebuilt view', incremental: 'incremental merge', seed: 'loaded seed', loader: 'loaded', asset: 'ran asset' };
const BUILDING: Record<string, string> = { table: 'building table', view: 'building view', incremental: 'incremental merge', seed: 'loading seed', loader: 'loading', asset: 'running asset' };
/** Kinds whose rows are what they added rather than what the table now holds. */
const ADDS_ROWS = new Set(['incremental', 'loader']);

/**
 * The rows a node can honestly report: what an incremental merge or a load added, what a table rebuilt in
 * full now holds, or, while it runs, what it has written so far; nothing for a view or an unknown count.
 */
export function rowsText(n: RunNode): string | null {
	const count = (rows: number) => `${rows.toLocaleString('en-GB')} ${rows === 1 ? 'row' : 'rows'}`;
	if (n.status === 'running') return n.rowsSoFar == null ? null : `${count(n.rowsSoFar)} so far`;
	if (n.kind != null && ADDS_ROWS.has(n.kind)) return n.rowsAffected == null ? null : `+${count(n.rowsAffected)}`;
	if (n.tableRows != null) return count(n.tableRows);
	return null;
}


/** What a node did, on one line: its outcome, rows, time and checks, with a warning or failed check named on it. */
export function outcomeText(n: RunNode, checks: readonly RunEvent[], skip: string | null, now: number): { text: string; level: RunEventLevel; tone: string } {
	const took = n.startedAt ? secsText((n.endedAt ? Date.parse(n.endedAt) : now) - Date.parse(n.startedAt)) : null;
	const rows = rowsText(n);
	const join = (parts: (string | null | false)[]) => parts.filter(Boolean).join(' · ');
	switch (n.status) {
		case 'success':
		case 'warn': {
			// One line however many checks: the worst outcome sets its colour.
			const results = checksOf(checks);
			const warned = n.status === 'warn' || worstCheck(results) === 'warned' || worstCheck(results) === 'failed';
			if (n.noChangeReason && !warned) return { text: join(['✓ no change', n.noChangeReason, took, checksText(results)]), level: 'info', tone: 'var(--sb-text-faint)' };
			return { text: join([`${warned ? '⚠' : '✓'} ${BUILT[n.kind ?? 'table'] ?? `built ${n.kind}`}`, rows, took, checksText(results)]), level: warned ? 'warn' : 'info', tone: warned ? 'var(--sb-warning)' : 'var(--sb-success)' };
		}
		case 'failed': {
			// Built, then failed by an error-severity check: the build's line, with its checks.
			if (n.errorCode === CHECK_FAILED_CODE) return { text: join([`✗ ${BUILT[n.kind ?? 'table'] ?? `built ${n.kind}`}`, rows, took, checksText(checksOf(checks))]), level: 'error', tone: 'var(--sb-error)' };
			const where = n.errorLocation ? (n.errorLocation.line == null ? n.errorLocation.file : `${n.errorLocation.file}:${n.errorLocation.line}`) : null;
			return { text: join(['✗ failed', n.errorMessage ?? n.errorCode, where]), level: 'error', tone: 'var(--sb-error)' };
		}
		case 'skipped':
			return { text: join(['⊘ skipped', skip?.replace(/^skipped: /, '') ?? null]), level: 'info', tone: 'var(--sb-text-faint)' };
		case 'running':
			return { text: join([`▸ ${BUILDING[n.kind ?? 'table'] ?? `building ${n.kind}`}`, rows, took && `${took} so far`]), level: 'info', tone: 'var(--sb-running)' };
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

	// Run-level events, and a node's `ctx.log` lines, which keep their own rows.
	const runLines = events.filter((e) => (e.type === 'node_log' || !(e.nodeKey && known.has(id(e.runId, e.nodeKey)))) && !e.type.startsWith('statement_')).map(entryOf);
	// One failure, one line: a failed query of a node folds into the node's line, with its query in place.
	const failedQueries = events.filter((e) => e.type === 'statement_failed' && !(e.nodeKey && known.has(id(e.runId, e.nodeKey)))).map(entryOf);
	const nodeLines = nodes.flatMap((n): LogEntry[] => {
		const evs = byNode.get(id(n.runId, n.nodeKey)) ?? [];
		const at = n.startedAt ?? n.endedAt ?? evs[0]?.occurredAt;
		if (!at || n.status === 'pending') return [];
		const checks = evs.filter((e) => e.type === 'audit_completed');
		const o = outcomeText(n, checks, skipCause(n), now);
		const where = n.errorLocation ? (n.errorLocation.line == null ? n.errorLocation.file : `${n.errorLocation.file}:${n.errorLocation.line}`) : null;
		const details: [string, string][] = [
			['Kind', n.kind ?? 'node'],
			...(n.startedAt ? [['Started', fullStamp(n.startedAt)] as [string, string]] : []),
			...(n.endedAt && n.startedAt ? [['Took', secsText(Date.parse(n.endedAt) - Date.parse(n.startedAt))] as [string, string]] : []),
			...(rowsText(n) ? [['Rows', rowsText(n)!] as [string, string]] : []),
			...(n.noChangeReason ? [['No change', n.noChangeReason] as [string, string]] : []),
			...(n.errorMessage ? [['Error', n.errorMessage] as [string, string]] : []),
			...(where ? [['Location', where] as [string, string]] : []),
			...(n.status === 'skipped' && skipCause(n) ? [['Why', skipCause(n)!.replace(/^skipped: /, '')] as [string, string]] : []),
		];
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
				pinned: false,
				tag: nodeTag(n, checks),
				details,
				checks: checksOf(checks),
				node: n,
				long: n.errorDetail ?? undefined
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
			omitted: l.omitted,
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
