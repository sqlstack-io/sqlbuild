import type { ApiClient, Scope } from '$lib/api';
import { channelMatches } from '$lib/api';
import { app } from './app.svelte';

/** How long a burst of change signals may last before one reload starts. */
const SIGNAL_PAUSE_MS = 300;

export type Resource<T> = {
	readonly data: T | undefined;
	readonly error: string | null;
	readonly loading: boolean;
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
 */
export function createResource<T>(
	load: (client: ApiClient, scope: Scope) => Promise<T>,
	topics: readonly string[] = []
): Resource<T> {
	let data = $state<T | undefined>(undefined);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let latest = 0;
	let settled = 0;
	let reloadAgain = false;
	let pause: ReturnType<typeof setTimeout> | null = null;

	function run(scope: Scope) {
		const request = ++latest;
		loading = true;
		const done = () => {
			settled = request;
			if (reloadAgain && request === latest) {
				reloadAgain = false;
				refresh();
			}
		};
		load(app.client, scope).then(
			(value) => {
				if (request !== latest) return;
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
		refresh
	};
}
