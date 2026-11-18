// The Overview's lower panels: recent deploys with quiet runs merged, and folders by what needs attention. Pure.

import type { GroupStatus, SnapshotChange } from '$lib/api';
import { formatAgo } from '$lib/format/time';
import { plural } from '$lib/format/plural';

/** Deploys that changed something the Recent changes panel shows. */
export const RECENT_DEPLOYS = 4;
/** Folders the Overview's Build status card shows at most; the catalog's folder view lists them all. */
export const FOLDER_ROWS = 8;

/** Whether a deploy changed any definition: a node, a job or trigger, a role or the environment. */
export function changedSomething(change: SnapshotChange): boolean {
	return change.nodes.length > 0 || change.orchestration.length > 0 || change.roles.length > 0 || change.environment !== null;
}

/**
 * The newest deploys, newest first, up to and including the `limit`th one that changed something, so
 * deploys without changes between them are kept to be merged and never crowd out a real change.
 */
export function recentDeploys(changes: readonly SnapshotChange[], limit = RECENT_DEPLOYS): SnapshotChange[] {
	const taken: SnapshotChange[] = [];
	let changed = 0;
	for (const change of changes) {
		if (changed >= limit) break;
		taken.push(change);
		if (changedSomething(change)) changed += 1;
	}
	return taken;
}

export type RecentDeployRow =
	| { kind: 'deploy'; key: string; change: SnapshotChange }
	| { kind: 'quiet'; key: string; count: number; newestAt: string; oldestAt: string; changes: SnapshotChange[] };

/** Deploys newest first, with each run of deploys that changed nothing merged into one row. */
export function recentDeployRows(changes: readonly SnapshotChange[]): RecentDeployRow[] {
	const rows: RecentDeployRow[] = [];
	for (const change of changes) {
		if (changedSomething(change)) {
			rows.push({ kind: 'deploy', key: change.snapshotId, change });
			continue;
		}
		const last = rows.at(-1);
		if (last?.kind === 'quiet') {
			last.count += 1;
			last.oldestAt = change.publishedAt;
			last.changes.push(change);
		} else {
			rows.push({ kind: 'quiet', key: `quiet-${change.snapshotId}`, count: 1, newestAt: change.publishedAt, oldestAt: change.publishedAt, changes: [change] });
		}
	}
	return rows;
}

/** e.g. "3 deploys without changes · 7h–10h ago", or "1 deploy without changes · 2d ago". */
export function quietDeployLine(row: Extract<RecentDeployRow, { kind: 'quiet' }>, now: number): string {
	const newest = formatAgo(row.newestAt, now);
	const oldest = formatAgo(row.oldestAt, now);
	const when = newest === oldest ? newest : `${newest.replace(/ ago$/, '')}–${oldest}`;
	return `${plural(row.count, 'deploy')} without changes · ${when}`;
}

/** Whether a folder has failed, changed or behind nodes. */
export function needsAttention(g: GroupStatus): boolean {
	return g.failed > 0 || g.changed > 0 || g.behind > 0;
}

/** Folders by what needs attention: most failed, then most changed, then most behind, then largest, then by name. */
export function sortFolders(groups: readonly GroupStatus[]): GroupStatus[] {
	return [...groups].sort(
		(a, b) => b.failed - a.failed || b.changed - a.changed || b.behind - a.behind || b.total - a.total || a.group.localeCompare(b.group)
	);
}

/**
 * The Overview card's folders: at most `limit`, most urgent first, however many need attention (the
 * catalog's folder view lists them all), with how many there are and how many need attention.
 */
export function cappedFolders(groups: readonly GroupStatus[], limit = FOLDER_ROWS): { rows: GroupStatus[]; total: number; attention: number } {
	return { rows: sortFolders(groups).slice(0, limit), total: groups.length, attention: groups.filter(needsAttention).length };
}
