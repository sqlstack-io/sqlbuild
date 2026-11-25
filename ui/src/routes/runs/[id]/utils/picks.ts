// The quick picks that fill the selection to plan, and toggling one node's name in it. Pure.

import type { RunNode } from '$lib/api';
import { nodeLabel } from '$lib/operate/labels';

export type QuickPick = 'unfinished' | 'skipped' | 'all';

/** The picks the toolbar offers; `all` stays for links and memory, the Rerun menu covers the whole run. */
export const QUICK_PICKS: { value: QuickPick; label: string }[] = [
	{ value: 'unfinished', label: "Didn't succeed" },
	{ value: 'skipped', label: 'Skipped' }
];

/** Nodes that ran and didn't succeed: failed, cancelled, or stopped before they ran. */
const UNFINISHED = new Set(['failed', 'cancelled', 'not_run']);

/**
 * The selector text a quick pick fills the selection with, in SQLBuild's syntax: names, and the job's
 * own selector (`job:shop_daily`) for all.
 */
export function quickPickText(pick: QuickPick, nodes: readonly RunNode[], jobKey: string | null): string {
	const names = (keep: (n: RunNode) => boolean) => [...new Set(nodes.filter(keep).map((n) => nodeLabel(n.nodeKey)))];
	if (pick === 'unfinished') return names((n) => UNFINISHED.has(n.status)).join(' ');
	if (pick === 'skipped') return names((n) => n.status === 'skipped').join(' ');
	return jobKey ? `job:${jobKey}` : names(() => true).join(' ');
}

/** The selection text with `name` added, or removed when it is already there as a plain name. */
export function withNameToggled(text: string, name: string): string {
	const tokens = text.split(/\s+/).filter(Boolean);
	return (tokens.includes(name) ? tokens.filter((t) => t !== name) : [...tokens, name]).join(' ');
}
