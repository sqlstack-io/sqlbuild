<script lang="ts">
	import type { NodeBuildCost } from '$lib/api';
	import { formatStamp } from '$lib/format/time';
	import { formatQueryCost } from '$lib/operate/cost';
	import { formatMs } from '$lib/build-area/labels';

	// One bar per build, oldest on the left: estimated query cost above, query time below.
	// Builds whose cost is still pending or unavailable keep their place as an empty slot.
	let { costs, days }: { costs: NodeBuildCost[]; days: number } = $props();
	const builds = $derived([...costs].reverse());
	const counted = $derived(builds.filter((c) => c.status === 'complete' || c.status === 'partial'));
	const maxCredits = $derived(Math.max(...counted.map((c) => c.credits), 0) || 1);
	const maxSeconds = $derived(Math.max(...counted.map((c) => c.seconds), 0) || 1);
	const total = $derived(counted.reduce((t, c) => ({ credits: t.credits + c.credits, usd: t.usd === null || c.usd === null ? null : t.usd + c.usd }), { credits: 0, usd: 0 as number | null }));
	const average = $derived(counted.length ? { credits: total.credits / counted.length, usd: total.usd === null ? null : total.usd / counted.length } : null);
	const label = (c: NodeBuildCost) =>
		`${formatStamp(c.startedAt)} · ${c.status === 'pending' ? 'cost pending' : c.status === 'complete' || c.status === 'partial' ? `${formatQueryCost(c)}${c.status === 'partial' ? ' (partial)' : ''} · ${formatMs(Math.round(c.seconds * 1000))} query time` : 'cost unavailable'}`;
</script>

<section class="chart" aria-label="Estimated query cost per build">
	<header>
		<h3>Estimated query cost per build</h3>
		<span>{counted.length} of {builds.length} builds in the last {days} days{#if average}&nbsp;· {formatQueryCost(average)} on average · {formatQueryCost(total)} in total{/if}</span>
	</header>
	<div class="strips" style:--n={builds.length}>
		<span class="axis">Cost</span>
		<div class="strip">
			{#each builds as c (c.buildRunId)}
				<i class:empty={!counted.includes(c)} class:partial={c.status === 'partial'} style:height="{counted.includes(c) ? Math.max(4, (c.credits / maxCredits) * 100) : 100}%" title={label(c)}></i>
			{/each}
		</div>
		<span class="axis">Query time</span>
		<div class="strip short">
			{#each builds as c (c.buildRunId)}
				<i class="time" class:empty={!counted.includes(c)} style:height="{counted.includes(c) ? Math.max(4, (c.seconds / maxSeconds) * 100) : 100}%" title={label(c)}></i>
			{/each}
		</div>
	</div>
</section>

<style>
	.chart { margin-bottom: 14px; padding: 10px 12px 12px; border: 1px solid var(--border); border-radius: 2px; }
	header { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 12px; margin-bottom: 10px; }
	h3 { margin: 0; font-size: 12.5px; font-weight: 600; }
	header span { color: var(--muted-foreground); font-size: 12px; }
	.strips { display: grid; grid-template-columns: 74px minmax(0, 1fr); align-items: end; gap: 6px 10px; }
	.axis { align-self: center; color: var(--sb-text-faint); font-size: 11.5px; }
	.strip { display: grid; grid-template-columns: repeat(var(--n), minmax(2px, 14px)); align-items: end; gap: 3px; height: 56px; border-bottom: 1px solid var(--inner-line); }
	.strip.short { height: 28px; }
	i { display: block; border-radius: 1px 1px 0 0; background: var(--primary); }
	i.time { background: color-mix(in srgb, var(--muted-foreground) 55%, transparent); }
	i.partial { background: color-mix(in srgb, var(--primary) 45%, transparent); }
	i.empty { background: repeating-linear-gradient(135deg, var(--inner-line) 0 3px, transparent 3px 6px); }
</style>
