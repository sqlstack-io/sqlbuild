import { browser } from '$app/environment';
import type { ApiClient, Scope } from '$lib/api';
import { channelMatches } from '$lib/api';
import { app } from './app.svelte';

/** How long a burst of change signals may last before one reload starts. */
const SIGNAL_PAUSE_MS = 300;
/** A first load faster than this shows no loading text, so it doesn't flash. */
const SLOW_MS = 400;

/**
 * The last answer of each read, kept in memory for this page load only, so returning to a page shows
 * it at once while it reloads. Keyed by person, location, page and read; signing out reloads the page,
 * which empties it.
 */
const remembered = new Map<string, unknown>();
const REMEMBER_MAX = 200;

function remember(key: string, value: unknown) {
	remembered.delete(key);
	remembered.set(key, value);
	if (remembered.size > REMEMBER_MAX) remembered.delete(remembered.keys().next().value!);
}

export type Resource<T> = {
	readonly data: T | undefined;
	readonly error: string | null;
	readonly loading: boolean;
	/** Loading with nothing to show for longer than a moment; pages show loading text only then. */
	readonly slow: boolean;
	refresh(): void;
};

/**
 * Loads data for the selected location and target, reloading when the scope
 * changes and when a "changed" signal arrives on one of `topics`.
 *
 * Change signals arrive in bursts while a run progresses, so they are coalesced:
 * one reload starts after a short pause, and signals during a reload queue at
 * most one more.
 *
 * Must be called during component initialisation. Reactive values read
 * synchronously inside `load` (such as route params) are also dependencies.
 *
 * A page that has loaded before shows its remembered data straight away and reloads it behind.
 */
export function createResource<T>(
	load: (client: ApiClient, scope: Scope) => Promise<T>,
	topics: readonly string[] = []
): Resource<T> {
	let data = $state<T | undefined>(undefined);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let slow = $state(false);
	let slowTimer: ReturnType<typeof setTimeout> | null = null;
	let shownKey = '';
	let latest = 0;
	let settled = 0;
	let reloadAgain = false;
	let pause: ReturnType<typeof setTimeout> | null = null;

	function run(scope: Scope) {
		const request = ++latest;
		// The page's path and query tell apart reads of different jobs, runs and filters made by the same code.
		const key = `${app.user?.name ?? ''}|${scope.locationId}|${browser ? window.location.pathname + window.location.search : ''}|${load}`;
		if (key !== shownKey && remembered.has(key)) data = remembered.get(key) as T;
		shownKey = key;
		loading = true;
		if (slowTimer) clearTimeout(slowTimer);
		slowTimer = setTimeout(() => (slow = data === undefined), SLOW_MS);
		const done = () => {
			if (slowTimer) clearTimeout(slowTimer);
			slowTimer = null;
			slow = false;
			settled = request;
			if (reloadAgain && request === latest) {
				reloadAgain = false;
				refresh();
			}
		};
		load(app.client, scope).then(
			(value) => {
				if (request !== latest) return;
				remember(key, value);
				data = value;
				error = null;
				loading = false;
				done();
			},
			(reason: unknown) => {
				if (request !== latest) return;
				error = reason instanceof Error ? reason.message : String(reason);
				loading = false;
				done();
			}
		);
	}

	function signalled() {
		if (pause) return;
		pause = setTimeout(() => {
			pause = null;
			if (settled === latest) refresh();
			else reloadAgain = true;
		}, SIGNAL_PAUSE_MS);
	}

	function refresh() {
		if (app.scope) run(app.scope);
	}

	$effect(() => {
		const scope = app.scope;
		if (scope) run(scope);
	});

	$effect(() => {
		if (!topics.length) return;
		const unsubscribe = app.client.changes.subscribe((signal) => {
			if (channelMatches(signal.channel, topics)) signalled();
		});
		return () => {
			unsubscribe();
			if (slowTimer) clearTimeout(slowTimer);
			if (pause) clearTimeout(pause);
			pause = null;
		};
	});

	return {
		get data() {
			return data;
		},
		get error() {
			return error;
		},
		get loading() {
			return loading;
		},
		get slow() {
			return slow;
		},
		refresh
	};
}
