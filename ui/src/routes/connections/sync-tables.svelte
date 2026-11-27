<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatMs, formatNumber } from '$lib/build-area/labels';
	import { plural } from '$lib/format/plural';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { bySeverity, isProblem, oldestClean, type Table } from './utils/connections';

	// A target's per-table sync list. Problem tables always show, each with its failure under it; the
	// tables that synced cleanly fold into one line, e.g. "13 more tables synced 4m ago".
	let { tables }: { tables: readonly Table[] } = $props();

	let open = $state(false);
	const problems = $derived(bySeverity(tables.filter(isProblem)));
	const clean = $derived(bySeverity(tables.filter((t) => !isProblem(t))));
	const rows = $derived(open ? [...problems, ...clean] : problems);
	const cleanAt = $derived(oldestClean(tables));

	const columns: TableColumn[] = [
		{ id: 'table', label: 'Table', minWidth: 150 },
		{ id: 'newest', label: 'Newest sync', width: '130px' },
		{ id: 'total', label: 'Mirrored', width: '100px', align: 'right', title: 'Records the server mirrors in total' },
		{ id: 'added', label: 'Last added', width: '100px', align: 'right', title: 'Records the newest sync added' },
		{ id: 'took', label: 'Took', width: '70px', align: 'right' },
		{ id: 'clean', label: 'Last clean sync', width: '120px' }
	];
</script>

<div class="sync-tables">
	{#if rows.length}
		<DataTable class="static compact framed" embedded {columns} {rows} key={(t) => t.table} empty="No tables reported.">
			{#snippet row(t)}
				<tr class:has-detail={!!t.failure}>
					<td><code>{t.table}</code></td>
					<td title={formatStamp(t.at)}>
						{#if t.waiting}<StatusLabel tone="var(--sb-warning)" label="waiting for the warehouse" />{:else if t.failure}<StatusLabel tone="var(--sb-error)" label="failed {formatAgo(t.at, app.now)}" />{:else}<StatusLabel tone="var(--sb-success)" label="synced {formatAgo(t.at, app.now)}" />{/if}
					</td>
					<td class:faint={t.mirroredTotal === null}>{formatNumber(t.mirroredTotal)}</td>
					<td>{formatNumber(t.recordsSent)}</td>
					<td>{formatMs(t.elapsedMs)}</td>
					<td class="faint" title={t.lastSyncedAt ? formatStamp(t.lastSyncedAt) : undefined}>{t.lastSyncedAt ? formatAgo(t.lastSyncedAt, app.now) : 'never'}</td>
				</tr>
				{#if t.failure}
					<tr class="detail">
						<td colspan={columns.length}>
							<span class="why">{t.failure}</span>
							{#if t.error}<code class="err-text">{t.error}</code>{/if}
						</td>
					</tr>
				{/if}
			{/snippet}
		</DataTable>
	{/if}
	{#if clean.length}
		<DisclosureButton {open} onclick={() => (open = !open)} class="more text-[12.5px]">
			{plural(clean.length, problems.length ? 'more table' : 'table')}{#if cleanAt}&nbsp;synced {formatAgo(cleanAt, app.now)}{/if}
		</DisclosureButton>
	{:else if !tables.length}
		<p class="faint">No sync reported yet.</p>
	{/if}
</div>

<style>
	.sync-tables {
		display: flex;
		max-width: 780px;
		flex-direction: column;
		align-items: flex-start;
		gap: 8px;
		font-size: 12.5px;
	}
	.sync-tables > :global(.sb-table-scroll) {
		width: 100%;
	}
	.sync-tables :global(.sb-list th:first-child),
	.sync-tables :global(.sb-list td:first-child) {
		padding-left: 2px;
	}
	.sync-tables :global(.sb-list th:last-child),
	.sync-tables :global(.sb-list td:last-child) {
		padding-right: 2px;
	}
	.sync-tables :global(.sb-list th) {
		height: 30px;
		padding: 0 10px;
		background: transparent;
		font-size: 12px;
	}
	.sync-tables :global(.sb-list td) {
		padding-right: 10px;
		padding-left: 10px;
	}
	.sync-tables :global(tr.has-detail td) {
		border-bottom: 0;
	}
	.detail td {
		padding-top: 0 !important;
		padding-left: 17px !important;
		color: var(--muted-foreground);
		font-size: 12px;
		white-space: normal;
	}
	.why {
		display: block;
		max-width: 72ch;
	}
	.err-text {
		display: block;
		margin-top: 2px;
		color: var(--sb-mono-err, var(--sb-error));
		overflow-wrap: anywhere;
	}
	.faint {
		color: var(--sb-text-faint);
	}
	code {
		font: 12px var(--font-mono);
	}
</style>
