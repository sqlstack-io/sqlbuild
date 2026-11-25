// Autocomplete for the selection field: what to offer for the part being typed, grouped as the
// design draws it (recent selections, selector types, values with node counts, node names). Each
// list stops at a few matches, so a keystroke costs one pass over the values at most.

import type { SelectionVocabulary, VocabularyEntry } from '$lib/api';

/** Selector types offered, with one-line hints, in the order they are listed. */
export const SELECTOR_HINTS: { type: string; hint: string }[] = [
	{ type: 'tag', hint: 'nodes with a tag' },
	{ type: 'path', hint: 'nodes under a folder (a name with / works too)' },
	{ type: 'job', hint: 'everything a job selects; globs allowed' },
	{ type: 'kind', hint: 'every node of one kind: model, source, seed, …' },
	{ type: 'source', hint: 'a source table' },
	{ type: 'seed', hint: 'a seed' },
	{ type: 'test', hint: 'a test' }
];

/** The vocabulary list each selector type takes its values from. */
export const VALUES_OF: Record<string, keyof Omit<SelectionVocabulary, 'snapshotId'>> = {
	tag: 'tags',
	path: 'paths',
	job: 'jobs',
	kind: 'kinds',
	source: 'sources',
	seed: 'seeds'
};

/** What a group of values is called, by selector type. */
const GROUP_NAME: Record<string, string> = { tag: 'Tags', path: 'Folders', job: 'Jobs', kind: 'Kinds', source: 'Sources', seed: 'Seeds' };

/** A node name to offer, with where it lives. */
export type NameEntry = { value: string; hint: string; count?: number };

export type SuggestionItem = {
	/** The part's new text. */
	insert: string;
	/** Selector type, for the icon and the `type:` prefix; `null` for a node name or a recent selection. */
	type: string | null;
	label: string;
	hint?: string;
	count?: number;
	/** Finishes the token as a chip; otherwise it stays text to go on typing. */
	complete: boolean;
};

export type SuggestionGroup = { name: string; items: SuggestionItem[]; counts?: boolean };

export const VALUE_LIMIT = 6;
const MIXED_LIMIT = 4;

/** Up to `limit` entries whose value contains `query`, ignoring case, in their order. */
export function firstMatches<E extends { value: string }>(entries: readonly E[], query: string, limit: number): E[] {
	const q = query.toLowerCase();
	const out: E[] = [];
	for (const e of entries) {
		if (out.length >= limit) break;
		if (!q || e.value.toLowerCase().includes(q)) out.push(e);
	}
	return out;
}

/** The most-used tags, for the empty field. */
export function popularTags(tags: readonly VocabularyEntry[], limit = 4): VocabularyEntry[] {
	return [...tags].sort((a, b) => b.count - a.count || a.value.localeCompare(b.value)).slice(0, limit);
}

/**
 * Suggestions for `typed`, the part in the input. A leading `+` stays on what is inserted. With
 * nothing typed: recent selections, the selector types and the popular tags; after `type:`, that
 * type's values; otherwise matching types, the values of a type whose name the fragment starts,
 * matching values of every other type and node names. `runs` are run selectors a page offers, such as
 * the rerun plan's `failed:<id>+`, listed first while they match.
 */
export function suggest(
	typed: string,
	vocabulary: SelectionVocabulary | null,
	names: readonly NameEntry[],
	recent: readonly { select: string; count: number | null }[] = [],
	runs: readonly SuggestionItem[] = []
): SuggestionGroup[] {
	const plus = typed.startsWith('+') ? '+' : '';
	const rest = typed.slice(plus.length);
	const groups: SuggestionGroup[] = [];
	// Run selectors the page offers (`failed:<id>+`, `run:<id>`, …), first, while what is typed could start one.
	const runItems = rest ? runs.filter((r) => r.insert.toLowerCase().startsWith(rest.toLowerCase()) || r.insert.slice(r.insert.indexOf(':') + 1).toLowerCase().startsWith(rest.toLowerCase())) : runs;
	if (!plus && runItems.length) groups.push({ name: 'Runs', items: [...runItems] });
	const values = (type: string, query: string, limit: number): SuggestionItem[] => {
		const list = vocabulary?.[VALUES_OF[type]] ?? [];
		return firstMatches(list, query, limit).map((e) => ({ insert: `${plus}${type}:${e.value}`, type, label: e.value, count: e.count, complete: true }));
	};
	if (!rest) {
		if (!plus && recent.length) groups.push({ name: 'Recent', items: recent.map((r) => ({ insert: r.select, type: null, label: r.select, count: r.count ?? undefined, complete: true })), counts: true });
		groups.push({ name: 'Selectors', items: SELECTOR_HINTS.map((s) => ({ insert: `${plus}${s.type}:`, type: s.type, label: `${s.type}:`, hint: s.hint, complete: false })) });
		if (vocabulary?.tags.length) groups.push({ name: 'Popular tags', items: popularTags(vocabulary.tags).map((e) => ({ insert: `${plus}tag:${e.value}`, type: 'tag', label: e.value, count: e.count, complete: true })), counts: true });
		return groups;
	}
	const colon = rest.indexOf(':');
	if (colon > 0) {
		const type = rest.slice(0, colon);
		if (VALUES_OF[type]) {
			const items = values(type, rest.slice(colon + 1), 50);
			if (items.length) groups.push({ name: GROUP_NAME[type], items, counts: true });
		}
		return groups;
	}
	const q = rest.toLowerCase();
	const types = SELECTOR_HINTS.filter((s) => s.type.startsWith(q));
	if (types.length) groups.push({ name: 'Selectors', items: types.map((s) => ({ insert: `${plus}${s.type}:`, type: s.type, label: `${s.type}:`, hint: s.hint, complete: false })) });
	// A fragment that starts a type's name, such as `tag`, lists that type's values first.
	const named = Object.keys(VALUES_OF).filter((type) => type.startsWith(q));
	for (const type of [...named, ...Object.keys(VALUES_OF).filter((type) => !named.includes(type))]) {
		const items = named.includes(type) ? values(type, '', VALUE_LIMIT) : values(type, rest, MIXED_LIMIT);
		if (items.length) groups.push({ name: GROUP_NAME[type], items, counts: true });
	}
	const nodes = firstMatches(names, rest, VALUE_LIMIT).map((n) => ({ insert: `${plus}${n.value}`, type: null, label: n.value, hint: n.hint, complete: true }));
	if (nodes.length) groups.push({ name: 'Nodes', items: nodes });
	return groups;
}
