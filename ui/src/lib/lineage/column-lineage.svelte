<script lang="ts">
	// Column-level lineage, dbt-style and scalable: a searchable list of the focal
	// model's columns on the left; pick one and the graph on the right shows just
	// THAT column's lineage chain (one small COL node per model it flows through,
	// transform-lens tags on each hop). Bounded by a column's depth, not by the
	// model's column count — so it scales to 100+ columns.
	import '@xyflow/svelte/dist/style.css';
	import { SvelteFlow, Background, Controls, type Node, type Edge } from '@xyflow/svelte';
	import ColCellNode from '$lib/components/lineage/col-cell-node.svelte';
	import FlowController from '$lib/lineage/flow-controller.svelte';
	import { layoutGraph } from '$lib/lineage/layout';
	import { highlightSqlbuild } from '$lib/sql/highlight';
	import { tick } from 'svelte';

	type ColModel = { id: string; name: string; kind: string; status: 'ok' | 'fail' | 'warn' | 'stale' | 'src'; columns: { name: string; type: string }[] };
	type Lens = 'raw' | 'passthrough' | 'rename' | 'transform';

	let {
		focal,
		models,
		colEdges,
		modelSql,
		modelColExpr
	}: {
		focal: string;
		models: ColModel[];
		colEdges: [string, string, Lens][];
		/** per-model SQL: modelId -> SQL text */
		modelSql?: Record<string, string>;
		/** per-model column expressions: modelId -> { colName -> defining substring } */
		modelColExpr?: Record<string, Record<string, string>>;
	} = $props();

	const nodeTypes = { colcell: ColCellNode };

	// --- resizable left list width + bottom SQL panel height (transient) ---
	let listW = $state(260);
	let listDragging = $state(false);
	let rootEl: HTMLDivElement;
	function startList(e: PointerEvent) {
		listDragging = true;
		(e.target as HTMLElement).setPointerCapture(e.pointerId);
	}
	function onList(e: PointerEvent) {
		if (!listDragging || !rootEl) return;
		listW = Math.min(420, Math.max(180, e.clientX - rootEl.getBoundingClientRect().left));
	}
	function endList(e: PointerEvent) {
		listDragging = false;
		try {
			(e.target as HTMLElement).releasePointerCapture(e.pointerId);
		} catch {}
	}



	// Lenses are neutral (status colours stay reserved for health); they differ by line style.
	const lensLabel: Record<Lens, string> = { raw: 'Source', passthrough: 'Passthrough', rename: 'Renamed', transform: 'Transformed' };
	const lensStroke: Record<Lens, string> = {
		raw: 'stroke:var(--sb-edge); stroke-width:1.4;',
		passthrough: 'stroke:var(--sb-edge); stroke-width:1.4;',
		rename: 'stroke:var(--muted-foreground); stroke-width:1.5; stroke-dasharray:5 4;',
		transform: 'stroke:var(--foreground); stroke-width:2.2;'
	};
	const lenses: Lens[] = ['passthrough', 'rename', 'transform'];

	const modelOf = (colId: string) => colId.split(/\.(.*)/)[0];
	const colOf = (colId: string) => colId.split(/\.(.*)/)[1];
	const nameOf = (id: string) => models.find((m) => m.id === id)?.name ?? id;
	const kindOf = (id: string) => models.find((m) => m.id === id)?.kind ?? 'table';
	const statusOf = (id: string) => models.find((m) => m.id === id)?.status ?? 'ok';
	const incomingLens = $derived(new Map<string, Lens>(colEdges.map(([, t, lens]) => [t, lens])));

	const focalModel = $derived(models.find((m) => m.id === focal));
	let colQuery = $state('');
	const COL_LIST_CAP = 200; // render cap; search narrows past this
	const matchedColumns = $derived(
		(focalModel?.columns ?? []).filter((c) => !colQuery.trim() || c.name.toLowerCase().includes(colQuery.toLowerCase()))
	);
	const totalColumns = $derived(focalModel?.columns.length ?? 0);
	const focalColumns = $derived(matchedColumns.slice(0, COL_LIST_CAP));
	const truncated = $derived(matchedColumns.length > COL_LIST_CAP);

	// selected column id (model.col); default to the first focal column
	let selectedCol = $state<string | null>(null);
	$effect(() => {
		focal;
		selectedCol = focalModel?.columns[0] ? `${focal}.${focalModel.columns[0].name}` : null;
	});

	// A chain node's SQL popover shows where the traced column comes from IN THAT
	// model (defining expression highlighted). Hover previews; click pins it;
	// clicking empty canvas unpins.
	let hoverNode = $state<string | null>(null);
	let pinnedNode = $state<string | null>(null);
	const activeNode = $derived(pinnedNode ?? hoverNode);
	const sqlParts = $derived.by(() => {
		if (!activeNode) return null;
		const mid = modelOf(activeNode);
		const text = modelSql?.[mid];
		if (!text) return null;
		const model = nameOf(mid);
		const expr = modelColExpr?.[mid]?.[colOf(activeNode)];
		const i = expr ? text.indexOf(expr) : -1;
		const hl = (s: string) => (s ? highlightSqlbuild(s) : '');
		if (i < 0) return { model, col: colOf(activeNode), before: hl(text), match: '', after: '' };
		return {
			model,
			col: colOf(activeNode),
			before: hl(text.slice(0, i)),
			match: hl(expr!),
			after: hl(text.slice(i + expr!.length))
		};
	});

	// Open the popover already positioned on the highlighted expression — set the
	// <pre> scrollTop directly (instant, before paint) rather than animating to it,
	// which is disorienting on large models.
	let markEl = $state<HTMLElement | null>(null);
	$effect(() => {
		activeNode;
		sqlParts;
		const el = markEl;
		if (!el) return;
		const pre = el.closest('pre');
		if (!pre) return;
		// centre the mark within the pre's viewport, no animation
		pre.scrollTop = Math.max(0, el.offsetTop - pre.clientHeight / 2 + el.offsetHeight / 2);
	});

	// the chain: all column ids reachable up/down from the selected column,
	// plus the edges among them.
	const chain = $derived.by(() => {
		const cols = new Set<string>();
		const chainEdges: [string, string, Lens][] = [];
		if (!selectedCol) return { cols, chainEdges };
		cols.add(selectedCol);
		const walk = (dir: 'up' | 'down') => {
			let frontier = [selectedCol!];
			while (frontier.length) {
				const next: string[] = [];
				for (const c of frontier)
					for (const [s, t, lens] of colEdges) {
						const [from, to] = dir === 'up' ? [t, s] : [s, t];
						if (from === c) {
							if (!cols.has(to)) {
								cols.add(to);
								next.push(to);
							}
							chainEdges.push([s, t, lens]);
						}
					}
				frontier = next;
			}
		};
		walk('up');
		walk('down');
		// dedupe edges
		const seen = new Set<string>();
		const uniq = chainEdges.filter(([s, t]) => {
			const k = `${s}->${t}`;
			if (seen.has(k)) return false;
			seen.add(k);
			return true;
		});
		return { cols, chainEdges: uniq };
	});

	// ordered hops for the derivation panel: furthest upstream first, then downstream uses
	const steps = $derived.by(() => {
		if (!selectedCol) return [];
		const depth = new Map<string, number>([[selectedCol, 0]]);
		const walk = (dir: 1 | -1) => {
			let frontier = [selectedCol!];
			while (frontier.length) {
				const next: string[] = [];
				for (const c of frontier)
					for (const [s, t] of chain.chainEdges) {
						const [from, to] = dir === -1 ? [t, s] : [s, t];
						if (from === c && !depth.has(to)) {
							depth.set(to, depth.get(c)! + dir);
							next.push(to);
						}
					}
				frontier = next;
			}
		};
		walk(-1);
		walk(1);
		return [...depth.entries()]
			.sort((a, b) => a[1] - b[1])
			.map(([cid, d]) => ({
				cid,
				depth: d,
				model: nameOf(modelOf(cid)),
				col: colOf(cid),
				lens: incomingLens.get(cid) ?? ('raw' as Lens),
				expr: modelColExpr?.[modelOf(cid)]?.[colOf(cid)]?.replace(/\s{2,}/g, ' ')
			}));
	});
	const hops = $derived(steps.filter((st) => st.depth <= 0));
	const uses = $derived(steps.filter((st) => st.depth > 0));
	const activeStep = $derived(activeNode);

	let nodes = $state.raw<Node[]>([]);
	let edges = $state.raw<Edge[]>([]);
	let fitFn = $state<((opts?: { duration?: number }) => void) | undefined>();

	// build the chain graph when selection changes (one node per column-in-a-model)
	let builtKey = '';
	$effect(() => {
		const { cols, chainEdges } = chain;
		const key = [...cols].sort().join(',');
		if (key === builtKey) return;
		builtKey = key;
		nodes = [...cols].map((cid) => ({
			id: cid,
			type: 'colcell',
			position: { x: 0, y: 0 },
			data: {
				model: nameOf(modelOf(cid)),
				column: colOf(cid),
				kind: kindOf(modelOf(cid)),
				status: statusOf(modelOf(cid)),
				lens: incomingLens.get(cid),
				lensLabel,
				focal: cid === selectedCol,
				active: cid === activeStep
			}
		}));
		edges = chainEdges.map(([s, t, lens]) => ({
			id: `${s}->${t}`,
			source: s,
			target: t,
			type: 'bezier',
			style: lensStroke[lens]
		}));
		layoutDone = '';
	});

	$effect(() => {
		const a = activeStep;
		if (!nodes.some((n) => n.data.active !== (n.id === a))) return;
		nodes = nodes.map((n) => (n.data.active === (n.id === a) ? n : { ...n, data: { ...n.data, active: n.id === a } }));
	});

	let layoutDone = $state('');
	const layoutKey = $derived(builtKey + '|' + nodes.map((n) => `${n.id}:${Math.round(n.measured?.height ?? 0)}`).join(','));
	$effect(() => {
		const key = layoutKey;
		if (key === layoutDone) return;
		if (nodes.length === 0 || !nodes.every((n) => n.measured?.width && n.measured?.height)) return;
		layoutDone = key;
		nodes = layoutGraph(nodes, edges, { rankSep: 70, nodeSep: 24 });
		tick().then(() => fitFn?.({ duration: 0 }));
	});

	function resetLayout() {
		nodes = layoutGraph(nodes, edges, { rankSep: 70, nodeSep: 24 });
		tick().then(() => fitFn?.({ duration: 200 }));
	}
</script>

<div bind:this={rootEl} class="flex h-full min-h-0" class:select-none={listDragging}>
	<!-- left: searchable column list (resizable width) -->
	<div class="relative flex shrink-0 flex-col border-r border-border" style:width="{listW}px">
		<div class="border-b border-border p-3">
			<div class="bg-secondary flex items-center gap-2 rounded-md border border-border px-2.5 py-1.5">
				<span class="text-[var(--sb-text-faint)] text-[11px]">⌕</span>
				<input bind:value={colQuery} placeholder="Search columns…" class="text-foreground placeholder:text-[var(--sb-text-faint)] w-full bg-transparent text-[12px] outline-none" />
				{#if colQuery}<button class="text-[var(--sb-text-faint)] hover:text-foreground text-[11px]" onclick={() => (colQuery = '')}>✕</button>{/if}
			</div>
			<div class="text-[var(--sb-text-faint)] mt-2 px-0.5 text-[11.5px] font-medium">
				{#if colQuery}{matchedColumns.length} of {totalColumns} columns{:else}{totalColumns} columns{/if}
			</div>
		</div>
		<div class="sb-scroll min-h-0 flex-1 overflow-auto py-1">
			{#each focalColumns as c (c.name)}
				{@const cid = `${focal}.${c.name}`}
				{@const lens = incomingLens.get(cid)}
				<button
					class="flex w-full items-center gap-2 px-3 py-2 text-left text-[12px]"
					style:background={selectedCol === cid ? 'color-mix(in srgb, var(--primary) 14%, transparent)' : 'transparent'}
					onclick={() => (selectedCol = cid)}
				>
					{#if selectedCol === cid}<span class="bg-primary absolute left-0 h-5 w-[2px]"></span>{/if}
					<span class="truncate" style:color={selectedCol === cid ? 'var(--foreground)' : 'var(--muted-foreground)'}>{c.name}</span>
					<span class="ml-auto shrink-0 text-[11.5px]" class:text-foreground={lens === 'transform'} class:text-muted-foreground={lens !== 'transform'}>{lens ? lensLabel[lens] : ''}</span>
					<span class="text-[var(--sb-text-faint)] w-[52px] shrink-0 text-right font-mono text-[11px]">{c.type.toLowerCase()}</span>
				</button>
			{:else}
				<div class="text-[var(--sb-text-faint)] px-3 py-4 text-center text-[11.5px]">No columns match.</div>
			{/each}
			{#if truncated}
				<div class="text-[var(--sb-text-faint)] border-t border-[var(--border-subtle)] px-3 py-2 text-[12px]">
					Showing first {COL_LIST_CAP} of {matchedColumns.length} — search to narrow.
				</div>
			{/if}
		</div>
		<!-- width resize handle -->
		<div
			class="sb-vhandle absolute top-0 bottom-0 right-0 z-20 w-1.5 translate-x-1/2 cursor-col-resize"
			onpointerdown={startList}
			onpointermove={onList}
			onpointerup={endList}
			role="separator"
			aria-label="Resize column list"
			tabindex="-1"
		></div>
	</div>

	<!-- right: selected column's chain -->
	<div class="flex min-h-0 flex-1 flex-col">
		<div class="flex items-center gap-4 border-b border-border px-4 py-2">
			{#if selectedCol}
				<span class="text-[12px]"><span class="text-muted-foreground">{nameOf(modelOf(selectedCol))}.</span><span class="text-foreground">{colOf(selectedCol)}</span></span>
			{/if}
			<div class="ml-auto flex items-center gap-3.5">
				{#each lenses as l (l)}
					<span class="flex items-center gap-1.5 text-[12px]"><svg width="18" height="6" aria-hidden="true"><line x1="0" y1="3" x2="18" y2="3" style={lensStroke[l]} /></svg><span class="text-muted-foreground">{lensLabel[l]}</span></span>
				{/each}
				<button class="text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] rounded-[2px] border border-border px-2 py-1 text-[11px]" onclick={resetLayout} title="Reset layout">⤢ Reset</button>
			</div>
		</div>
		<div class="flex min-h-0 flex-1">
		<div
			class="sb-flow relative min-h-0 min-w-0 flex-1"
			style="background: var(--sb-inset)"
			role="presentation"
			onmouseover={(e) => {
				const el = (e.target as HTMLElement).closest('[data-colid]') as HTMLElement | null;
				hoverNode = el?.dataset.colid ?? null;
			}}
			onfocus={(e) => {
				const el = (e.target as HTMLElement).closest('[data-colid]') as HTMLElement | null;
				hoverNode = el?.dataset.colid ?? null;
			}}
			onblur={() => (hoverNode = null)}
			onmouseout={(e) => {
				const to = (e.relatedTarget as HTMLElement | null)?.closest?.('[data-colid]');
				if (!to) hoverNode = null;
			}}
			onclick={(e) => {
				const el = (e.target as HTMLElement).closest('[data-colid]') as HTMLElement | null;
				// click a node -> pin it (toggle off if same); click empty canvas -> unpin
				pinnedNode = el?.dataset.colid ? (pinnedNode === el.dataset.colid ? null : el.dataset.colid) : null;
			}}
		>
			{#if selectedCol}
				<SvelteFlow bind:nodes bind:edges {nodeTypes} fitView fitViewOptions={{ padding: 0.12, maxZoom: 1 }} nodesDraggable proOptions={{ hideAttribution: true }}>
					<FlowController bind:fitView={fitFn} />
					<Background gap={64} bgColor="var(--sb-inset)" patternColor="var(--sb-grid)" />
					<Controls />
				</SvelteFlow>

			{:else}
				<div class="text-[var(--sb-text-faint)] grid h-full place-items-center text-[12px]">Select a column to trace its lineage.</div>
			{/if}
		</div>

		{#if selectedCol}
			<aside class="derivation sb-scroll">
				<div class="dv-head">
					<strong>How it's derived</strong>
					<span>{hops.length - 1} {hops.length - 1 === 1 ? 'step' : 'steps'} from source</span>
				</div>
				<ol class="dv-steps">
					{#each hops as st (st.cid)}
						<li class:focal={st.depth === 0} class:active={activeStep === st.cid}>
							<button onmouseenter={() => (hoverNode = st.cid)} onmouseleave={() => (hoverNode = null)} onclick={() => (pinnedNode = pinnedNode === st.cid ? null : st.cid)}>
								<i></i>
								<span class="dv-name"><span class="m">{st.model}.</span>{st.col}</span>
								<span class="dv-lens" class:strong={st.lens === 'transform'}>{lensLabel[st.lens]}</span>
								{#if st.expr && st.lens !== 'raw'}<code>{@html highlightSqlbuild(st.expr)}</code>{/if}
							</button>
						</li>
					{/each}
				</ol>
				{#if uses.length}
					<div class="dv-sub">Used downstream</div>
					<ol class="dv-steps">
						{#each uses as st (st.cid)}
							<li class:active={activeStep === st.cid}>
								<button onmouseenter={() => (hoverNode = st.cid)} onmouseleave={() => (hoverNode = null)} onclick={() => (pinnedNode = pinnedNode === st.cid ? null : st.cid)}>
									<i></i>
									<span class="dv-name"><span class="m">{st.model}.</span>{st.col}</span>
									<span class="dv-lens" class:strong={st.lens === 'transform'}>{lensLabel[st.lens]}</span>
									{#if st.expr}<code>{@html highlightSqlbuild(st.expr)}</code>{/if}
								</button>
							</li>
						{/each}
					</ol>
				{/if}
				{#if sqlParts && pinnedNode}
					<div class="dv-sub">SQL · {sqlParts.model}.sql</div>
					<pre class="dv-sql">{@html sqlParts.before}{#if sqlParts.match}<mark bind:this={markEl}>{@html sqlParts.match}</mark>{/if}{@html sqlParts.after}</pre>
				{:else}
					<p class="dv-hint">Select a step to see it in the model's SQL.</p>
				{/if}
			</aside>
		{/if}
		</div>
	</div>
</div>

<style>
	.sb-vhandle::after {
		content: '';
		position: absolute;
		top: 0;
		bottom: 0;
		left: 50%;
		width: 1px;
		transform: translateX(-0.5px);
		background: transparent;
		transition: background 0.12s;
	}
	.sb-vhandle:hover::after {
		background: var(--primary);
	}
	.sb-flow :global(.svelte-flow__controls-button) {
		background: var(--card);
		border-bottom: 1px solid var(--border);
	}
	.sb-flow :global(.svelte-flow__controls-button svg) {
		fill: var(--muted-foreground);
	}
	.sb-flow :global(.svelte-flow__attribution) {
		display: none;
	}
	.derivation {
		width: 360px;
		flex: 0 0 360px;
		overflow-y: auto;
		border-left: 1px solid var(--border);
		background: var(--card);
	}
	.dv-head {
		padding: 14px 16px 10px;
	}
	.dv-head strong {
		display: block;
		font-size: 13px;
		font-weight: 600;
	}
	.dv-head span {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.dv-sub {
		padding: 14px 16px 6px;
		border-top: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 600;
	}
	.dv-steps {
		margin: 0;
		padding: 0 8px 8px;
		list-style: none;
	}
	.dv-steps li {
		position: relative;
	}
	.dv-steps li + li::before {
		content: '';
		position: absolute;
		top: -8px;
		left: 15px;
		height: 18px;
		border-left: 1px solid var(--border-strong, var(--border));
	}
	.dv-steps button {
		display: grid;
		width: 100%;
		grid-template-columns: 14px minmax(0, 1fr) auto;
		align-items: center;
		gap: 4px 8px;
		padding: 8px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--foreground);
		text-align: left;
	}
	.dv-steps button:hover,
	.dv-steps li.active button {
		background: var(--sb-hover);
	}
	.dv-steps i {
		width: 8px;
		height: 8px;
		margin-left: 3px;
		border: 1.5px solid var(--muted-foreground);
		border-radius: 50%;
	}
	.dv-steps li.focal i {
		border-color: var(--primary);
		background: var(--primary);
	}
	.dv-name {
		overflow: hidden;
		font: 12.5px var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dv-name .m {
		color: var(--muted-foreground);
	}
	.dv-lens {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.dv-lens.strong {
		color: var(--foreground);
		font-weight: 600;
	}
	.dv-steps code {
		grid-column: 2 / -1;
		overflow: hidden;
		padding: 3px 7px;
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--sb-mono, var(--foreground));
		font: 11.5px/1.5 var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dv-sql {
		max-height: 280px;
		margin: 0 16px 16px;
		overflow: auto;
		padding: 10px 12px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--sb-mono, var(--foreground));
		font: 11.5px/1.6 var(--font-mono);
	}
	.dv-sql mark {
		padding: 0 2px;
		border-radius: 2px;
		background: color-mix(in srgb, var(--primary) 26%, transparent);
		color: inherit;
	}
	.dv-hint {
		margin: 0;
		padding: 12px 16px 16px;
		border-top: 1px solid var(--border);
		color: var(--sb-text-faint);
		font-size: 12px;
	}
</style>
