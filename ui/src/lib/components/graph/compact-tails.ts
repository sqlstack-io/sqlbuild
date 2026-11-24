// The part of each card's name a zoomed-out pill shows: the shortest run of its last `__` parts that no
// sibling card (in the same box, or loose on the graph) ends with, so `orders__stg__events` reads
// `events` unless a sibling shares that ending. Pure.

import { sharedLeads } from '$lib/lineage/discovered-labels';

/** The title a card shows: its label, else its name, its job's key or its folder. */
export function compactTitleOf(data: Record<string, unknown> | undefined): string | null {
	const title = data?.label ?? data?.name ?? data?.jobKey ?? data?.group;
	return typeof title === 'string' ? title : null;
}

/** Each name's tail: the shortest suffix of its `__` parts that is unique among `names`. */
export function compactTails(names: Iterable<string>): Map<string, string> {
	const unique = [...new Set(names)];
	const split = unique.map((name) => name.split('__'));
	// How many names end with each run of parts; a name counts once per run.
	const ends = new Map<string, number>();
	for (const parts of split) for (let k = 1; k <= parts.length; k++) {
		const tail = parts.slice(-k).join('__');
		ends.set(tail, (ends.get(tail) ?? 0) + 1);
	}
	const tails = new Map<string, string>();
	unique.forEach((name, i) => {
		const parts = split[i];
		let tail = name;
		for (let k = 1; k < parts.length; k++) {
			const run = parts.slice(-k).join('__');
			if (run && ends.get(run) === 1) {
				tail = run;
				break;
			}
		}
		tails.set(name, tail);
	});
	return tails;
}

/** Each card's pill name, by card id: its title's tail among the cards with the same parent. */
export function siblingTails(cards: { id: string; parent: string; title: string }[]): Map<string, string> {
	const byId = new Map<string, string>();
	for (const siblings of Map.groupBy(cards, (c) => c.parent).values()) {
		const tails = compactTails(siblings.map((c) => c.title));
		for (const c of siblings) byId.set(c.id, tails.get(c.title) ?? c.title);
	}
	return byId;
}

/** Each card's pill name's shared lead among its siblings' pill names (`sharedLeads`), by card id. */
export function siblingLeads(cards: { id: string; parent: string }[], tails: Map<string, string>): Map<string, number> {
	const byId = new Map<string, number>();
	for (const siblings of Map.groupBy(cards, (c) => c.parent).values()) {
		const leads = sharedLeads(siblings.map((c) => tails.get(c.id) ?? ''));
		for (const c of siblings) byId.set(c.id, leads.get(tails.get(c.id) ?? '') ?? 0);
	}
	return byId;
}

/** The context key a graph canvas gives its cards their pill names under. */
export const COMPACT_TAILS = Symbol('compact-tails');
/** A card's pill name and its parent box, by card id; undefined outside a graph canvas. */
export type CompactTails = { tail: (id: string) => string | undefined; parent: (id: string) => string | undefined; lead: (id: string) => number; zoom: () => number };
