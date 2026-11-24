<script lang="ts">
	import { monotonePath, plot } from '../utils/smooth';

	// A row's cost per day as a smooth filled line across its cell. The day under the pointer gives
	// the tooltip, worked out as the pointer moves, so a row draws two paths, not one shape per day.
	let { values, label, dayTitle }: { values: readonly number[]; label: string; dayTitle?: (day: number, value: number) => string } = $props();
	const H = 20;
	const points = $derived(plot(values, 100, H, 2));
	const line = $derived(monotonePath(points));
	const area = $derived(points.length ? `${line} L100,${H} L0,${H} Z` : '');
	let title = $state<string | null>(null);
	function onpointermove(e: PointerEvent) {
		if (!dayTitle || values.length === 0) return;
		const box = (e.currentTarget as SVGElement).getBoundingClientRect();
		const day = Math.min(values.length - 1, Math.max(0, Math.round(((e.clientX - box.left) / Math.max(1, box.width)) * (values.length - 1))));
		title = dayTitle(day, values[day]);
	}
</script>

<svg class="spark" viewBox="0 0 100 {H}" preserveAspectRatio="none" role="img" aria-label={label} {onpointermove}>
	<title>{title ?? label}</title>
	<path class="area" d={area} />
	<path class="line" d={line} vector-effect="non-scaling-stroke" />
</svg>

<style>
	.spark { display: block; width: 100%; height: 20px; }
	.area { fill: color-mix(in srgb, var(--primary) 16%, transparent); }
	.line { fill: none; stroke: var(--primary); stroke-width: 1.3; stroke-linejoin: round; }
</style>
