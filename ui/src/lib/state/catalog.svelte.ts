import type { CatalogNode, Scope } from '$lib/api';
import { app } from './app.svelte';
import { CatalogFeed, type CatalogView } from './catalog-feed';
import type { Resource } from './resource.svelte';

/**
 * The selected location's catalog nodes, one copy for every page: lineage, catalog, node view,
 * job view, plan, rerun and the overview read it here instead of each reading the listing.
 */
class CatalogNodes {
	view = $state.raw<CatalogView>({ nodes: undefined, error: null, loading: false });
	#feed = new CatalogFeed(
		() => app.client,
		(view) => (this.view = view)
	);
	#listening = false;

	/** Starts reading the scope's nodes, such as from a route prefetch, unless they are held. */
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

export const catalogNodes = new CatalogNodes();

/**
 * The shared catalog nodes as a page resource. Must be called during component initialisation;
 * reading starts for the selected scope and follows it.
 */
export function useCatalogNodes(): Resource<CatalogNode[]> {
	$effect(() => {
		const scope = app.scope;
		if (scope) catalogNodes.ensure(scope);
	});
	return {
		get data() {
			return catalogNodes.view.nodes;
		},
		get error() {
			return catalogNodes.view.error;
		},
		get loading() {
			return catalogNodes.view.loading;
		},
		refresh: () => catalogNodes.refresh()
	};
}
