<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { to } from '$lib/paths';
	import type { MicrobatchCoverage, WarehouseObject } from '$lib/api';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { formatBytes, formatNumber, relationName, warehouseColumnLabel, warehouseColumnTone } from '$lib/build-area/labels';

	// The synced information-schema object, its sampled statistics and, for
	// microbatch models, the projection of `_sqlbuild_microbatches`.
	let { warehouse, microbatch }: { warehouse: WarehouseObject; microbatch: MicrobatchCoverage | null } = $props();

	const W = 640;
	const H = 90;
	const path = $derived.by(() => {
		const s = warehouse.stats;
		if (s.length < 2) return '';
		const min = Math.min(...s.map((p) => p.rows));
		const max = Math.max(...s.map((p) => p.rows));
		// A flat series runs through the middle rather than along the floor.
		const y = (v: number) => (max === min ? H / 2 : H - 6 - ((v - min) / (max - min)) * (H - 12));
		return s.map((p, i) => `${i ? 'L' : 'M'}${((i / (s.length - 1)) * W).toFixed(1)},${y(p.rows).toFixed(1)}`).join(' ');
	});
	/** Declared types are compared only where the sync knows them. */
	const compared = $derived(warehouse.columns.some((c) => c.state));
	const mismatched = $derived(warehouse.columns.filter((c) => c.state && c.state !== 'match').length);
	const batchTone = { complete: 'var(--sb-success)', gap: 'var(--sb-error)', replay_required: 'var(--sb-warning)' } as const;
	const batchLabel = { complete: 'complete', gap: 'gap', replay_required: 'replay required' } as const;
	/** Timestamp bounds as local stamps; integer bounds as written. */
	const bound = (text: string | null) => (microbatch?.cursorType === 'timestamp' ? formatStamp(text) : (text ?? '—'));
	const columnColumns = $derived<TableColumn[]>([
		{ id: 'column', label: 'Column', minWidth: 200 },
		...(compared ? [{ id: 'declared', label: 'Declared', width: '22%' }] : []),
		{ id: 'actual', label: 'In the warehouse', width: '22%' },
		...(compared ? [{ id: 'state', label: 'State', width: '160px' }] : [])
	]);
</script>

<div class="wrap">
	<div class="grid">
		<Panel title="Object" note="Observed {formatAgo(warehouse.syncedAt, app.now)}">
			<KeyValues
				rows={[
					{ label: 'Relation', value: relationName(warehouse.relation), mono: true },
					{ label: 'Type', value: warehouse.objectType },
					warehouse.rows !== null && { label: 'Rows', value: formatNumber(warehouse.rows) },
					warehouse.bytes !== null && { label: 'Size', value: formatBytes(warehouse.bytes) },
					warehouse.createdAt && { label: 'Created', value: formatStamp(warehouse.createdAt) },
					warehouse.lastAlteredAt && { label: 'Last altered', value: `${formatStamp(warehouse.lastAlteredAt)} · ${formatAgo(warehouse.lastAlteredAt, app.now)}` }
				].filter((row) => !!row)}
			/>
		</Panel>
		{#if warehouse.stats.length}
			<Panel title="Rows over time" note="{formatNumber(warehouse.stats.length)} {warehouse.stats.length === 1 ? 'sample' : 'samples'}">
				<svg viewBox="0 0 {W} {H}" class="spark" preserveAspectRatio="none" aria-label="Row count over time">
					<path d={path} fill="none" stroke="var(--primary)" stroke-width="1.6" vector-effect="non-scaling-stroke" />
				</svg>
				<div class="spark-axis">
					<span>{formatStamp(warehouse.stats[0].at)} · {formatNumber(warehouse.stats[0].rows)} rows</span>
					<span>{formatNumber(warehouse.stats.at(-1)?.rows)} rows · now</span>
				</div>
			</Panel>
		{/if}
	</div>

	<Panel title="Columns" note={!compared ? `${warehouse.columns.length} columns` : mismatched ? `${mismatched} differ` : 'All match'}>
		<DataTable class="static" embedded columns={columnColumns} rows={warehouse.columns} key={(c) => c.name} empty="No columns reported.">
			{#snippet row(c)}
					<tr>
						<td><code>{c.name}</code></td>
						{#if compared}<td><code class="muted">{c.declaredType ?? '—'}</code></td>{/if}
						<td><code class="muted">{c.actualType ?? '—'}</code></td>
						{#if compared}<td>{#if c.state}<StatusLabel tone={warehouseColumnTone[c.state]} label={warehouseColumnLabel[c.state]} muted={c.state === 'match'} />{/if}</td>{/if}
					</tr>
			{/snippet}
		</DataTable>
	</Panel>

	{#if microbatch}
		<Panel title="Microbatch coverage" help="Tracked only for concurrent microbatch models" note="{microbatch.cursorColumn} · {microbatch.batchSize} batches · complete to {bound(microbatch.contiguousFrontier)}">
			<div class="batches">
				{#each microbatch.batches as b (b.start)}
					<a class="batch" href={b.runId ? to(`/runs/${b.runId}`) : undefined} style:background={batchTone[b.state]} title="{bound(b.start)} – {bound(b.end)} · {batchLabel[b.state]}{b.runId ? ` · ${b.runId}` : ''}"><span class="sr-only">{batchLabel[b.state]}</span></a>
				{/each}
			</div>
			<div class="legend">
				<span><i style:background={batchTone.complete}></i>complete</span>
				<span title="Holes between completed partitions"><i style:background={batchTone.gap}></i>{microbatch.knownGaps} known {microbatch.knownGaps === 1 ? 'gap' : 'gaps'}</span>
				<span><i style:background={batchTone.replay_required}></i>replay required</span>
				<span class="muted">{microbatch.batchesTruncated ? 'Newest only · ' : ''}since {bound(microbatch.earliest)}</span>
			</div>
		</Panel>
	{/if}
</div>

<style>
	.wrap {
		padding: 20px 24px 48px;
	}
	.grid {
		display: grid;
		grid-template-columns: minmax(320px, 0.8fr) minmax(0, 1.2fr);
		gap: 28px;
	}
	.spark {
		display: block;
		width: 100%;
		height: 90px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.spark-axis {
		display: flex;
		justify-content: space-between;
		margin-top: 5px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	code {
		font: 12px var(--font-mono);
	}
	.muted {
		color: var(--muted-foreground);
	}
	.batches {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(12px, 1fr));
		gap: 2px;
	}
	.batch {
		height: 18px;
		border-radius: 2px;
		opacity: 0.8;
	}
	.batch:hover {
		opacity: 1;
	}
	.legend {
		display: flex;
		gap: 16px;
		margin-top: 10px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.legend span {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.legend i {
		width: 8px;
		height: 8px;
		border-radius: 2px;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0 0 0 0);
	}
</style>
