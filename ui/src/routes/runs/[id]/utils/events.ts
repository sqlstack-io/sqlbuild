// Plain one-line summaries of run events; the raw type stays available as a tooltip.

import type { RunEvent, RunEventLevel, RunOutputLine } from '$lib/api';
import { eventTone } from '$lib/operate/labels';
import type { EventCategory } from '../types';

export const EVENT_CATEGORIES: { value: EventCategory; label: string }[] = [
	{ value: 'run', label: 'Run' },
	{ value: 'nodes', label: 'Nodes' },
	{ value: 'queries', label: 'Queries' },
	{ value: 'checks', label: 'Checks' },
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
			return 'Waiting for its turn';
		case 'claimed':
			return 'Its turn came: the data it writes is reserved for this run';
		case 'launched':
			return 'Started a container for the run';
		case 'cancel_requested':
			return e.message;
		case 'presumed_failed':
			return 'Lost contact with the run; marked failed';
		case 'terminated':
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
export function logEntries(events: readonly RunEvent[], output: readonly (RunOutputLine & { runId: string })[]): LogEntry[] {
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
			level: 'info',
			category: 'output',
			text: l.text,
			title: l.stream,
			tone: l.stream === 'stderr' ? 'var(--sb-warning)' : 'var(--sb-text-faint)',
			stream: l.stream,
			statementId: null,
			queries: [],
			pinned: lastStderr.get(l.runId) === i
		})
	);
	// A stable sort keeps each source's own order for equal times.
	return [...fromEvents, ...fromOutput].sort((a, b) => a.at.localeCompare(b.at));
}
