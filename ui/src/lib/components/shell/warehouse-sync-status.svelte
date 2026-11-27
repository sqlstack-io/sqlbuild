<script lang="ts">
	import { formatAgo, formatClock, formatIn, formatStamp } from '$lib/format/time';
	import { to } from '$lib/paths';
	import { app } from '$lib/state/app.svelte';
	import { syncLine } from './sync-line';

	// The location syncs SQLBuild's own run history from the warehouse after each run ends and on request;
	// the shell says when that last happened, or, while it fails, what stands, since when and how it retries.
	const sync = $derived(app.warehouseSync);
	const failing = $derived(sync?.failing ?? null);

	const text = $derived.by(() => {
		if (!sync) return '';
		if (failing) return syncLine(failing, formatClock(failing.since, app.now));
		if (sync.lastError) return `history sync failing · last synced ${formatAgo(sync.lastSyncedAt, app.now)}`;
		return sync.lastSyncedAt ? `history synced ${formatAgo(sync.lastSyncedAt, app.now)}` : 'history not synced yet';
	});

	const title = $derived.by(() => {
		if (!sync) return '';
		const what = "What's been built, checks and freshness, read back from the warehouse.";
		const last = sync.lastSyncedAt ? `Last synced ${formatStamp(sync.lastSyncedAt)}.` : 'Not synced yet.';
		if (failing) {
			const next = failing.halted ? 'Stopped: resume it from the Overview or the location page.' : failing.retryAt ? `Next attempt ${formatIn(failing.retryAt, app.now)}.` : '';
			return `${text}\n${last} ${next}\n${what}`;
		}
		return sync.lastError ? `${sync.lastError}\n${what}` : sync.lastSyncedAt ? `${what} ${last}` : what;
	});
</script>

<!-- Holds the usual text's width while loading, so the top bar doesn't shift when the report arrives. -->
{#if failing && app.location}
	<a class="sync failed" href={to(`/locations/${app.location.id}`)} {title} data-testid="sync-status">{text}</a>
{:else}
	<span class="sync" class:failed={!!sync?.lastError} class:waiting={!sync} {title} data-testid="sync-status">{sync ? text : 'history synced just now'}</span>
{/if}

<style>
	.sync {
		color: var(--muted-foreground);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		display: inline-block;
		min-width: 0;
		max-width: 52ch;
		overflow: hidden;
		text-overflow: ellipsis;
		vertical-align: middle;
	}
	.sync.waiting {
		visibility: hidden;
	}
	.sync.failed {
		color: var(--sb-error);
	}
</style>
