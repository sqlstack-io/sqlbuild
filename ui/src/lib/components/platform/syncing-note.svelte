<script lang="ts">
	import { firstSyncPending, liveSyncOf } from '$lib/api/http-locations';
	import { createResource } from '$lib/state/resource.svelte';

	/** Shown above views that read the server's copy of the warehouse while the location's first sync runs. */
	type Props = { what: string };
	let { what }: Props = $props();

	const sync = createResource((client, scope) => (client.supports('getWarehouseSync') ? client.getWarehouseSync(scope) : Promise.resolve(null)), ['warehouse_sync']);
	const running = $derived(firstSyncPending(sync.data) ? liveSyncOf(sync.data) : null);
</script>

{#if running}
	<p class="syncing-note" role="status" title="{what} fill in when it completes">
		Syncing warehouse history · {running.position}/{running.kinds}
	</p>
{/if}

<style>
	.syncing-note {
		margin: 0 0 12px;
		padding: 8px 12px;
		border: 1px solid var(--border);
		border-radius: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
</style>
