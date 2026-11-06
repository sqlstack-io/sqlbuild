<script lang="ts">
	let { value, exact }: { value: string; exact?: string } = $props();
</script>

<time class="relative-time" title={exact ?? value}>{value}</time>

<style>
	.relative-time {
		color: var(--muted-foreground);
		font-size: 12.5px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
</style>
