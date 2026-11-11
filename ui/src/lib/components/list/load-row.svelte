<script lang="ts">
	import type { Resource } from '$lib/state/resource.svelte';

	let { colspan, resource, empty }: { colspan: number; resource: Resource<unknown>; empty: string } = $props();
	const widths = [46, 34, 52, 28, 40];
</script>

{#if resource.error}
	<tr class="load-row">
		<td {colspan} class="empty error">
			Couldn't load: {resource.error}
			<button onclick={() => resource.refresh()}>Retry</button>
		</td>
	</tr>
{:else if resource.loading && resource.data === undefined}
	<!-- One bar per row: a table's declared columns can differ from the ones it shows. -->
	{#each widths as width, row (row)}
		<tr class="load-row" role={row === 0 ? 'status' : undefined} aria-label={row === 0 ? 'Loading' : undefined}>
			<td {colspan} class="bar"><span class="sb-skeleton" style:width="{width}%"></span></td>
		</tr>
	{/each}
{:else}
	<tr class="load-row">
		<td {colspan} class="empty">{empty}</td>
	</tr>
{/if}

<style>
	:global(.sb-list tbody tr.load-row) {
		cursor: default;
	}
	:global(.sb-list tbody tr.load-row:hover) {
		background: transparent;
	}
	.bar {
		padding-top: 16px;
		padding-bottom: 16px;
	}
	.error {
		color: var(--sb-error);
	}
	button {
		margin-left: 8px;
		color: var(--primary);
	}
</style>
