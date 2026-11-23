<script lang="ts" generics="R extends CostRow">
	import Panel from '$lib/components/platform/panel.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { plural } from '$lib/format/plural';
	import type { CostRow, CreditPrice } from './types';
	import { change, costTitle, hours, sharePct, usd } from './utils';

	// A ranked breakdown: name, cost, share of the period and one count, most expensive first.
	// `limit` rows show until "Show all"; `sub` adds a muted second fact after the name.
	let {
		title,
		help,
		rows,
		total,
		price,
		name,
		count,
		sub,
		href,
		limit = 8,
		showChange = false,
		showTime = false,
		bar = true,
		selected,
		onselect
	}: {
		title: string;
		help?: string;
		rows: R[];
		total: number;
		price: CreditPrice;
		name: string;
		count: { label: string; value: (r: R) => number };
		sub?: (r: R) => string;
		href?: (r: R) => string | undefined;
		limit?: number;
		showChange?: boolean;
		showTime?: boolean;
		/** Draw the share as a bar beside its percentage. */
		bar?: boolean;
		selected?: string | null;
		onselect?: (r: R) => void;
	} = $props();

	let all = $state(false);
	const shown = $derived(all ? rows : rows.slice(0, limit));
	const top = $derived(rows[0]?.credits || 1);
	const columns = $derived<TableColumn[]>([
		{ id: 'name', label: name, minWidth: 110 },
		{ id: 'cost', label: 'Cost', width: '92px', align: 'right' },
		{ id: 'share', label: 'Share', width: bar ? '116px' : '68px', align: 'right' },
		...(showTime ? [{ id: 'time', label: 'Time', width: '72px', align: 'right' as const, title: 'Warehouse time the queries used' }] : []),
		{ id: 'count', label: count.label, width: '72px', align: 'right' },
		...(showChange ? [{ id: 'change', label: 'Change', width: '76px', align: 'right' as const, title: 'Against the period before' }] : [])
	]);
</script>

<Panel {title} {help} note={plural(rows.length, name.toLowerCase())}>
	<DataTable class="compact framed {onselect ? '' : 'static'}" embedded {columns} rows={shown} key={(r) => r.key} empty="No cost in this period.">
		{#snippet row(r)}
			{@const c = change(r.credits, r.previous)}
			{@const link = href?.(r)}
			<tr class:selected={selected === r.key} onclick={() => onselect?.(r)}>
				<td>
					{#if link}<a href={link} title={r.key}>{r.key}</a>{:else}<span title={r.key}>{r.key}</span>{/if}{#if sub}<span class="sub">{sub(r)}</span>{/if}
				</td>
				<td title={costTitle(r.credits, price)}>{usd(r.credits, price)}</td>
				<td><span class="share">{#if bar}<span class="bar"><i style:width="{(r.credits / top) * 100}%"></i></span>{/if}<span class="pct">{sharePct(r.credits, total)}</span></span></td>
				{#if showTime}<td class="muted">{hours(r.seconds)}</td>{/if}
				<td class="muted">{count.value(r).toLocaleString('en')}</td>
				{#if showChange}<td class="change {c?.tone ?? ''}">{c?.text ?? '—'}</td>{/if}
			</tr>
		{/snippet}
	</DataTable>
	{#if rows.length > limit}
		<button class="more" onclick={() => (all = !all)}>{all ? 'Show fewer' : `Show all ${rows.length.toLocaleString('en')}`}</button>
	{/if}
</Panel>

<style>
	.sub {
		margin-left: 8px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.share {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		gap: 8px;
	}
	.bar {
		width: 40px;
		height: 4px;
		border-radius: 2px;
		background: var(--inner-line);
	}
	.bar i {
		display: block;
		height: 100%;
		border-radius: 2px;
		background: var(--primary);
	}
	.pct {
		width: 34px;
		color: var(--muted-foreground);
	}
	.change.up {
		color: var(--sb-warning);
	}
	.change.down {
		color: var(--sb-success);
	}
	.change.flat {
		color: var(--sb-text-faint);
	}
	tr.selected {
		background: color-mix(in srgb, var(--primary) 10%, transparent);
	}
	.more {
		margin-top: 6px;
		padding: 2px 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.more:hover {
		color: var(--primary);
		text-decoration: underline;
	}
</style>
