// How graph cards and the side panel name nodes. A discovered
// node, `asset:<asset>[<step>]`, is named by its step, the only part its siblings don't share; any
// other name that must shorten loses the prefix it shares with its siblings, never the part that
// tells it apart. Pure.

import { shortName } from '$lib/build-area/labels';

/** A discovered node's asset and step, from its key; null for any other node. */
export function discoveredStep(key: string): { asset: string; step: string } | null {
	const m = /^(.+)\[([^\]]+)\]$/.exec(shortName(key));
	return m ? { asset: m[1], step: m[2] } : null;
}

/** A card's name: a discovered node's step, else its key without its kind prefix. */
export function cardLabel(key: string): string {
	return discoveredStep(key)?.step ?? shortName(key);
}

/** Characters a name may be cut after: the joints between its words. */
const JOINTS = /[_.[/:-]/;

/** How many leading characters of `a` it shares with `b`, up to and including their last shared joint. */
function sharedUpToJoint(a: string, b: string): number {
	let n = 0;
	while (n < a.length && n < b.length && a[n] === b[n]) n++;
	while (n > 0 && !JOINTS.test(a[n - 1])) n--;
	return n;
}

/**
 * For each name, the length of the longest prefix, ending at a joint, that it shares with another
 * name: the part a shortened name drops first. 0 when it shares none, or when what is left would
 * read the same as another name's rest, as `stg_customers` and `dim_customers` would.
 */
export function sharedLeads(names: string[]): Map<string, number> {
	const sorted = [...new Set(names)].sort();
	const leads = new Map<string, number>();
	sorted.forEach((name, i) => {
		// Sorted, the name sharing the longest prefix is a neighbour.
		const before = i > 0 ? sharedUpToJoint(name, sorted[i - 1]) : 0;
		const after = i < sorted.length - 1 ? sharedUpToJoint(name, sorted[i + 1]) : 0;
		const lead = Math.max(before, after);
		leads.set(name, lead < name.length ? lead : 0);
	});
	const rests = Map.groupBy(sorted, (name) => name.slice(leads.get(name)));
	for (const same of rests.values()) if (same.length > 1) for (const name of same) leads.set(name, 0);
	return leads;
}

/** `text` in pieces that may break after a joint (`_ . [ / :`), for `<wbr>` between them. */
export function breakable(text: string): string[] {
	return text.split(/(?<=[_.[/:])/);
}
