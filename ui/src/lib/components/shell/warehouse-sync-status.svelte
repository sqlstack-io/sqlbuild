<script lang="ts">
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';

	// One neutral fact beside the location's name: when its run history last synced from the warehouse.
	// Problems, a failing sync among them, are the banner's, on one line under the top bar.
	const sync = $derived(app.warehouseSync);
	const text = $derived.by(() => {
		if (!sync) return '';
		if (sync.failing || sync.lastError) return sync.lastSyncedAt ? `history last synced ${formatAgo(sync.lastSyncedAt, app.now)}` : 'history not synced yet';
		return sync.lastSyncedAt ? `history synced ${formatAgo(sync.lastSyncedAt, app.now)}` : 'history not synced yet';
	});
	const title = $derived(
		sync?.lastSyncedAt
			? `What's been built, checks and freshness, read back from the warehouse. Last synced ${formatStamp(sync.lastSyncedAt)}.`
			: "What's been built, checks and freshness, read back from the warehouse."
	);
</script>

<!-- Holds the usual text's width while loading, so the top bar doesn't shift when the report arrives. -->
<span class="sync" class:waiting={!sync} {title} data-testid="sync-status">{sync ? text : 'history synced just now'}</span>

<style>
	.sync {
		min-width: 0;
		color: var(--muted-foreground);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.sync.waiting {
		visibility: hidden;
	}
</style>
