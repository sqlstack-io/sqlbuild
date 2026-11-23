// The Cost page's selection: the selection bar's state, kept in `?select=…&exclude=…` like other
// pages, resolved over the Cost mock in the browser (the mock server knows only the demo catalog).
// Links that drill set the URL; typing writes it back.
import { goto } from '$app/navigation';
import { page } from '$app/state';
import { readSelectionParams, writeSelectionParams } from '$lib/selection/command';
import type { RecentSelection, SelectionState } from '$lib/state/selection.svelte';
import { costVocabulary, evaluateCostSelection } from '$lib/api/mock/cost-selection';

const norm = (s: string) => s.split(/\s+/).filter(Boolean).join(' ');

export function createCostSelection(): SelectionState {
	const start = readSelectionParams(page.url.searchParams);
	let select = $state(start.select);
	let exclude = $state(start.exclude);
	let seen = `${start.select}\n${start.exclude}`;
	const vocabulary = costVocabulary();

	// A link that drills changes the URL: take it in.
	$effect(() => {
		const s = readSelectionParams(page.url.searchParams);
		const key = `${s.select}\n${s.exclude}`;
		if (key === seen) return;
		seen = key;
		select = s.select;
		exclude = s.exclude;
	});
	// Typing changes the selection: write it to the URL.
	$effect(() => {
		const key = `${norm(select)}\n${norm(exclude)}`;
		if (key === seen) return;
		seen = key;
		const url = new URL(page.url);
		url.search = writeSelectionParams(page.url.searchParams, { select, exclude }).toString();
		void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
	});

	const active = $derived(!!(select.trim() || exclude.trim()));
	const evaluation = $derived(active ? evaluateCostSelection(select, exclude) : null);
	const keys = $derived(evaluation && !evaluation.error ? new Set(evaluation.keys) : active ? new Set<string>() : null);
	const names = $derived(keys && new Set([...keys].map((k) => k.slice(k.indexOf(':') + 1))));
	const recent: RecentSelection[] = [];

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
		get active() {
			return active;
		},
		get supported() {
			return true;
		},
		get evaluation() {
			return evaluation;
		},
		get problem() {
			return evaluation?.error ?? null;
		},
		get failed() {
			return null;
		},
		get evaluating() {
			return false;
		},
		get keys() {
			return keys;
		},
		get vocabulary() {
			return vocabulary;
		},
		get recent() {
			return recent;
		},
		has(key: string) {
			return !active || keys === null || keys.has(key);
		},
		hasName(name: string | null) {
			return !active || names === null || (name !== null && names.has(name));
		},
		remember() {}
	} as SelectionState;
}
