<script lang="ts">
	import type { Resource } from '$lib/state/resource.svelte';

	let { colspan, resource, empty }: { colspan: number; resource: Resource<unknown>; empty: string } = $props();
</script>

<tr class="load-row">
	{#if resource.error}
		<td {colspan} class="empty error">
			Couldn't load: {resource.error}
			<button onclick={() => resource.refresh()}>Retry</button>
		</td>
	{:else if resource.loading && resource.data === undefined}
		<td {colspan} class="empty">Loading…</td>
	{:else}
		<td {colspan} class="empty">{empty}</td>
	{/if}
</tr>

<style>
	:global(.sb-list tbody tr.load-row) {
		cursor: default;
	}
	.error {
		color: var(--sb-error);
	}
	button {
		margin-left: 8px;
		color: var(--primary);
	}
</style>
