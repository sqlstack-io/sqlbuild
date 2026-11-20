import type { Check, Scope } from '$lib/api';
import { app } from './app.svelte';
import { ChecksFeed, type ChecksView } from './checks-feed';
import type { Resource } from './resource.svelte';

/** The selected location's checks, one copy for the Checks page, the plan page and the node view. */
class SharedChecks {
	view = $state.raw<ChecksView>({ checks: undefined, error: null, loading: false, revision: 0 });
	#feed = new ChecksFeed(
		() => app.client,
		(view) => (this.view = view)
	);
	#listening = false;

	/** Starts reading the scope's checks, such as from a route prefetch, unless they are held and current. */
	ensure(scope: Scope): void {
		this.#listen();
		this.#feed.ensure(scope);
	}

	/** A mounted page showing the checks; the returned function is its unmount. */
	watch(scope: Scope): () => void {
		this.#listen();
		return this.#feed.watch(scope);
	}

	#listen() {
		if (this.#listening) return;
		this.#listening = true;
		app.client.changes.subscribe((signal) => this.#feed.handle(signal));
	}

	refresh(): void {
		this.#feed.refresh();
	}
}

export const sharedChecks = new SharedChecks();

/**
 * The shared checks as a page resource, with the count of reads that have answered (`revision`), which
 * a page can follow to reload once per read. Must be called during component initialisation; the page
 * watches the checks of the selected scope while it is mounted.
 */
export function useChecks(): Resource<Check[]> & { readonly revision: number } {
	$effect(() => {
		const scope = app.scope;
		if (scope) return sharedChecks.watch(scope);
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
		get revision() {
			return sharedChecks.view.revision;
		},
		refresh: () => sharedChecks.refresh()
	};
}
