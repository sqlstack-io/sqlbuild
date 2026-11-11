<script lang="ts">
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';

	// The location syncs SQLBuild's own tables from the warehouse after each run ends and on request;
	// the shell only says when that last happened.
	const sync = $derived(app.warehouseSync);

	const text = $derived.by(() => {
		if (!sync) return '';
		if (sync.lastError) return `Warehouse sync failing · last synced ${formatAgo(sync.lastSyncedAt, app.now)}`;
		return sync.lastSyncedAt ? `Warehouse last synced ${formatAgo(sync.lastSyncedAt, app.now)}` : 'Warehouse not synced yet';
	});

	const title = $derived.by(() => {
		if (!sync) return '';
		const what = "What's been built, checks and freshness, read back from the warehouse.";
		return sync.lastError ? `${sync.lastError}\n${what}` : sync.lastSyncedAt ? `${what} Last synced ${formatStamp(sync.lastSyncedAt)}.` : what;
	});
</script>

<!-- Holds the usual text's width while loading, so the top bar doesn't shift when the report arrives. -->
<span class="sync" class:failed={!!sync?.lastError} class:waiting={!sync} {title}>{sync ? text : 'Warehouse last synced just now'}</span>

<style>
	.sync {
		color: var(--muted-foreground);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		display: inline-block;
		min-width: 14.6em;
		text-align: right;
	}
	.sync.waiting {
		visibility: hidden;
	}
	.sync.failed {
		color: var(--sb-error);
	}
</style>
