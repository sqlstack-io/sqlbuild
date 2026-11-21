// The quick picks that fill the selection to plan, and toggling one node's name in it. Pure.

import type { RunNode } from '$lib/api';
import { nodeLabel } from '$lib/operate/labels';

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
