<script lang="ts">
	// "Planned in 10.6 s", opening where the planning time went: each step's seconds and share.
	import * as Popover from '$lib/components/ui/popover';
	import { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import { cn } from '$lib/utils';
	import TimerIcon from '@lucide/svelte/icons/timer';
	import type { PlanDocument, PlanTimings } from '$lib/api';
	import { seconds, timingFootnote, timingRows, timingTitle } from './plan-timings';

	let { plan, timings, now }: { plan: PlanDocument; timings: PlanTimings; now: number } = $props();
	const rows = $derived(timingRows(timings));
</script>

<Popover.Root>
	<Popover.Trigger class={cn(buttonVariants({ variant: 'ghost', size: 'sm' }), 'plan-timings-trigger')} data-testid="plan-timings">
		<TimerIcon />{timingTitle(timings, now)}
	</Popover.Trigger>
	<Popover.Content class="w-[520px] p-0" align="end">
		<div class="timings">
			<h4>Where the planning time went</h4>
			<!-- raw-table: a short breakdown of one plan's steps with share bars, not a list of records -->
			<table>
				<thead><tr><th>Step</th><th class="num">Seconds</th><th>Share</th></tr></thead>
				<tbody>
					{#each rows as r (r.name)}
						<tr>
							<td>{r.name}</td>
							<td class="num">{r.seconds}</td>
							<td><span class="share"><span class="bar" style:width="{Math.max(r.share * 100, 2)}%"></span></span><span class="pct">{r.percent}</span></td>
						</tr>
					{/each}
				</tbody>
				<tfoot><tr><td>Total</td><td class="num">{seconds(timings.total_seconds)}</td><td></td></tr></tfoot>
			</table>
			<p class="foot">{timingFootnote(plan, timings, now)}</p>
		</div>
	</Popover.Content>
</Popover.Root>

<style>
	.timings {
		padding: 12px 18px 12px;
		font-size: 12.5px;
	}
	h4 {
		margin: 0 0 8px;
		font-size: 13px;
		font-weight: 600;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th {
		padding: 6px 0;
		color: var(--muted-foreground);
		font-size: 11.5px;
		font-weight: 500;
		text-align: left;
	}
	td {
		padding: 7px 0;
		border-top: 1px solid var(--border-subtle);
	}
	.num {
		width: 64px;
		padding-right: 16px;
		font-variant-numeric: tabular-nums;
		text-align: right;
	}
	th:last-child,
	td:last-child {
		width: 136px;
	}
	.share {
		display: inline-block;
		width: 84px;
		height: 7px;
		margin-right: 8px;
		vertical-align: middle;
	}
	.bar {
		display: block;
		height: 100%;
		border-radius: 1px;
		background: var(--primary);
	}
	.pct {
		color: var(--muted-foreground);
		font-variant-numeric: tabular-nums;
	}
	tfoot td {
		border-top: 1px solid var(--border);
		font-weight: 600;
	}
	.foot {
		margin: 10px 0 0;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
</style>
