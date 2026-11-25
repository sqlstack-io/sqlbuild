<script lang="ts">
	import { formatAgo, formatClock, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { health } from '$lib/state/health.svelte';
	import HealthTable from './health-table.svelte';

	/** The page the person asked for, which loads by itself once the server can serve it again. */
	let { page }: { page: string } = $props();
	const outage = $derived(health.outage);
</script>

<div class="outage" role="status" aria-live="polite" data-testid="outage">
	<h1>{page} isn't available</h1>
	{#if outage}
		<p class="reason" data-testid="outage-reason">{outage.reason}</p>
		<p class="muted">
			{#if outage.since}<span title={formatStamp(outage.since)}>Since {formatClock(outage.since, app.now)} ({formatAgo(outage.since, app.now)})</span> · {/if}Checking again every {health.retrySeconds} s; this page loads by itself when the server is back.
		</p>
	{/if}
	{#if health.report}
		<div class="table"><HealthTable report={health.report} details={health.details} /></div>
	{/if}
</div>

<style>
	.outage {
		max-width: 920px;
		padding: 40px 32px;
	}
	h1 {
		font-size: 15px;
		font-weight: 600;
	}
	.reason {
		margin-top: 10px;
		font-size: 13.5px;
	}
	.muted {
		margin-top: 4px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.table {
		margin-top: 24px;
	}
</style>
