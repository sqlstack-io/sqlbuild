<script lang="ts">
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import HealthTable from '$lib/components/shell/health-table.svelte';
	import Loading from '$lib/components/loading.svelte';
	import { formatStamp } from '$lib/format/time';
	import { health } from '$lib/state/health.svelte';
</script>

<!-- Public: the shell shows it without a session, and monitoring can read the same /api/health. -->
<AppTopbar title="Status" />
<div class="status" data-testid="status-page">
	{#if health.unreachable}
		<p class="reason" data-testid="status-reason">{health.unreachable.reason}</p>
	{/if}
	{#if health.report}
		<p class="muted">Checked {formatStamp(health.report.checkedAt)} · the server reports this at <code>/api/health</code>, without signing in.</p>
		<HealthTable report={health.report} details={health.details} />
	{:else if !health.unreachable}
		<div class="w-60"><Loading /></div>
	{/if}
</div>

<style>
	.status {
		max-width: 1040px;
		overflow: auto;
		padding: 20px;
	}
	.muted {
		margin-bottom: 12px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.reason {
		margin-bottom: 12px;
		font-size: 13.5px;
	}
</style>
