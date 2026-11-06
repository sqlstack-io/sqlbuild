<script lang="ts">
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';

	// The location syncs SQLBuild's own tables from the warehouse on its own schedule;
	// the shell only says when that last happened.
	const sync = $derived(app.warehouseSync);

	const text = $derived.by(() => {
		if (!sync) return 'Warehouse sync —';
		if (sync.lastError) return `Warehouse sync failing · last synced ${formatAgo(sync.lastSyncedAt, app.now)}`;
		return sync.lastSyncedAt ? `Warehouse last synced ${formatAgo(sync.lastSyncedAt, app.now)}` : 'Warehouse not synced yet';
	});

	const title = $derived.by(() => {
		if (!sync) return '';
		const what = "What's been built, checks and freshness, read back from the warehouse.";
		return sync.lastError ? `${sync.lastError}\n${what}` : sync.lastSyncedAt ? `${what} Last synced ${formatStamp(sync.lastSyncedAt)}.` : what;
	});
</script>

<span class="sync" class:failed={!!sync?.lastError} {title}>{text}</span>

<style>
	.sync {
		color: var(--muted-foreground);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.sync.failed {
		color: var(--sb-error);
	}
</style>
