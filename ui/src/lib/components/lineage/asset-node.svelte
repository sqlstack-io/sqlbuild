<script lang="ts">
	import GitCompareIcon from '@lucide/svelte/icons/git-compare';
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import {
		Database,
		Table2,
		Eye,
		ChartGantt,
		FunctionSquare,
		Sprout,
		Download,
		GitMerge,
		Repeat,
		Columns3,
		Package,
		TableProperties,
		type Icon as IconType
	} from '@lucide/svelte';
	import { nodeFields } from '$lib/lineage/node-fields.svelte';
	import { formatAgo } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import CompactFace from '$lib/components/graph/compact-face.svelte';
	import { fonts } from '$lib/components/graph/fonts.svelte';
	import { fitStatus } from '$lib/lineage/status-line';
	import { getContext } from 'svelte';
	import { COMPACT_TAILS, type CompactTails } from '$lib/components/graph/compact-tails';
	import { measureText, nameOnlyText } from '$lib/components/graph/compact-title';
	import { cardStage } from '$lib/lineage/fit';
	import { discoveredStep } from '$lib/lineage/discovered-labels';

	type AssetData = {
		name: string;
		/** Shown instead of the key when set; the key stays in the tooltip. */
		label?: string;
		kind: string;
		/** The last build outcome; staleness is shown separately. */
		status: 'ok' | 'fail' | 'warn' | 'run' | 'src' | 'none';
		/** The main stale reason, if any. */
		stale?: 'changed' | 'compiled' | 'upstream' | 'behind';
		/** The node is in an active run, so its status is that run's, not its last build's. */
		live?: boolean;
		checks?: string;
		checksColor?: string;
		dimmed?: boolean;
		icon?: string;
		/** When the node was last built, or a source last loaded. */
		lastBuiltAt?: string;
		description?: string;
		tags?: string[];
		columnCount?: number;
	};

	let { id, data, selected }: NodeProps = $props();
	const d = $derived(data as AssetData);
	const f = $derived(nodeFields.value);

	const statusColor: Record<string, string> = {
		ok: 'var(--sb-success)',
		fail: 'var(--sb-error)',
		warn: 'var(--sb-warning)',
		run: 'var(--sb-running)',
		src: 'var(--sb-text-faint)',
		none: 'var(--sb-text-faint)',
		skip: 'var(--sb-skipped, var(--sb-text-faint))',
		wait: 'var(--sb-text-faint)',
		notrun: 'var(--sb-text-faint)'
	};
	const statusLabel: Record<string, string> = {
		ok: 'Success',
		fail: 'Failed',
		warn: 'Warning',
		run: 'Running',
		src: 'Source',
		none: 'Not built',
		// A run's own graph: what happened to the node in that run.
		skip: 'Skipped',
		wait: 'Waiting',
		notrun: 'Not run'
	};
	const staleLabel: Record<string, string> = { changed: 'changed', compiled: 'changed', upstream: 'upstream', behind: 'behind' };
	const staleTitle: Record<string, string> = {
		changed: 'Stale · changed: the definition differs from what was last built in this target',
		compiled: 'Stale · changed: the compiled SQL differs from what was last built in this target',
		upstream: 'Stale · changed (upstream): an upstream definition changed since the last build',
		behind: 'Stale · behind: upstream has newer data than the last build'
	};
	const icons: Record<string, typeof IconType> = {
		source: Database,
		loader: Download,
		seed: Sprout,
		view: Eye,
		table: Table2,
		merge: GitMerge,
		delete_insert: Repeat,
		microbatch: ChartGantt,
		asset: Package,
		udf: FunctionSquare,
		table_fn: TableProperties,
		fn: FunctionSquare
	};
	// No kind word on the card: the icon tells the kinds and incremental strategies apart, and the full
	// materialization is in the tooltip and the side panel.
	const Glyph = $derived(icons[d.icon ?? ''] ?? Table2);
	// Zoomed out to the name-only stage, the name is fitted to the card at its enlarged size: a
	// discovered step's shared prefix gives way first, any other name loses its end.
	const tails = getContext<CompactTails | undefined>(COMPACT_TAILS);
	const shownName = $derived.by(() => {
		const label = d.label ?? d.name;
		const zoom = tails?.zoom();
		if (zoom === undefined || cardStage(zoom) !== 'name') return label;
		void fonts.ready;
		void fonts.version;
		return nameOnlyText(label, zoom, discoveredStep(id) ? (tails?.lead(id) ?? 0) : 0, measureText);
	});
	const tone = $derived(statusColor[d.status] ?? statusColor.none);
	const when = $derived(d.lastBuiltAt ? (d.status === 'src' ? `loaded ${formatAgo(d.lastBuiltAt, app.now)}` : formatAgo(d.lastBuiltAt, app.now)) : '');

	// Zoomed out, the card's state is its colour: a failure or a run first, then staleness, then the outcome.
	const faceTone = $derived(
		d.live || d.status === 'fail' || d.status === 'run' ? tone : d.stale === 'behind' ? 'var(--sb-behind)' : d.stale ? 'var(--sb-stale)' : tone
	);
	// The status line stays one line: worked out once per card, the drift chip and then the status word
	// shorten when its parts don't fit; the time never does.
	const fit = $derived(
		(void fonts.ready,
		fitStatus({
			label: f.status ? statusLabel[d.status] : undefined,
			when: f.lastBuilt && when ? when : undefined,
			drift: d.stale ? staleLabel[d.stale] : undefined,
			checks: f.checks && d.checks ? d.checks : undefined
		}))
	);
	const showStatus = $derived(f.status || (f.checks && d.checks) || (f.lastBuilt && when) || d.stale);

	const hasDetail = $derived(
		!!(
			(f.description && d.description) ||
			(f.tags && d.tags?.length) ||
			(f.columns && d.columnCount != null)
		)
	);
</script>

<div class="node" class:selected class:dimmed={d.dimmed} style:--face={faceTone}>
	<Handle type="target" position={Position.Left} class="node-handle" />

	<div class="head">
		<Glyph size={14} strokeWidth={1.75} class="glyph" />
		<span class="name" title={d.kind ? `${d.name} · ${d.kind}` : d.name} data-label={d.label ?? d.name}>{shownName}</span>
	</div>

	{#if hasDetail}
		<div class="detail">
			{#if f.description && d.description}<p>{d.description}</p>{/if}
			{#if f.columns && d.columnCount != null}<span><Columns3 size={11} /> {d.columnCount} columns</span>{/if}
			{#if f.tags && d.tags?.length}<span>{d.tags.join(', ')}</span>{/if}
		</div>
	{/if}

	{#if showStatus}
		<div class="status" style:--tone={tone}>
			{#if f.status}
				<i title={statusLabel[d.status]}></i>
				{#if fit.label}<span class="label">{statusLabel[d.status]}</span>{/if}
			{/if}
			{#if f.lastBuilt && when}<span class="when" class:first={!f.status || !fit.label}>{when}</span>{/if}
			{#if d.stale}
				<span class="drift {d.stale}" title={staleTitle[d.stale]}><GitCompareIcon size={10} strokeWidth={2} />{#if fit.driftWord}{staleLabel[d.stale]}{/if}</span>
			{/if}
			{#if f.checks && d.checks}
				<span class="checks" style:color={d.checksColor ?? 'var(--muted-foreground)'}>{d.checks}</span>
			{/if}
		</div>
	{/if}

	<CompactFace {id} title={d.label ?? d.name} tone={faceTone} kind="asset" />
	<Handle type="source" position={Position.Right} class="node-handle" />
</div>

<style>
	.node {
		position: relative;
		width: 236px;
		overflow: hidden;
		border: 1px solid var(--node-border);
		border-radius: 2px;
		background: var(--card);
		color: var(--foreground);
		font-family: var(--font-sans);
		transition:
			opacity 0.15s,
			border-color 0.15s;
	}
	.node:hover {
		border-color: color-mix(in srgb, var(--foreground) 35%, transparent);
	}
	.node.selected {
		border-color: var(--primary);
		box-shadow: 0 0 0 1px var(--primary);
	}
	.node.dimmed {
		opacity: 0.35;
	}
	.head {
		display: flex;
		height: 34px;
		align-items: center;
		gap: 8px;
		padding: 0 10px;
	}
	.head :global(.glyph) {
		flex: 0 0 auto;
		color: var(--muted-foreground);
	}
	.name {
		min-width: 0;
		flex: 1;
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: 12.5px;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* Name-only stage: the same card, its other lines hidden and its name enlarged to stay about 10 px
	   on screen; the card keeps its size, so nothing moves. Its state shows as a stripe and the icon's colour. */
	:global(.graph-canvas[data-stage='name']) .node:not(.selected) {
		box-shadow: inset 3px 0 0 var(--face);
	}
	:global(.graph-canvas[data-stage='name']) .detail,
	:global(.graph-canvas[data-stage='name']) .status {
		visibility: hidden;
	}
	:global(.graph-canvas[data-stage='name']) .head {
		transform: translateY(13px);
	}
	:global(.graph-canvas[data-stage='name']) .head :global(.glyph) {
		width: calc(14px * min(var(--sb-inv, 1), 1.8));
		height: calc(14px * min(var(--sb-inv, 1), 1.8));
		color: var(--face);
	}
	:global(.graph-canvas[data-stage='name']) .name {
		font-size: clamp(12.5px, calc(10px * var(--sb-inv, 1)), 26px);
	}
	/* drift is a cause of staleness, so it shares the stale colour */
	.drift {
		display: inline-flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 3px;
		padding: 0 5px;
		border: 1px solid transparent;
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-stale) 16%, transparent);
		color: var(--sb-stale);
		font-size: 10.5px;
		font-weight: 600;
		line-height: 15px;
	}
	.drift.upstream {
		border: 1px dashed color-mix(in srgb, var(--sb-stale) 55%, transparent);
		background: transparent;
		font-weight: 500;
	}
	.drift.behind {
		background: color-mix(in srgb, var(--sb-behind) 12%, transparent);
		color: var(--sb-behind);
	}
	.when.first::before {
		content: none;
	}
	.detail {
		display: flex;
		flex-direction: column;
		gap: 3px;
		padding: 0 10px 8px 32px;
		color: var(--muted-foreground);
		font-size: 11px;
	}
	.detail p {
		margin: 0;
		line-height: 1.35;
	}
	.detail span {
		display: flex;
		align-items: center;
		gap: 5px;
	}
	.status {
		display: flex;
		height: 26px;
		align-items: center;
		gap: 6px;
		padding: 0 10px;
		border-top: 1px solid var(--border);
		background: color-mix(in srgb, var(--tone) 9%, var(--card));
		font-family: var(--font-mono);
		font-size: 11px;
		/* One line, whatever the parts: they are fitted to it (status-line.ts); none wraps or shrinks. */
		white-space: nowrap;
		overflow: hidden;
	}
	.status > :global(*) {
		flex-shrink: 0;
	}
	.status i {
		width: 7px;
		height: 7px;
		flex: 0 0 7px;
		border-radius: 50%;
		background: var(--tone);
	}
	.label {
		color: var(--foreground);
		font-weight: 500;
	}
	.when {
		color: var(--muted-foreground);
	}
	.when::before {
		content: '·';
		margin-right: 6px;
		color: var(--sb-text-faint);
	}
	.checks {
		margin-left: auto;
		font-variant-numeric: tabular-nums;
	}
	.node :global(.node-handle) {
		width: 6px;
		height: 6px;
		min-width: 0;
		min-height: 0;
		border: 0;
		background: transparent;
	}
</style>
