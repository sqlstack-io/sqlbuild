import { topFolder } from '$lib/lineage/catalog-graph';
import { checkCountsOf } from './http-catalog';
import type { Acknowledgement, AttentionItem, AttentionRun, CatalogNode, Check, GroupStatus, Overview, PlatformEvent, Run, SnapshotChange } from './types';

const DAY = 24 * 60 * 60 * 1000;
const MODEL_KINDS = new Set(['view', 'table', 'incremental']);
const PROBLEM_STATUSES = new Set(['failed', 'partial', 'presumed_failed']);
const ACTIVE_STATUSES = new Set(['queued', 'launched', 'running', 'unresponsive']);
const RECENT_CHANGES = 4;

export type OverviewInputs = {
	/** Listed runs, newest first; the listing holds the newest 1,000, so a busier day undercounts. */
	runs: Run[];
	checks: Check[];
	nodes: CatalogNode[];
	/** Publications, newest first. */
	changes: SnapshotChange[];
	events: PlatformEvent[];
	/** Problem runs marked handled, by run ID. */
	acknowledgements: Map<string, Acknowledgement>;
	now: number;
};

/**
 * One problem per job, newest first: the job's latest run that ended today, when that run did not succeed.
 * Only a later failure or a good run of the whole job replaces it, so a cancelled run or a run of part of the
 * job never hides a failure, and acknowledging the latest failure never brings an older one back.
 * Each ad-hoc run is its own item.
 */
export function problemRunsOf(topLevel: Run[], now: number): Run[] {
	const latest = new Map<string, Run>();
	for (const r of topLevel) {
		if (!r.endedAt || Date.parse(r.endedAt) < now - DAY) continue;
		const settles = PROBLEM_STATUSES.has(r.status) || ((r.status === 'success' || r.status === 'warn') && !r.narrowing && !r.window);
		if (!settles) continue;
		const job = r.jobKey ?? `adhoc:${r.id}`;
		const seen = latest.get(job);
		if (!seen || Date.parse(r.endedAt) > Date.parse(seen.endedAt!)) latest.set(job, r);
	}
	return [...latest.values()].filter((r) => PROBLEM_STATUSES.has(r.status)).sort((a, b) => Date.parse(b.endedAt!) - Date.parse(a.endedAt!));
}

const attentionRun = ({ id, jobKey, kind, status, statusReason, nodeCounts, startedAt, endedAt }: Run): AttentionRun => ({ id, jobKey, kind, status, statusReason, nodeCounts, startedAt, endedAt });

/** Build state per top-level folder, for the nodes SQLBuild builds; a node can be both changed and behind. */
export function groupStatusOf(nodes: CatalogNode[]): GroupStatus[] {
	const groups = new Map<string, GroupStatus>();
	for (const n of nodes) {
		if (n.kind === 'source') continue;
		const group = topFolder(n);
		const g = groups.get(group) ?? { group, total: 0, current: 0, changed: 0, behind: 0, failed: 0 };
		const stale = n.stale ?? [];
		g.total += 1;
		if (n.lastOutcome === 'failed') g.failed += 1;
		if (stale.some((s) => s.reason !== 'behind')) g.changed += 1;
		if (stale.some((s) => s.reason === 'behind')) g.behind += 1;
		if (!stale.length && n.lastOutcome !== 'failed' && n.lastBuild) g.current += 1;
		groups.set(group, g);
	}
	return [...groups.values()].sort((a, b) => a.group.localeCompare(b.group));
}

/**
 * The overview from reads the person already has. Staleness comes from the catalog listing, which
 * does not check source freshness, so late sources stay unknown. Groups are top-level folders, and
 * group status is null when the server reports no build state. There is no cost read, so the page hides cost.
 */
export function overviewOf({ runs, checks, nodes, changes, events, acknowledgements, now }: OverviewInputs): Overview {
	const acknowledged = (r: Run) => acknowledgements.get(r.id) ?? null;
	const topLevel = runs.filter((r) => !r.parentRunId);
	const recent = topLevel.filter((r) => Date.parse(r.queuedAt) >= now - DAY);
	const count = (statuses: string[]) => recent.filter((r) => statuses.includes(r.status)).length;
	const attention: AttentionItem[] = [
		...problemRunsOf(topLevel, now).map((r): AttentionItem => ({ kind: 'run_problem', run: attentionRun(r), acknowledged: acknowledged(r) })),
		...topLevel
			.filter((r) => r.status === 'unresponsive')
			.map((r): AttentionItem => ({ kind: 'run_unresponsive', run: attentionRun(r), lastHeartbeatAt: null, acknowledged: acknowledged(r) })),
		...events.filter((e) => e.active).map((event): AttentionItem => ({ kind: 'platform_event', event }))
	];
	const staleKeys = (test: (reason: string) => boolean) => nodes.filter((n) => n.stale?.some((r) => test(r.reason))).map((n) => n.key);
	const changed = staleKeys((reason) => reason !== 'behind');
	const behind = staleKeys((reason) => reason === 'behind');
	const staleNodes = new Set([...changed, ...behind]).size;
	if (staleNodes) attention.push({ kind: 'stale_models', changed, behind });
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
			stale: { nodes: staleNodes, changed: changed.length, behind: behind.length, sources: null },
			cost: null
		},
		attention,
		groups: nodes.some((n) => n.stale !== null) ? groupStatusOf(nodes) : null,
		recentChanges: changes.slice(0, RECENT_CHANGES),
		latestRun: latest ? { id: latest.id, status: latest.status, endedAt: latest.endedAt } : null
	};
}
