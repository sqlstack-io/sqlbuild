<script lang="ts">
	// A row's cost per day as a filled line across its cell's width.
	let { values, label }: { values: number[]; label: string } = $props();
	const max = $derived(Math.max(...values, 0) || 1);
	const n = $derived(Math.max(1, values.length - 1));
	const line = $derived(values.map((v, i) => `${(i / n) * 100},${20 - (v / max) * 18}`).join(' '));
</script>

<svg class="spark" viewBox="0 0 100 20" preserveAspectRatio="none" role="img" aria-label={label}>
	<title>{label}</title>
	<polygon points="0,20 {line} 100,20" />
	<polyline points={line} vector-effect="non-scaling-stroke" />
</svg>

<style>
	.spark { display: block; width: 100%; max-width: 640px; height: 20px; }
	polygon { fill: color-mix(in srgb, var(--primary) 16%, transparent); }
	polyline { fill: none; stroke: var(--primary); stroke-width: 1.3; stroke-linejoin: round; }
</style>
