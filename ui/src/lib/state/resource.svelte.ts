import type { ApiClient, Scope } from '$lib/api';
import { channelMatches } from '$lib/api';
import { app } from './app.svelte';

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

	function run(scope: Scope) {
		const request = ++latest;
		loading = true;
		load(app.client, scope).then(
			(value) => {
				if (request !== latest) return;
				data = value;
				error = null;
				loading = false;
			},
			(reason: unknown) => {
				if (request !== latest) return;
				error = reason instanceof Error ? reason.message : String(reason);
				loading = false;
			}
		);
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
		return app.client.changes.subscribe((signal) => {
			if (channelMatches(signal.channel, topics)) refresh();
		});
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
