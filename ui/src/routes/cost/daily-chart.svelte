<script lang="ts">
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import type { CostPage } from './types';
	import { OTHER, costTitle, dayLabel, stacks, usd } from './utils';

	// Cost per day, stacked by job or by warehouse size; the six largest keep their own colour.
	let { cost }: { cost: CostPage } = $props();
	let by = $state<'byJob' | 'bySize'>('byJob');
	const stack = $derived(stacks(cost.daily, by));
	const COLOURS = ['var(--primary)', 'var(--sb-secondary)', 'var(--sb-stale)', 'var(--sb-warning)', 'var(--sb-behind)', 'var(--sb-success)'];
	const colour = (key: string) => (key === OTHER ? 'color-mix(in srgb, var(--muted-foreground) 22%, transparent)' : COLOURS[stack.keys.indexOf(key) % COLOURS.length]);
	// Three gridlines at round dollar amounts.
	const step = $derived.by(() => {
		const max = stack.max * cost.price.usdPerCredit;
		const raw = max / 3;
		const mag = 10 ** Math.floor(Math.log10(raw || 1));
		return ([1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw) / cost.price.usdPerCredit;
	});
	const ticks = $derived([1, 2, 3].map((i) => i * step).filter((t) => t <= stack.max * 1.05));
	const ceil = $derived(Math.max(stack.max, ticks.at(-1) ?? 0));
	const dayTitle = (d: (typeof stack.days)[number]) =>
		[`${dayLabel(d.date, true)} · ${usd(d.total, cost.price)}`, ...d.parts.filter((p) => p.credits > 0).map((p) => `${p.key}: ${usd(p.credits, cost.price)}`)].join('\n');
</script>

<Panel title="Cost per day" note="{cost.daily.length} days">
	{#snippet actions()}
		<SegmentedFilter label="Stack by" bind:value={by} options={[{ value: 'byJob', label: 'Job' }, { value: 'bySize', label: 'Warehouse size' }]} />
	{/snippet}
	<div class="chart">
		<div class="plot" style:--n={stack.days.length}>
			{#each ticks as t (t)}
				<span class="grid" style:bottom="{(t / ceil) * 100}%"><b>{usd(t, cost.price)}</b></span>
			{/each}
			{#each stack.days as d (d.date)}
				<div class="day" title={dayTitle(d)}>
					<div class="stack" style:height="{(d.total / ceil) * 100}%">
						{#each [...d.parts].reverse() as p (p.key)}
							<i style:flex-grow={p.credits} style:background={colour(p.key)}></i>
						{/each}
					</div>
					<span class="x">{dayLabel(d.date, stack.days.length <= 7)}</span>
				</div>
			{/each}
		</div>
		<ul class="legend">
			{#each stack.keys as k (k)}
				{@const total = stack.days.reduce((t, d) => t + (d.parts.find((p) => p.key === k)?.credits ?? 0), 0)}
				<li title={costTitle(total, cost.price)}><i style:background={colour(k)}></i><span class="name">{k}</span><span class="v">{usd(total, cost.price)}</span></li>
			{/each}
		</ul>
	</div>
</Panel>

<style>
	.chart { display: grid; grid-template-columns: minmax(0, 1fr) 250px; gap: 28px; padding: 14px 0 4px; border-top: 1px solid var(--border); }
	.plot { position: relative; display: grid; height: 190px; grid-template-columns: repeat(var(--n), minmax(0, 1fr)); align-items: end; gap: 4px; margin: 0 0 22px 56px; border-bottom: 1px solid var(--border); }
	.grid { position: absolute; right: 0; left: -56px; border-top: 1px dashed var(--inner-line); pointer-events: none; }
	.grid b { position: absolute; top: -8px; left: 0; width: 40px; color: var(--sb-text-faint); font-size: 11px; font-weight: 400; text-align: right; font-variant-numeric: tabular-nums; }
	.day { position: relative; display: flex; height: 100%; flex-direction: column; justify-content: flex-end; align-items: center; }
	.day:hover .stack { filter: brightness(1.2); }
	.stack { display: flex; width: 100%; max-width: 44px; flex-direction: column; overflow: hidden; border-radius: 2px 2px 0 0; }
	.stack i { display: block; min-height: 0; flex-basis: 0; }
	.x { position: absolute; bottom: -20px; color: var(--sb-text-faint); font-size: 11px; white-space: nowrap; }
	.legend { display: flex; flex-direction: column; gap: 6px; margin: 0; padding: 0; list-style: none; font-size: 12.5px; }
	.legend li { display: grid; grid-template-columns: 10px minmax(0, 1fr) auto; align-items: center; gap: 8px; }
	.legend i { width: 10px; height: 10px; border-radius: 2px; }
	.legend .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.legend .v { font-variant-numeric: tabular-nums; }
</style>
