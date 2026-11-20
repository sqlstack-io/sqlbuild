// Run page prototype (CHI-1591): skip causes, the summary line and quick picks. Pure; no side effects.

import type { RunEdge, RunEvent, RunNode } from '$lib/api';
import { nodeLabel } from '$lib/operate/labels';

/**
 * Why a skipped node was skipped, on one line, naming the chain: a node under a failure says that node
 * failed; one under another skipped node says that node was skipped, and after which failure.
 */
export function skipCause(n: RunNode, nodes: readonly RunNode[], edges: readonly RunEdge[]): string | null {
	if (n.status !== 'skipped') return null;
	const why = n.skippedBecauseOf;
	if (!why) return 'skipped: an upstream node did not run';
	const across = why.runId !== n.runId ? ` in ${why.jobKey ?? why.runId}` : '';
	const byKey = new Map(nodes.map((x) => [x.nodeKey, x]));
	const parents = edges.filter((e) => e.to === n.nodeKey).map((e) => byKey.get(e.from));
	if (parents.some((p) => p?.nodeKey === why.nodeKey)) return `skipped: ${nodeLabel(why.nodeKey)} failed${across}`;
	const via = parents.find((p) => p?.status === 'skipped' && p.skippedBecauseOf?.nodeKey === why.nodeKey);
	if (via) return `skipped: ${nodeLabel(via.nodeKey)} was skipped (after ${nodeLabel(why.nodeKey)} failed${across})`;
	return `skipped: ${nodeLabel(why.nodeKey)} failed${across}`;
}

export type RunSummary = {
	built: number;
	incremental: number;
	incrementalRows: number;
	failed: number;
	warned: number;
	skipped: number;
	notRun: number;
	running: number;
	waiting: number;
	checksPassed: number;
	checksWarned: number;
	checksFailed: number;
};

/** What the run did, for the line under the header: built nodes and the rows incrementals added, failures, skips and checks. */
export function summaryOf(nodes: readonly RunNode[], events: readonly RunEvent[], active: boolean): RunSummary {
	const built = nodes.filter((n) => n.status === 'success' || n.status === 'warn');
	const incremental = built.filter((n) => n.kind === 'incremental');
	const checks = events.filter((e) => e.type === 'audit_completed');
	return {
		built: built.length,
		incremental: incremental.length,
		incrementalRows: incremental.reduce((sum, n) => sum + (n.rowsAffected ?? 0), 0),
		failed: nodes.filter((n) => n.status === 'failed').length,
		warned: nodes.filter((n) => n.status === 'warn').length,
		skipped: nodes.filter((n) => n.status === 'skipped').length,
		notRun: nodes.filter((n) => n.status === 'not_run' || n.status === 'cancelled' || (!active && n.status === 'pending')).length,
		running: nodes.filter((n) => n.status === 'running').length,
		waiting: active ? nodes.filter((n) => n.status === 'pending').length : 0,
		checksPassed: checks.filter((e) => e.level === 'info').length,
		checksWarned: checks.filter((e) => e.level === 'warn').length,
		checksFailed: checks.filter((e) => e.level === 'error').length
	};
}

export type QuickPick = 'failed' | 'downstream' | 'skipped' | 'all';

export const QUICK_PICKS: { value: QuickPick; label: string }[] = [
	{ value: 'failed', label: 'Failed' },
	{ value: 'downstream', label: 'Failed + downstream' },
	{ value: 'skipped', label: 'Skipped' },
	{ value: 'all', label: 'All' }
];

/**
 * The selector text a quick pick fills the selection with, in SQLBuild's syntax: names, `name+` for a
 * node and its downstream, and the job's own selector (`job:shop_daily`) for All.
 */
export function quickPickText(pick: QuickPick, nodes: readonly RunNode[], jobKey: string | null): string {
	const names = (keep: (n: RunNode) => boolean) => [...new Set(nodes.filter(keep).map((n) => nodeLabel(n.nodeKey)))];
	if (pick === 'failed') return names((n) => n.status === 'failed').join(' ');
	if (pick === 'downstream') return names((n) => n.status === 'failed').map((name) => `${name}+`).join(' ');
	if (pick === 'skipped') return names((n) => n.status === 'skipped').join(' ');
	return jobKey ? `job:${jobKey}` : names(() => true).join(' ');
}

/** The selection text with `name` added, or removed when it is already there as a plain name. */
export function withNameToggled(text: string, name: string): string {
	const tokens = text.split(/\s+/).filter(Boolean);
	return (tokens.includes(name) ? tokens.filter((t) => t !== name) : [...tokens, name]).join(' ');
}

/** A full local timestamp with milliseconds, such as `2026-11-20 09:22:51.204`. */
export function fullStamp(iso: string): string {
	const d = new Date(iso);
	const p = (n: number, w = 2) => String(n).padStart(w, '0');
	return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`;
}
