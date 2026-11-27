<script lang="ts">
	import { plural } from '$lib/format/plural';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { to } from '$lib/paths';
	import { severityTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';

	// One quiet fact beside the location's name: how many problems it has open, linking to where they
	// are listed in full, or else when its run history last synced from the warehouse. The details,
	// errors included, live on the location's page, never here.
	const sync = $derived(app.warehouseSync);
	const open = $derived(app.platformEvents.filter((e) => e.active && e.severity !== 'info' && e.locationId === app.locationId));
	const worst = $derived(open.find((e) => e.severity === 'critical') ?? open[0] ?? null);
	const failing = $derived(!!sync?.failing || !!sync?.lastError);

	const text = $derived.by(() => {
		if (!sync) return '';
		if (failing) return 'history sync failing';
		return sync.lastSyncedAt ? `history synced ${formatAgo(sync.lastSyncedAt, app.now)}` : 'history not synced yet';
	});
	const title = $derived(
		sync?.lastSyncedAt
			? `What's been built, checks and freshness, read back from the warehouse. Last synced ${formatStamp(sync.lastSyncedAt)}.`
			: "What's been built, checks and freshness, read back from the warehouse."
	);
</script>

{#if worst && app.location}
	<a class="sync problems" href={to(`/locations/${app.location.id}#problems`)} title="Open problems of this location, listed on its page" data-testid="sync-status"
		><i style:background={severityTone[worst.severity]}></i>{plural(open.length, 'problem')}</a
	>
{:else if failing && app.location}
	<a class="sync problems" href={to(`/locations/${app.location.id}#problems`)} {title} data-testid="sync-status"><i style:background="var(--sb-error)"></i>{text}</a>
{:else}
	<!-- Holds the usual text's width while loading, so the top bar doesn't shift when the report arrives. -->
	<span class="sync" class:waiting={!sync} {title} data-testid="sync-status">{sync ? text : 'history synced just now'}</span>
{/if}

<style>
	.sync {
		display: inline-flex;
		min-width: 0;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.sync.waiting {
		visibility: hidden;
	}
	.problems:hover {
		color: var(--foreground);
	}
	i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
</style>
