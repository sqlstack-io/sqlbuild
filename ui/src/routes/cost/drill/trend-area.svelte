<script lang="ts">
	import type { CostPage } from '../types';
	import { costTitle, dayLabel, monotonePath, plot, usd } from '../utils';

	// The level's cost per day as a smooth area, drawn like the rows' trends; each day's exact cost
	// on hover, with a dot on the day.
	let { cost, values, height = 56 }: { cost: CostPage; values: number[]; height?: number } = $props();
	const W = 1000;
	const points = $derived(plot(values, W, height, 4));
	const line = $derived(monotonePath(points));
	const step = $derived(values.length > 1 ? W / (values.length - 1) : W);
	const max = $derived(Math.max(...values, 0));
	const title = (i: number) => `${dayLabel(cost.daily[i]?.date ?? '', true)} · ${usd(values[i], cost.price)} · ${costTitle(values[i], cost.price)}`;
</script>

<div class="trend-area" aria-label="Cost per day">
	<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" style:height="{height}px">
		<path class="area" d="{line} L{W},{height} L0,{height} Z" />
		<path class="line" d={line} vector-effect="non-scaling-stroke" />
		{#each values as _, i (i)}
			<g class="day">
				<rect x={Math.max(0, i * step - step / 2)} y="0" width={step} height={height}><title>{title(i)}</title></rect>
				<line x1={points[i][0]} x2={points[i][0]} y1={points[i][1]} y2={height} vector-effect="non-scaling-stroke" />
			</g>
		{/each}
	</svg>
	<div class="axis"><span>{dayLabel(cost.daily[0]?.date ?? '', true)}</span><span>{usd(max, cost.price)} peak</span><span>{dayLabel(cost.daily.at(-1)?.date ?? '', true)}</span></div>
</div>

<style>
	.trend-area { min-width: 0; }
	svg { display: block; width: 100%; border-bottom: 1px solid var(--border); }
	.area { fill: color-mix(in srgb, var(--primary) 18%, transparent); }
	.line { fill: none; stroke: var(--primary); stroke-width: 1.6; stroke-linejoin: round; }
	rect { fill: transparent; }
	.day line { stroke: transparent; stroke-width: 1; }
	.day:hover rect { fill: color-mix(in srgb, var(--primary) 8%, transparent); }
	.day:hover line { stroke: var(--primary); }
	.axis { display: flex; justify-content: space-between; margin-top: 4px; color: var(--sb-text-faint); font-size: 11px; }
</style>
