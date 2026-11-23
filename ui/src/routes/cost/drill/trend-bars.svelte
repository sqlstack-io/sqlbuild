<script lang="ts">
	import type { CostPage } from '../types';
	import { costTitle, dayLabel, usd } from '../utils';

	// The level's cost per day as small bars, each with its day and cost on hover.
	let { cost, values, height = 56 }: { cost: CostPage; values: number[]; height?: number } = $props();
	const max = $derived(Math.max(...values, 0) || 1);
</script>

<div class="trend" style:--h="{height}px" style:--n={values.length} aria-label="Cost per day">
	<div class="bars">
		{#each values as v, i (i)}
			<i style:height="{Math.max(v > 0 ? 2 : 0, (v / max) * 100)}%" title="{dayLabel(cost.daily[i]?.date ?? '', true)} · {usd(v, cost.price)} · {costTitle(v, cost.price)}"></i>
		{/each}
	</div>
	<div class="axis"><span>{dayLabel(cost.daily[0]?.date ?? '', true)}</span><span>{usd(max, cost.price)} peak</span><span>{dayLabel(cost.daily.at(-1)?.date ?? '', true)}</span></div>
</div>

<style>
	.trend { min-width: 0; }
	.bars { display: grid; height: var(--h); grid-template-columns: repeat(var(--n), minmax(0, 1fr)); align-items: end; gap: 2px; border-bottom: 1px solid var(--border); }
	.bars i { display: block; border-radius: 1px 1px 0 0; background: color-mix(in srgb, var(--primary) 75%, transparent); }
	.bars i:hover { background: var(--primary); }
	.axis { display: flex; justify-content: space-between; margin-top: 4px; color: var(--sb-text-faint); font-size: 11px; }
</style>
