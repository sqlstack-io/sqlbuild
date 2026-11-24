<script lang="ts">
	import type { CostPrice } from '$lib/api';
	import { costTitle, dayLabel, money } from '../utils/format';
	import { monotonePath, plot } from '../utils/smooth';

	// A level's cost per day as a smooth area, drawn like the rows' trends; each day's exact cost on
	// hover, with a line down from the day.
	let { dates, values, price, height = 56 }: { dates: string[]; values: number[]; price: CostPrice | null; height?: number } = $props();
	const W = 1000;
	const points = $derived(plot(values, W, height, 4));
	const line = $derived(monotonePath(points));
	const step = $derived(values.length > 1 ? W / (values.length - 1) : W);
	const max = $derived(Math.max(...values, 0));
	const title = (i: number) => `${dayLabel(dates[i] ?? '', true)} · ${money(values[i], price)}${price ? ` · ${costTitle(values[i], price)}` : ''}`;
</script>

<div class="trend-area" role="img" aria-label="Cost per day">
	<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" style:height="{height}px">
		<path class="area" d="{line} L{W},{height} L0,{height} Z" />
		<path class="line" d={line} vector-effect="non-scaling-stroke" />
		{#each values as _, i (i)}
			{@const x = Math.max(0, i * step - step / 2)}
			<g class="day">
				<rect {x} y="0" width={Math.min(step, W - x)} {height}><title>{title(i)}</title></rect>
				<line x1={points[i][0]} x2={points[i][0]} y1={points[i][1]} y2={height} vector-effect="non-scaling-stroke" />
			</g>
		{/each}
	</svg>
	<div class="axis"><span>{dayLabel(dates[0] ?? '', true)}</span><span>{money(max, price)} peak</span><span>{dayLabel(dates.at(-1) ?? '', true)}</span></div>
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
