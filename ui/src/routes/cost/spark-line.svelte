<script lang="ts">
	// A row's cost per day as a small line, the last day marked.
	let { values, title }: { values: number[]; title?: string } = $props();
	const W = 84;
	const H = 18;
	const max = $derived(Math.max(...values, 0) || 1);
	const points = $derived(values.map((v, i) => `${values.length > 1 ? (i / (values.length - 1)) * W : W / 2},${H - 1 - (v / max) * (H - 2)}`));
</script>

<svg class="spark" width={W} height={H} viewBox="0 0 {W} {H}" role="img" aria-label={title}>
	<title>{title}</title>
	<polyline points={points.join(' ')} />
	{#if points.length}<circle cx={points.at(-1)!.split(',')[0]} cy={points.at(-1)!.split(',')[1]} r="1.8" />{/if}
</svg>

<style>
	.spark { display: inline-block; overflow: visible; vertical-align: middle; }
	polyline { fill: none; stroke: var(--primary); stroke-width: 1.3; stroke-linejoin: round; }
	circle { fill: var(--primary); }
</style>
