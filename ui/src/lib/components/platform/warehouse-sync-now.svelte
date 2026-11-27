<script lang="ts">
	import type { Scope } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage } from '$lib/platform/labels';
	import { syncNowState, syncReportKey, type SyncRequest } from '$lib/platform/sync-now';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import GatedButton from './gated-button.svelte';

	/** One line, e.g. "Warehouse synced 4m ago [Sync now]"; a run ending syncs it too. */
	type Props = { scope: Scope };
	let { scope }: Props = $props();

	const sync = createResource((client) => client.getWarehouseSync(scope), ['warehouse_sync']);
	let requesting = $state(false);
	/** Any newer sync report ends the wait; so does the request's five-minute expiry. */
	let requested = $state<SyncRequest | null>(null);
	let failure = $state<string | null>(null);
	const last = $derived(sync.data ?? null);
	const status = $derived(syncNowState(last, requested, app.now));
	const summary = $derived(last?.completed ? ` Last full sync: ${last.completed.kinds} kinds, ${last.completed.rows.toLocaleString('en-US')} rows in ${(last.completed.elapsedMs / 1000).toFixed(1)} s.` : '');

	async function request() {
		requesting = true;
		failure = null;
		try {
			const seen = { at: Date.now(), seen: syncReportKey(last) };
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
	{#if status.kind === 'requested'}
		<span title="The location reads SQLBuild's state and catalog from the warehouse"><StatusLabel tone="var(--sb-running)" label="syncing…" /></span>
	{:else if status.kind === 'failing'}
		<span title={status.error}><StatusLabel tone="var(--sb-error)" label="failing{status.lastSyncedAt ? ` · last synced ${formatAgo(status.lastSyncedAt, app.now)}` : ''}" /></span>
	{:else if status.kind === 'running'}
		<span title="The location reads SQLBuild's state and catalog from the warehouse"><StatusLabel tone="var(--sb-running)" label="syncing history {status.running.position}/{status.running.kinds} · {status.running.table.replaceAll('_', ' ')} · {status.running.rows.toLocaleString('en-US')} rows" /></span>
	{:else if status.kind === 'synced'}
		<span title="Synced {formatStamp(status.at)}. A sync also follows every run that ends.{summary}"><StatusLabel tone="var(--sb-success)" label="synced {formatAgo(status.at, app.now)}" /></span>
	{:else if last || sync.error}
		<StatusLabel tone="var(--sb-text-faint)" label="not synced yet" muted />
	{/if}
	{#if failure}<span class="err" title={failure}>{failure}</span>{/if}
	<GatedButton requires="connection.test" busy={requesting || status.kind === 'requested'} onclick={request} title="Read what's been built, checks and freshness from the warehouse now">Sync now</GatedButton>
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
