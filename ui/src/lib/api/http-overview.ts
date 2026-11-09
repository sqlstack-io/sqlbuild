import { checkCountsOf } from './http-catalog';
import type { AttentionItem, AttentionRun, CatalogNode, Check, Overview, PlatformEvent, Run, SnapshotChange } from './types';

const DAY = 24 * 60 * 60 * 1000;
const MODEL_KINDS = new Set(['view', 'table', 'incremental']);
const PROBLEM_STATUSES = new Set(['failed', 'partial', 'presumed_failed']);
const ACTIVE_STATUSES = new Set(['queued', 'launched', 'running', 'unresponsive']);
/** The failed runs the health banner lists; the runs page has the rest. */
const PROBLEM_RUNS = 3;
const RECENT_CHANGES = 4;

export type OverviewInputs = {
	/** Listed runs, newest first; the listing holds the newest 1,000, so a busier day undercounts. */
	runs: Run[];
	checks: Check[];
	nodes: CatalogNode[];
	/** Publications, newest first. */
	changes: SnapshotChange[];
	events: PlatformEvent[];
	now: number;
};

const attentionRun = ({ id, jobKey, kind, status, statusReason, startedAt, endedAt }: Run): AttentionRun => ({ id, jobKey, kind, status, statusReason, startedAt, endedAt });

/**
 * The overview from reads the person already has. The server has no project-wide build state, cost
 * or group read yet, so stale counts and group status are null and cost is left out; the page hides them.
 */
export function overviewOf({ runs, checks, nodes, changes, events, now }: OverviewInputs): Overview {
	const topLevel = runs.filter((r) => !r.parentRunId);
	const recent = topLevel.filter((r) => Date.parse(r.queuedAt) >= now - DAY);
	const count = (statuses: string[]) => recent.filter((r) => statuses.includes(r.status)).length;
	const attention: AttentionItem[] = [
		...topLevel
			.filter((r) => r.endedAt && Date.parse(r.endedAt) >= now - DAY && PROBLEM_STATUSES.has(r.status))
			.slice(0, PROBLEM_RUNS)
			.map((r): AttentionItem => ({ kind: 'run_problem', run: attentionRun(r) })),
		...topLevel.filter((r) => r.status === 'unresponsive').map((r): AttentionItem => ({ kind: 'run_unresponsive', run: attentionRun(r), lastHeartbeatAt: null })),
		...events.filter((e) => e.active).map((event): AttentionItem => ({ kind: 'platform_event', event }))
	];
	const thisWeek = changes.filter((c) => Date.parse(c.publishedAt) >= now - 7 * DAY).flatMap((c) => c.nodes);
	const latest = topLevel.filter((r) => r.endedAt).sort((a, b) => Date.parse(b.endedAt!) - Date.parse(a.endedAt!))[0];
	return {
		metrics: {
			models: {
				total: nodes.filter((n) => MODEL_KINDS.has(n.kind)).length,
				addedThisWeek: thisWeek.filter((c) => c.op === 'added' && c.key.startsWith('model:')).length,
				removedThisWeek: thisWeek.filter((c) => c.op === 'removed' && c.key.startsWith('model:')).length
			},
			runs24h: {
				total: recent.length,
				success: count(['success']),
				warn: count(['warn']),
				partial: count(['partial']),
				failed: count(['failed', 'presumed_failed']),
				active: recent.filter((r) => ACTIVE_STATUSES.has(r.status)).length
			},
			checks: checkCountsOf(checks),
			stale: null,
			cost: null
		},
		attention,
		groups: null,
		recentChanges: changes.slice(0, RECENT_CHANGES),
		latestRun: latest ? { id: latest.id, status: latest.status, endedAt: latest.endedAt } : null
	};
}
