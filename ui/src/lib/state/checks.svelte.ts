import type { Check, Scope } from '$lib/api';
import { app } from './app.svelte';
import { ChecksFeed, type ChecksView } from './checks-feed';
import type { Resource } from './resource.svelte';

/** The selected location's checks, one copy for the Checks page, the plan page and the node view. */
class SharedChecks {
	view = $state.raw<ChecksView>({ checks: undefined, error: null, loading: false });
	#feed = new ChecksFeed(
		() => app.client,
		(view) => (this.view = view)
	);
	#listening = false;

	/** Starts reading the scope's checks, such as from a route prefetch, unless they are held. */
	ensure(scope: Scope): void {
		if (!this.#listening) {
			this.#listening = true;
			app.client.changes.subscribe((signal) => this.#feed.handle(signal));
		}
		this.#feed.ensure(scope);
	}

	refresh(): void {
		this.#feed.refresh();
	}
}

export const sharedChecks = new SharedChecks();

/**
 * The shared checks as a page resource. Must be called during component initialisation; reading
 * starts for the selected scope and follows it.
 */
export function useChecks(): Resource<Check[]> {
	$effect(() => {
		const scope = app.scope;
		if (scope) sharedChecks.ensure(scope);
	});
	return {
		get data() {
			return sharedChecks.view.scopeKey === app.scope?.locationId ? sharedChecks.view.checks : undefined;
		},
		get error() {
			return sharedChecks.view.error;
		},
		get loading() {
			return sharedChecks.view.loading;
		},
		refresh: () => sharedChecks.refresh()
	};
}
