<script lang="ts">
	import type { Scope } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import GatedButton from './gated-button.svelte';

	/** One line, e.g. "Warehouse synced 4m ago [Sync now]"; a run ending syncs it too. */
	type Props = { scope: Scope };
	let { scope }: Props = $props();

	const sync = createResource((client) => client.getWarehouseSync(scope), ['warehouse_sync']);
	let requesting = $state(false);
	/**
	 * The sync report this page saw when it asked, compared by value rather than by clock, since browser and
	 * server clocks differ. Any newer report ends the wait; so does the request's five-minute expiry.
	 */
	let requested = $state<{ at: number; seen: string } | null>(null);
	let failure = $state<string | null>(null);
	const last = $derived(sync.data ?? null);
	const reportOf = (report: typeof last) => `${report?.lastSyncedAt ?? ''}|${report?.lastError ?? ''}`;
	const pending = $derived(requested !== null && reportOf(last) === requested.seen && app.now - requested.at < 5 * 60_000);

	async function request() {
		requesting = true;
		failure = null;
		try {
			const seen = { at: Date.now(), seen: reportOf(last) };
			await app.client.requestWarehouseSync(scope);
			requested = seen;
		} catch (error) {
			failure = errorMessage(error);
		} finally {
			requesting = false;
		}
	}
</script>

<div class="sync-now">
	<span class="lead">Warehouse sync</span>
	{#if pending}
		<span title="The location reads SQLBuild's state and catalog from the warehouse"><StatusLabel tone="var(--sb-running)" label="syncing…" /></span>
	{:else if last?.lastError}
		<span title={last.lastError}><StatusLabel tone="var(--sb-error)" label="failing{last.lastSyncedAt ? ` · last synced ${formatAgo(last.lastSyncedAt, app.now)}` : ''}" /></span>
	{:else if last?.lastSyncedAt}
		<span title="Synced {formatStamp(last.lastSyncedAt)}. A sync also follows every run that ends."><StatusLabel tone="var(--sb-success)" label="synced {formatAgo(last.lastSyncedAt, app.now)}" /></span>
	{:else if last || sync.error}
		<StatusLabel tone="var(--sb-text-faint)" label="not synced yet" muted />
	{/if}
	{#if failure}<span class="err" title={failure}>{failure}</span>{/if}
	<GatedButton requires="connection.test" busy={requesting || pending} onclick={request} title="Read what's been built, checks and freshness from the warehouse now">Sync now</GatedButton>
</div>

<style>
	.sync-now {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		margin-left: 12px;
	}
	.lead {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.err {
		overflow: hidden;
		color: var(--sb-error);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
