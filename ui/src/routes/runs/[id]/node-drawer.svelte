<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import Button from '$lib/components/ui/button/button.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import InspectorSection from '$lib/components/lineage/inspector-section.svelte';
	import OutcomeStrip from '$lib/components/build-area/outcome-strip.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { to } from '$lib/paths';
	import { formatDuration } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { nodeOutcomeTone } from '$lib/status';
	import { relationName } from '$lib/build-area/labels';
	import { nodeLabel, nodeStatusTone, shortRunId } from '$lib/operate/labels';
	import type { RunDetailState } from './state.svelte';
	import { formatQueryCost } from './utils';
	import { fullStamp, skipCause, warehouseQueryUrl } from './utils/proto';

	// A node in this run, in a drawer from the right (CHI-1591), modelled on the Lineage sidebar but
	// scoped to the run: what it did here, its SQL as it ran, its last builds, what it printed, and where
	// it lives. It shows the node last clicked on the timeline or the graph.
	let { view }: { view: RunDetailState } = $props();

	const key = $derived(view.drawer);
	const n = $derived(key ? (view.nodes.find((x) => x.nodeKey === key) ?? null) : null);
	const name = $derived(key ? nodeLabel(key) : '');
	const cause = $derived(n ? skipCause(n, view.nodes, view.detail?.edges ?? []) : null);
	const checks = $derived(key ? (view.eventsRes.data ?? []).filter((e) => e.type === 'audit_completed' && e.nodeKey === key) : []);
	const statements = $derived(key ? view.statements.filter((s) => s.nodeKey === key && s.runId === n?.runId) : []);
	const running = $derived(statements.find((s) => s.status === 'running') ?? null);
	const cost = $derived(key ? (view.cost?.resources.find((r) => r.nodeKey === key) ?? null) : null);
	const output = $derived(key ? (view.outputRes.data ?? []).filter((l) => l.nodeKey === key && l.runId === n?.runId) : []);
	const STATUS: Record<string, string> = { success: 'Built', warn: 'Built, with warnings', failed: 'Failed', skipped: 'Skipped', running: 'Building', pending: 'Waiting', not_run: 'Not run', cancelled: 'Not run (cancelled)' };
	const added = $derived(n?.kind === 'incremental' || n?.kind === 'loader');
	// Rows only where they are real: added rows, or a rebuilt table's count; while running only loaders and microbatches know.
	const rows = $derived(n?.rowsAffected == null ? null : `${added ? '+' : ''}${n.rowsAffected.toLocaleString('en-GB')}${n.status === 'running' ? ' so far' : ''}`);

	// The node's catalog facts: SQL or code, relation, description, and its last builds.
	const catalog = useCatalogNodes();
	const detail = createResource((client, scope) => {
		const k = key;
		const listing = catalog.data;
		return k && listing && app.client.supports('getCatalogNode') ? client.getCatalogNode(scope, k, listing) : Promise.resolve(null);
	});
	const d = $derived(detail.data && detail.data.key === key ? detail.data : null);
	// The SQL as it ran: the statement's own text when it was recorded, else the compiled definition.
	const ran = createResource((client, scope) => {
		const st = statements[0];
		return st?.sqlRecorded && app.client.supports('getStatementSql') ? client.getStatementSql(scope, st.runId, st.id).then((r) => r.sql) : Promise.resolve(null);
	});
	let sqlMode = $state<'raw' | 'compiled'>('compiled');

	// "open in warehouse" only where the adapter has a URL for a query; per-adapter URLs come later.
	const queryHref = $derived(running?.queryId ? warehouseQueryUrl(app.location?.target.adapter, running.queryId) : null);

	// Resizable from its left edge; the page beside it takes the rest.
	let width = $state(480);
	let dragging = $state(false);
	const clampWidth = (w: number) => Math.round(Math.min(Math.max(340, w), Math.max(360, window.innerWidth * 0.6)));

	function onkey(e: KeyboardEvent) {
		if (e.key === 'Escape' && key) view.openDrawer(null);
	}
</script>

<svelte:window onkeydown={onkey} />

{#if n && key}
	<aside class="drawer" style:width="{width}px" aria-label="{name} in this run" data-testid="node-drawer">
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
		<div
			class="resize"
			role="separator"
			aria-orientation="vertical"
			aria-label="Resize the drawer"
			aria-valuenow={width}
			tabindex="0"
			onpointerdown={(e) => {
				dragging = true;
				(e.target as HTMLElement).setPointerCapture(e.pointerId);
			}}
			onpointermove={(e) => dragging && (width = clampWidth(window.innerWidth - e.clientX))}
			onpointerup={() => (dragging = false)}
			onkeydown={(e) => {
				if (e.key === 'ArrowLeft') width = clampWidth(width + 24);
				if (e.key === 'ArrowRight') width = clampWidth(width - 24);
			}}
		></div>
		<header>
			<div class="title">
				<span class="name" title={key}>{name}</span>
				<div class="links">
					<a href={to(`/catalog/${key}`)} title="Open {key} in the Catalog">Catalog ↗</a>
					<a href={to(`/lineage?focus=${encodeURIComponent(key)}`)} title="Open {key} on Lineage">Lineage ↗</a>
					<Button variant="ghost" size="icon-sm" onclick={() => view.openDrawer(null)} aria-label="Close" title="Close (Esc)"><XIcon /></Button>
				</div>
			</div>
			<div class="meta">
				<span>{n.kind}</span>
				{#if d?.group}<span>{d.group}</span>{/if}
				{#if n.jobKey && view.detail?.children.length}<span><a href={to(`/runs/${n.runId}`)}>{n.jobKey} · {shortRunId(n.runId)}</a></span>{/if}
			</div>
		</header>

		<div class="body">
			<InspectorSection title="In this run">
				<div class="status-top"><StatusLabel tone={nodeStatusTone[n.status]} label={STATUS[n.status] ?? n.status} /></div>
				<div class="stats">
					<div><span>Duration</span><b>{n.startedAt ? `${formatDuration(n.startedAt, n.endedAt, app.now)}${n.status === 'running' ? ' so far' : ''}` : '—'}</b></div>
					<div><span>Rows</span><b>{rows ?? '—'}</b></div>
					<div><span>Checks</span><b>{checks.length ? `${checks.filter((c) => c.level === 'info').length}/${checks.length}` : '—'}</b></div>
					<div><span>Cost</span><b>{cost ? formatQueryCost(cost) : '—'}</b></div>
				</div>
				{#if cause}<p class="why">{cause}</p>{/if}
				{#if n.status === 'failed' && n.errorMessage}
					<div class="error">
						<b>{n.errorMessage}</b>
						{#if n.errorLocation}<div class="faint mono">{n.errorLocation.file}{n.errorLocation.line != null ? `:${n.errorLocation.line}` : ''}</div>{/if}
					</div>
				{/if}
				<dl class="kv">
					{#if n.startedAt}<div><dt>Started</dt><dd class="mono">{fullStamp(n.startedAt)}</dd></div>{/if}
					{#if running?.queryId}
						<div>
							<dt>Query</dt>
							<dd><span class="mono">{running.queryId}</span>{#if queryHref} · <a href={queryHref} target="_blank" rel="noreferrer" title="Open this query in the warehouse's console">open in warehouse ↗</a>{/if}</dd>
						</div>
					{/if}
					{#if statements.length}
						<div><dt>Statements</dt><dd><Button variant="link" size="inline" onclick={() => (view.tab = 'statements')}>{statements.length} in the warehouse →</Button></dd></div>
					{/if}
				</dl>
				{#if checks.length}
					<ul class="checks">
						{#each checks as c (c.id)}
							<li><i style:background={c.level === 'warn' ? 'var(--sb-warning)' : c.level === 'error' ? 'var(--sb-error)' : 'var(--sb-success)'}></i><span class="mono">{c.message}</span></li>
						{/each}
					</ul>
				{/if}
			</InspectorSection>

			{#if d?.recentOutcomes.length}
				<InspectorSection title="Builds">
					<OutcomeStrip outcomes={d.recentOutcomes} tones={nodeOutcomeTone} caption="Last {d.recentOutcomes.length} builds" />
				</InspectorSection>
			{/if}

			{#if output.length}
				<InspectorSection title="Output">
					{#snippet meta()}
						<span>{(output.reduce((sum, l) => sum + (l.omitted ?? 1), 0)).toLocaleString('en-GB')} lines</span>
						{#if output.some((l) => l.omitted)}<a class="viewer-link" href={to(`/runs/${n.runId}/output/${key}`)}>Open in viewer →</a>{/if}
					{/snippet}
					<div class="output">
						{#each output.slice(-60) as l, i (i)}
							{#if l.omitted}
								<!-- The live view is capped (CHI-1597): the rest is in the paged viewer. -->
								<a class="omitted" href={to(`/runs/${n.runId}/output/${key}`)}>+{l.omitted.toLocaleString('en-GB')} lines · open in viewer →</a>
							{:else}
								<div class:err={l.stream === 'stderr'}><span class="stream">{l.stream}</span><pre>{l.text}</pre></div>
							{/if}
						{/each}
					</div>
				</InspectorSection>
			{/if}

			{#if d?.sql}
				<InspectorSection title="SQL as it ran">
					<CodePreview file="{name}.sql" code={d.sql.raw} compiled={ran.data ?? d.sql.compiled} bind:mode={sqlMode} maxLines={16} />
				</InspectorSection>
			{:else if d?.code}
				<InspectorSection title="Code">
					<CodePreview file={d.code.file} code={d.code.source} language="text" maxLines={16} />
				</InspectorSection>
			{/if}

			{#if d}
				<InspectorSection title="Details">
					<dl class="kv">
						{#if d.relation}<div><dt>Relation</dt><dd class="mono">{relationName(d.relation)}</dd></div>{/if}
						{#if d.path}<div><dt>File</dt><dd class="mono">{d.path}</dd></div>{/if}
					</dl>
					{#if d.description}<p class="desc">{d.description}</p>{/if}
				</InspectorSection>
			{/if}
		</div>
	</aside>
{/if}

<style>
	.drawer {
		position: relative;
		display: flex;
		min-height: 0;
		flex: none;
		flex-direction: column;
		border-left: 1px solid var(--border-subtle);
		background: var(--card);
		font-size: 12.5px;
	}
	.resize {
		position: absolute;
		top: 0;
		bottom: 0;
		left: -3px;
		z-index: 2;
		width: 6px;
		cursor: col-resize;
	}
	.resize:hover,
	.resize:focus-visible {
		background: color-mix(in srgb, var(--primary) 35%, transparent);
	}
	header {
		padding: 14px 16px 12px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.title {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.name {
		overflow: hidden;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: 14px;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.links {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-left: auto;
		font-size: 12px;
	}
	.links a {
		color: var(--primary);
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		margin-top: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.meta span + span::before {
		content: '·';
		margin-right: 12px;
		color: var(--sb-text-faint);
	}
	.body {
		overflow-y: auto;
		flex: 1;
		padding: 4px 8px 24px;
	}
	.status-top {
		margin: 2px 0 8px;
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 8px;
		margin-bottom: 8px;
	}
	.stats div {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.stats span {
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.stats b {
		font-weight: 500;
		font-variant-numeric: tabular-nums;
	}
	.kv {
		display: flex;
		flex-direction: column;
		gap: 3px;
		margin: 6px 0 0;
	}
	.kv div {
		display: grid;
		grid-template-columns: 90px 1fr;
	}
	dt {
		color: var(--sb-text-faint);
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.why,
	.desc {
		margin: 6px 0 0;
		color: var(--muted-foreground);
	}
	.error {
		margin-top: 6px;
		padding: 8px 10px;
		border-left: 2px solid var(--sb-error);
		background: color-mix(in srgb, var(--sb-error) 6%, transparent);
	}
	.checks {
		display: flex;
		flex-direction: column;
		gap: 3px;
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
	}
	.checks i {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	.output {
		max-height: 260px;
		overflow: auto;
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.output div {
		display: flex;
		gap: 8px;
		padding: 1px 4px;
	}
	.output div.err {
		background: color-mix(in srgb, var(--sb-warning) 6%, transparent);
		color: color-mix(in srgb, var(--sb-warning) 70%, var(--foreground));
	}
	.output .stream {
		flex: none;
		width: 42px;
		color: var(--sb-text-faint);
	}
	.output pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.viewer-link {
		margin-left: auto;
		color: var(--primary);
		font-size: 12px;
	}
	.omitted {
		display: block;
		padding: 2px 4px 4px;
		color: var(--primary);
		font-family: var(--font-sans);
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.mono {
		font-family: var(--font-mono);
	}
</style>
