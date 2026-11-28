<script lang="ts">
	import type { Scope } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage, syncWords, type ActionWords } from '$lib/platform/labels';
	import { syncNowState, syncReportKey, type SyncRequest } from '$lib/platform/sync-now';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import ActionStatus from './action-status.svelte';

	/** "Warehouse sync ● synced 4m ago [Sync now]"; a run ending syncs it too. */
	/** `problemsHref` is where a failing or stalled sync is explained in full, on the location's page its Problems list. */
	/** `blocked`: the connection check failed, so a failing sync waits on it and the connection alone carries the red. */
	type Props = { scope: Scope; problemsHref?: string | null; blocked?: boolean };
	let { scope, problemsHref = null, blocked = false }: Props = $props();

	const sync = createResource((client) => client.getWarehouseSync(scope), ['warehouse_sync']);
	let requesting = $state(false);
	/** Any newer sync report ends the wait; so does the request's five-minute expiry. */
	let requested = $state<SyncRequest | null>(null);
	/** A request the server refused at once, with why, and when. */
	let refused = $state<{ at: number; cause: string } | null>(null);
	const last = $derived(sync.data ?? null);
	const status = $derived(syncNowState(last, requested, app.now));
	const summary = $derived(last?.completed ? `\nLast full sync: ${last.completed.kinds} kinds, ${last.completed.rows.toLocaleString('en-US')} rows in ${(last.completed.elapsedMs / 1000).toFixed(1)} s.` : '');
	const words = $derived.by((): ActionWords => {
		if (refused && !requesting) return { tone: 'failed', label: `couldn’t request · ${formatAgo(new Date(refused.at).toISOString(), Math.max(app.now, refused.at))}`, title: refused.cause };
		// copy-allow: the product owner's wording for a sync waiting on a failed connection check
		if (blocked && status.kind === 'failing') return { tone: 'never', label: 'blocked: can’t connect', title: 'The sync can’t run until the connection check passes.' };
		const shown = syncWords(status, app.now);
		return status.kind === 'synced' ? { ...shown, title: `${shown.title}${summary}` } : shown;
	});
	const cause = $derived(refused && !requesting ? refused.cause : null);
	const href = $derived(!refused && !blocked && (status.kind === 'failing' || status.kind === 'stalled') ? problemsHref : null);
	const waiting = $derived(requesting || status.kind === 'requested' || status.kind === 'running');
	const since = $derived(
		status.kind === 'requested'
			? `Requested ${status.by ? `by ${status.by} ` : ''}at ${formatStamp(new Date(status.at).toISOString())}; waiting for the location.`
			: status.kind === 'running'
				? `Running${status.running.startedAt ? ` since ${formatStamp(status.running.startedAt)}` : ''}; it finishes on its own.`
				: 'Sending the request…'
	);

	async function request() {
		requesting = true;
		refused = null;
		try {
			const seen = { at: Date.now(), seen: syncReportKey(last) };
			await app.client.requestWarehouseSync(scope);
			requested = seen;
		} catch (error) {
			refused = { at: Date.now(), cause: errorMessage(error) };
		} finally {
			requesting = false;
		}
	}
</script>

{#if last || sync.error}
	<ActionStatus
		lead="Warehouse sync"
		{words}
		{cause}
		{href}
		testid="sync-now"
		action={{ label: 'Sync now', requires: 'connection.test', onclick: request, busy: waiting, busyTitle: since, title: "Read what's been built, checks and freshness from the warehouse now" }}
	/>
{/if}
