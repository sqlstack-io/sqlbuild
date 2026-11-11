<script lang="ts">
	/** Placeholder lines a section shows while its read has nothing to show yet. */
	let { lines = 3 }: { lines?: number } = $props();
	const widths = [68, 52, 60, 44, 56, 40];
</script>

<div class="loading" role="status" aria-label="Loading">
	{#each Array.from({ length: lines }, (_, i) => i) as i (i)}
		<span class="sb-skeleton" style:width="{widths[i % widths.length]}%"></span>
	{/each}
</div>

<style>
	.loading {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 4px 0;
	}
</style>
