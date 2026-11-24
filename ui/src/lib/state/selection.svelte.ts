// The selection bar's state: the selection and except text, what the server resolves them to
// (debounced, latest answer wins), the selector vocabulary for autocomplete (read once per
// snapshot and shared by every page), recent selections (this browser) and the URL.

import { browser } from '$app/environment';
import { goto } from '$app/navigation';
import { page } from '$app/state';
import type { ApiClient, Scope, SelectionEvaluation, SelectionVocabulary } from '$lib/api';
import { readSelectionParams, writeSelectionParams, type Selection } from '$lib/selection/command';
import { untrack } from 'svelte';
import { app } from './app.svelte';

/** A selection's text with its spacing evened out, for comparing. */
const squash = (text: string) => text.split(/\s+/).filter(Boolean).join(' ');

/** Typing pauses this long before a selection is evaluated. */
export const EVALUATE_DELAY_MS = 150;
/** Keys read per request; the server allows up to 10,000. */
const PAGE = 10_000;
/** Most keys read for one selection; past this the page treats the selection as unknown in detail. */
const MAX_KEYS = 200_000;
const RECENT_KEY = 'sqlbuild.selection.recent';
const RECENT_MAX = 5;

export type RecentSelection = Selection & { count: number | null };

// ---- vocabulary: one read per location and snapshot, shared ----

type Held = { snapshotId: string | null; data: Promise<SelectionVocabulary> };
const vocabularies = new Map<string, Held>();

/** The location's vocabulary; read again only when `snapshotId` names a newer snapshot than the one held. */
export function vocabularyFor(client: ApiClient, scope: Scope, snapshotId: string | null): Promise<SelectionVocabulary> {
	const held = vocabularies.get(scope.locationId);
	if (held && (snapshotId === null || held.snapshotId === null || held.snapshotId === snapshotId)) return held.data;
	const entry: Held = { snapshotId, data: client.selectionVocabulary(scope) };
	vocabularies.set(scope.locationId, entry);
	entry.data.then(
		(v) => (entry.snapshotId = v.snapshotId),
		() => vocabularies.delete(scope.locationId)
	);
	return entry.data;
}

// ---- recent selections ----

export function readRecent(): RecentSelection[] {
	if (!browser) return [];
	try {
		const list = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
		return Array.isArray(list) ? list.filter((r) => typeof r?.select === 'string').slice(0, RECENT_MAX) : [];
	} catch {
		return [];
	}
}

function saveRecent(list: RecentSelection[]) {
	if (browser) localStorage.setItem(RECENT_KEY, JSON.stringify(list.slice(0, RECENT_MAX)));
}

// ---- the selection ----

export type SelectionState = ReturnType<typeof createSelection>;

/**
 * A selection bar's state. Must be created during component initialisation. With `url`, the
 * selection starts from (unless `initial` is given) and is kept in the page's `?select=…&exclude=…`;
 * the older parameters in `replaces`, read into `initial` by the page, are dropped from the URL.
 * With `follow`, links and Back that change the URL's selection change it here too.
 */
export function createSelection(options: { url?: boolean; follow?: boolean; initial?: Selection; replaces?: string[] } = {}) {
	const start = options.initial ?? (options.url ? readSelectionParams(page.url.searchParams) : { select: '', exclude: '' });
	let select = $state(start.select);
	let exclude = $state(start.exclude);
	let evaluation = $state.raw<SelectionEvaluation | null>(null);
	/** Every key the selection resolves to, once read; null while unknown or when nothing is selected. */
	let keys = $state.raw<ReadonlySet<string> | null>(null);
	let evaluating = $state(false);
	let failed = $state<string | null>(null);
	let vocabulary = $state.raw<SelectionVocabulary | null>(null);
	let recent = $state.raw<RecentSelection[]>(readRecent());
	let asked = 0;

	const active = $derived(!!(select.trim() || exclude.trim()));
	/** The selected nodes' names, without their `kind:`, for lists that name nodes only by name. */
	const names = $derived(keys && new Set([...keys].map((k) => k.slice(k.indexOf(':') + 1))));
	const supported = $derived(app.client.supports('evaluateSelection'));

	async function evaluate(scope: Scope, request: Selection, ticket: number) {
		evaluating = true;
		try {
			const first = await app.client.evaluateSelection(scope, { ...request, limit: PAGE });
			if (ticket !== asked) return;
			evaluation = first;
			failed = null;
			if (first.error) {
				keys = new Set();
				return;
			}
			const all = [...first.keys];
			let next = first.nextOffset;
			while (next !== null && all.length < MAX_KEYS) {
				const more = await app.client.evaluateSelection(scope, { ...request, offset: next, limit: PAGE });
				if (ticket !== asked) return;
				all.push(...more.keys);
				next = more.nextOffset;
			}
			keys = next === null ? new Set(all) : null;
			// A newer snapshot than the vocabulary's: read the values again.
			if (first.snapshotId && vocabulary && vocabulary.snapshotId !== first.snapshotId) loadVocabulary(scope, first.snapshotId);
		} catch (error) {
			if (ticket !== asked) return;
			failed = error instanceof Error ? error.message : String(error);
			evaluation = null;
			keys = null;
		} finally {
			if (ticket === asked) evaluating = false;
		}
	}

	function loadVocabulary(scope: Scope, snapshotId: string | null) {
		vocabularyFor(app.client, scope, snapshotId).then(
			(v) => (vocabulary = v),
			() => undefined
		);
	}

	$effect(() => {
		const scope = app.scope;
		if (scope && app.client.supports('selectionVocabulary')) loadVocabulary(scope, null);
	});

	$effect(() => {
		const scope = app.scope;
		const request = { select: select.trim(), exclude: exclude.trim() };
		const ticket = ++asked;
		if (!scope || !supported || (!request.select && !request.exclude)) {
			evaluation = null;
			keys = null;
			failed = null;
			evaluating = false;
			return;
		}
		evaluating = true;
		const timer = setTimeout(() => evaluate(scope, request, ticket), EVALUATE_DELAY_MS);
		return () => clearTimeout(timer);
	});

	if (options.follow) {
		// Links and Back change the URL: take the selection from it, unless it already says the same.
		let seen = page.url.search;
		$effect(() => {
			const search = page.url.search;
			if (search === seen) return;
			seen = search;
			const linked = readSelectionParams(page.url.searchParams);
			untrack(() => {
				if (squash(linked.select) !== squash(select)) select = linked.select;
				if (squash(linked.exclude) !== squash(exclude)) exclude = linked.exclude;
			});
		});
	}
	if (options.url) {
		$effect(() => {
			const params = writeSelectionParams(page.url.searchParams, { select, exclude });
			for (const old of options.replaces ?? []) params.delete(old);
			if (params.toString() === page.url.searchParams.toString()) return;
			const url = new URL(page.url);
			url.search = params.toString();
			void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
		});
	}

	return {
		get select() {
			return select;
		},
		set select(v: string) {
			select = v;
		},
		get exclude() {
			return exclude;
		},
		set exclude(v: string) {
			exclude = v;
		},
		/** Whether anything is selected or excepted; otherwise every node is in. */
		get active() {
			return active;
		},
		get supported() {
			return supported;
		},
		get evaluation() {
			return evaluation;
		},
		get problem() {
			return evaluation?.error ?? null;
		},
		/** Reading the selection failed (not a refusal: the server could not be asked). */
		get failed() {
			return failed;
		},
		get evaluating() {
			return evaluating;
		},
		/** The selected keys; null when nothing narrows (every node is in) or not known yet. */
		get keys() {
			return keys;
		},
		get vocabulary() {
			return vocabulary;
		},
		get recent() {
			return recent;
		},
		/** Whether a node is in the selection: every node is when nothing narrows or the answer is pending. */
		has(key: string): boolean {
			return !active || keys === null || keys.has(key);
		},
		/** Whether a node named `name` (any kind) is in the selection, as `has` does. */
		hasName(name: string | null): boolean {
			return !active || names === null || (name !== null && names.has(name));
		},
		/** Remembers the current selection among this browser's recent ones. */
		remember() {
			const s = select.trim();
			if (!s || evaluation?.error) return;
			const entry = { select: s, exclude: exclude.trim(), count: evaluation?.count ?? null };
			recent = [entry, ...recent.filter((r) => r.select !== s || r.exclude !== entry.exclude)].slice(0, RECENT_MAX);
			saveRecent(recent);
		}
	};
}
