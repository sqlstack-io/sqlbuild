<script lang="ts">
	import { monotonePath, plot } from '../utils';

	// A row's cost per day as a filled line across its cell's width. `smooth` draws a monotone curve
	// through the same points; `titles` gives each day's exact value on hover.
	let { values, label, smooth = false, titles, fill = false }: { values: number[]; label: string; smooth?: boolean; titles?: string[]; fill?: boolean } = $props();
	const H = 20;
	const points = $derived(plot(values, 100, H, 2));
	const line = $derived(smooth ? monotonePath(points) : `M${points.map((p) => p.join(',')).join(' L')}`);
	const area = $derived(points.length ? `${line} L100,${H} L0,${H} Z` : '');
	const step = $derived(values.length > 1 ? 100 / (values.length - 1) : 100);
</script>

<svg class="spark" class:fill viewBox="0 0 100 {H}" preserveAspectRatio="none" role="img" aria-label={label}>
	{#if !titles}<title>{label}</title>{/if}
	<path class="area" d={area} />
	<path class="line" d={line} vector-effect="non-scaling-stroke" />
	{#if titles}
		{#each titles as t, i (i)}
			<rect x={Math.max(0, i * step - step / 2)} y="0" width={step} height={H}><title>{t}</title></rect>
		{/each}
	{/if}
</svg>

<style>
	.spark { display: block; width: 100%; max-width: 640px; height: 20px; }
	.spark.fill { max-width: none; }
	.area { fill: color-mix(in srgb, var(--primary) 16%, transparent); }
	.line { fill: none; stroke: var(--primary); stroke-width: 1.3; stroke-linejoin: round; }
	rect { fill: transparent; }
	rect:hover { fill: color-mix(in srgb, var(--primary) 14%, transparent); }
</style>
