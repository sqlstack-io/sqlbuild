<script lang="ts">
	import CopyIcon from '@lucide/svelte/icons/copy';
	import CheckIcon from '@lucide/svelte/icons/check';
	import XIcon from '@lucide/svelte/icons/x';
	import WrapTextIcon from '@lucide/svelte/icons/wrap-text';
	import { compileRefs, highlightLines, outlineSql } from '$lib/sql/outline';
	import { diffLines, diffStats, foldDiff, type DiffLine, type DiffRow } from '$lib/sql/diff';
	import type { SqlVersion } from '$lib/sql/versions';

	type Mode = 'raw' | 'compiled' | 'diff' | 'drift';

	let {
		name,
		raw,
		compiled: compiledSql,
		mode = $bindable<Mode>('raw'),
		versions = [],
		upstreamNote,
		onRef,
		onClose
	}: {
		name: string;
		raw: string;
		/** Compiled SQL from the server; defaults to substituting refs in `raw`. */
		compiled?: string;
		mode?: Mode;
		versions?: SqlVersion[];
		upstreamNote?: string;
		onRef?: (model: string) => void;
		onClose?: () => void;
	} = $props();

	let baselineId = $state('built');
	let driftSide = $state<'compiled' | 'raw'>('compiled');
	// Drift layout: remember an explicit choice; otherwise pick by available width.
	const LAYOUT_KEY = 'sb-drift-layout';
	let layoutChoice = $state<'unified' | 'split' | null>(
		typeof localStorage !== 'undefined' ? (localStorage.getItem(LAYOUT_KEY) as 'unified' | 'split' | null) : null
	);
	let viewerW = $state(0);
	const layout = $derived(layoutChoice ?? (viewerW >= 1100 ? 'split' : 'unified'));
	function chooseLayout(v: 'unified' | 'split') {
		layoutChoice = v;
		localStorage.setItem(LAYOUT_KEY, v);
	}
	let showAll = $state(false);
	const compiled = $derived(compiledSql ?? compileRefs(raw));
	const baseline = $derived(versions.find((v) => v.id === baselineId) ?? versions[0]);
	const baseCompiled = $derived(baseline ? (baseline.compiled ?? compileRefs(baseline.raw)) : '');
	const rawDrift = $derived(!!baseline && baseline.raw !== raw);
	const compiledDrift = $derived(!!baseline && baseCompiled !== compiled);
	const driftBefore = $derived(driftSide === 'raw' ? (baseline?.raw ?? '') : baseCompiled);
	const driftAfter = $derived(driftSide === 'raw' ? raw : compiled);
	const driftLines = $derived(diffLines(driftBefore, driftAfter));
	const driftRows = $derived(foldDiff(driftLines, showAll ? Number.MAX_SAFE_INTEGER : 3));
	const stats = $derived(diffStats(driftLines));
	const beforeHl = $derived(highlightLines(driftBefore, false));
	const afterHl = $derived(highlightLines(driftAfter, false));
	const lineHtml = (l: DiffLine) => (l.op === 'del' ? beforeHl[(l.a ?? 1) - 1] : afterHl[(l.b ?? 1) - 1]) || ' ';
	const against = $derived(baseline ? `the ${baseline.label.charAt(0).toLowerCase()}${baseline.label.slice(1)}` : '');
	const explanation = $derived.by(() => {
		if (!baseline) return { tone: 'none', text: 'No stored versions to compare.' };
		if (rawDrift) return { tone: 'direct', text: `Direct change — this model's SQL has changed since ${against}.` };
		if (compiledDrift)
			return { tone: 'compiled', text: `Raw SQL matches ${against}, but the compiled SQL differs — a macro, variable or reference now resolves differently.` };
		if (upstreamNote) return { tone: 'upstream', text: `SQL matches ${against}. Upstream drift: ${upstreamNote}` };
		return { tone: 'none', text: `No drift — identical to ${against}.` };
	});
	type SplitRow = { key: string; fold?: number; left?: DiffLine; right?: DiffLine };
	const splitRows = $derived.by(() => {
		const out: SplitRow[] = [];
		let dels: DiffLine[] = [];
		let adds: DiffLine[] = [];
		const flush = () => {
			for (let i = 0; i < Math.max(dels.length, adds.length); i++) out.push({ key: `p${out.length}`, left: dels[i], right: adds[i] });
			dels = [];
			adds = [];
		};
		for (const r of driftRows as DiffRow[]) {
			if (r.op === 'fold') {
				flush();
				out.push({ key: r.key, fold: r.count });
			} else if (r.op === 'del') dels.push(r);
			else if (r.op === 'add') adds.push(r);
			else {
				flush();
				out.push({ key: `s${out.length}`, left: r, right: r });
			}
		}
		flush();
		return out;
	});

	let wrap = $state(false);
	let copied = $state(false);
	let activeLine = $state<number | null>(null);
	let codeEl = $state<HTMLDivElement>();

	const outline = $derived(outlineSql(raw));
	const rawLines = $derived(highlightLines(raw, true));
	const compiledLines = $derived(highlightLines(compiled, false));
	const primaryLines = $derived(mode === 'compiled' ? compiledLines : rawLines);

	function jump(line: number) {
		if (mode === 'compiled') mode = 'raw';
		activeLine = line;
		codeEl?.querySelector(`[data-line="${line}"]`)?.scrollIntoView({ block: 'center', behavior: 'smooth' });
	}

	function onCodeClick(e: MouseEvent) {
		const ref = (e.target as HTMLElement).closest<HTMLElement>('.sql-ref')?.dataset.ref;
		if (ref) onRef?.(ref);
	}

	async function copy() {
		await navigator.clipboard?.writeText(mode === 'compiled' ? compiled : raw);
		copied = true;
		setTimeout(() => (copied = false), 1200);
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose?.()} />

<section class="sql-viewer" aria-label="SQL for {name}" bind:clientWidth={viewerW}>
	<header>
		<span class="file">{name}.sql</span>
		<span class="meta">{raw.split('\n').length} lines</span>
		<div class="modes">
			{#each [['raw', 'Raw'], ['compiled', 'Compiled'], ['diff', 'Side by side'], ...(versions.length ? [['drift', 'Drift']] : [])] as [m, label] (m)}
				<button class:on={mode === m} onclick={() => (mode = m as Mode)}>{label}</button>
			{/each}
		</div>
		<button class="icon" class:on={wrap} onclick={() => (wrap = !wrap)} title="Wrap long lines" aria-label="Wrap long lines"><WrapTextIcon size={15} /></button>
		<button class="icon" onclick={copy} title="Copy SQL" aria-label="Copy SQL">
			{#if copied}<CheckIcon size={15} />{:else}<CopyIcon size={15} />{/if}
		</button>
		{#if onClose}
			<button class="icon" onclick={() => onClose?.()} title="Close (Esc)" aria-label="Close SQL"><XIcon size={16} /></button>
		{/if}
	</header>

	{#if mode === 'drift'}
		<div class="drift-bar">
			<label>Compare with
				<select bind:value={baselineId}>
					{#each versions as v (v.id)}<option value={v.id}>{v.label} · {v.detail}</option>{/each}
				</select>
			</label>
			<div class="mini-seg">
				{#each [['compiled', 'Compiled'], ['raw', 'Raw']] as [v, l] (v)}
					<button class:on={driftSide === v} onclick={() => (driftSide = v as 'compiled' | 'raw')}>{l}</button>
				{/each}
			</div>
			<div class="mini-seg">
				{#each [['unified', 'Unified'], ['split', 'Split']] as [v, l] (v)}
					<button class:on={layout === v} onclick={() => chooseLayout(v as 'unified' | 'split')}>{l}</button>
				{/each}
			</div>
			<span class="stats"><b class="add">+{stats.added}</b><b class="del">−{stats.removed}</b></span>
			<button class="text-btn" onclick={() => (showAll = !showAll)}>{showAll ? 'Fold unchanged' : 'Show all lines'}</button>
		</div>
		<div class="drift-note {explanation.tone}"><i></i>{explanation.text}</div>
	{/if}

	<div class="body" class:diff={mode === 'diff' || mode === 'drift'}>
		{#if mode !== 'diff' && mode !== 'drift'}
		<nav class="outline sb-scroll">
			{#if outline.config.length}
				<div class="group">
					<h4>Config</h4>
					{#each outline.config as c (c.key)}
						<div class="config"><span>{c.key}</span><b>{c.value}</b></div>
					{/each}
				</div>
			{/if}
			{#if outline.refs.length}
				<div class="group">
					<h4>References</h4>
					{#each outline.refs as r (r.label)}
						<div class="item-row">
							<button class="item" onclick={() => jump(r.line)}>{r.label}</button>
							<button class="go" onclick={() => onRef?.(r.label)} title="Select in graph">↗</button>
						</div>
					{/each}
				</div>
			{/if}
			{#if outline.steps.length || outline.finalSelect}
				<div class="group">
					<h4>Steps</h4>
					{#each outline.steps as s (s.label)}
						<button class="item" onclick={() => jump(s.line)}>{s.label}<span>L{s.line}</span></button>
					{/each}
					{#if outline.finalSelect}
						<button class="item final" onclick={() => jump(outline.finalSelect!)}>Final select<span>L{outline.finalSelect}</span></button>
					{/if}
				</div>
			{/if}
			{#if outline.columns.length}
				<div class="group">
					<h4>Output · {outline.columns.length}</h4>
					{#each outline.columns as c (c)}<div class="col">{c}</div>{/each}
				</div>
			{/if}
		</nav>
		{/if}

		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
		{#if mode === 'drift'}
			<div class="code drift sb-scroll" class:wrap>
				{#if stats.added + stats.removed === 0}
					<div class="no-diff">No {driftSide} SQL differences from {baseline?.label ?? 'the baseline'}.</div>
				{:else if layout === 'unified'}
					<div class="pane">
						{#each driftRows as r (r.op === 'fold' ? r.key : `${r.op}${r.a ?? ''}-${r.b ?? ''}`)}
							{#if r.op === 'fold'}
								<button class="fold" onclick={() => (showAll = true)}>⋯ {r.count} unchanged lines</button>
							{:else}
								<div class="dline {r.op}">
									<span class="ln">{r.a ?? ''}</span><span class="ln">{r.b ?? ''}</span>
									<span class="sign">{r.op === 'add' ? '+' : r.op === 'del' ? '−' : ''}</span>
									<code>{@html lineHtml(r)}</code>
								</div>
							{/if}
						{/each}
					</div>
				{:else}
					<div class="pane split">
						{#each splitRows as r (r.key)}
							{#if r.fold}
								<button class="fold" onclick={() => (showAll = true)}>⋯ {r.fold} unchanged lines</button>
							{:else}
								<div class="split-row">
									<div class="dline {r.left ? (r.left.op === 'same' ? 'same' : 'del') : 'empty'}">
										<span class="ln">{r.left?.a ?? ''}</span><code>{@html r.left ? (r.left.op === 'same' ? afterHl[(r.left.b ?? 1) - 1] : lineHtml(r.left)) || ' ' : ' '}</code>
									</div>
									<div class="dline {r.right ? (r.right.op === 'same' ? 'same' : 'add') : 'empty'}">
										<span class="ln">{r.right?.b ?? ''}</span><code>{@html r.right ? lineHtml(r.right) : ' '}</code>
									</div>
								</div>
							{/if}
						{/each}
					</div>
				{/if}
			</div>
		{:else}
		<div class="code sb-scroll" class:wrap class:diff={mode === 'diff'} bind:this={codeEl} onclick={onCodeClick}>
			<div class="pane">
				{#if mode === 'diff'}<div class="pane-title">Raw</div>{/if}
				{#each primaryLines as line, i (i)}
					<div class="line" class:active={activeLine === i + 1} data-line={i + 1}>
						<span class="ln">{i + 1}</span><code>{@html line || ' '}</code>
					</div>
				{/each}
			</div>
			{#if mode === 'diff'}
				<div class="pane">
					<div class="pane-title">Compiled</div>
					{#each compiledLines as line, i (i)}
						<div class="line"><span class="ln">{i + 1}</span><code>{@html line || ' '}</code></div>
					{/each}
				</div>
			{/if}
		</div>
		{/if}
	</div>
</section>

<style>
	.sql-viewer {
		display: flex;
		height: 100%;
		flex-direction: column;
		background: var(--card);
	}
	header {
		display: flex;
		height: 46px;
		flex: 0 0 46px;
		align-items: center;
		gap: 10px;
		padding: 0 10px 0 16px;
		border-bottom: 1px solid var(--border);
	}
	.file {
		font: 600 13px var(--font-mono);
	}
	.meta {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.modes {
		display: flex;
		gap: 2px;
		margin-left: auto;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.modes button {
		height: 26px;
		padding: 0 10px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.modes button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.14);
	}
	.icon {
		display: grid;
		width: 30px;
		height: 30px;
		place-items: center;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
	}
	.icon:hover,
	.icon.on {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.body {
		display: grid;
		min-height: 0;
		flex: 1;
		grid-template-columns: 210px minmax(0, 1fr);
	}
	.body.diff {
		grid-template-columns: minmax(0, 1fr);
	}
	.outline {
		overflow-y: auto;
		padding: 12px 10px;
		border-right: 1px solid var(--border);
	}
	.group + .group {
		margin-top: 18px;
	}
	h4 {
		margin: 0 0 6px 6px;
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 500;
	}
	.config {
		display: flex;
		gap: 8px;
		padding: 3px 6px;
		font-size: 12px;
	}
	.config span {
		color: var(--muted-foreground);
	}
	.config b {
		margin-left: auto;
		overflow: hidden;
		font: 500 11.5px var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.item-row {
		display: flex;
		align-items: center;
	}
	.item {
		display: flex;
		width: 100%;
		align-items: center;
		padding: 4px 6px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--foreground);
		font: 12px var(--font-mono);
		text-align: left;
	}
	.item:hover {
		background: var(--sb-hover);
	}
	.item span {
		margin-left: auto;
		color: var(--sb-text-faint);
		font: 11px var(--font-sans);
	}
	.item.final {
		font-family: var(--font-sans);
	}
	.go {
		border: 0;
		background: transparent;
		color: var(--primary);
		font-size: 12px;
	}
	.col {
		padding: 2px 6px;
		color: var(--muted-foreground);
		font: 12px var(--font-mono);
	}
	.code {
		display: grid;
		overflow: auto;
		background: var(--sb-inset);
		color: var(--sb-mono);
		font: 12.5px/1.75 var(--font-mono);
	}
	.code.diff {
		grid-template-columns: 1fr 1fr;
	}
	.pane {
		min-width: max-content;
		padding: 10px 0 40px;
	}
	.code.diff {
		align-items: start;
	}
	.code.diff .pane {
		min-width: 0;
		overflow-x: auto;
	}
	.code.diff .line {
		width: max-content;
		min-width: 100%;
	}
	.code.diff .pane + .pane {
		border-left: 1px solid var(--border);
	}
	.code.wrap .pane {
		min-width: 0;
	}
	.pane-title {
		padding: 0 16px 8px 52px;
		color: var(--muted-foreground);
		font: 500 12px var(--font-sans);
	}
	.line {
		display: flex;
		padding-right: 20px;
	}
	.line:hover {
		background: color-mix(in srgb, var(--sb-hover) 55%, transparent);
	}
	.line.active {
		background: color-mix(in srgb, var(--primary) 14%, transparent);
	}
	.ln {
		width: 44px;
		flex: 0 0 44px;
		padding-right: 14px;
		color: var(--sb-text-faint);
		text-align: right;
		user-select: none;
	}
	.line code {
		font: inherit;
		white-space: pre;
	}
	.code.wrap .line code {
		padding-left: 2ch;
		text-indent: -2ch;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.code :global(.sql-ref) {
		padding: 0;
		border: 0;
		border-bottom: 1px dashed color-mix(in srgb, var(--syn-string) 60%, transparent);
		background: transparent;
		color: var(--syn-string);
		font: inherit;
		cursor: pointer;
	}
	.code :global(.sql-ref:hover) {
		border-bottom-style: solid;
		color: var(--primary);
	}

	.drift-bar {
		display: flex;
		height: 44px;
		flex: 0 0 44px;
		align-items: center;
		gap: 10px;
		padding: 0 16px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.drift-bar label {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--muted-foreground);
	}
	.drift-bar select {
		height: 28px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.mini-seg {
		display: flex;
		gap: 2px;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.mini-seg button {
		height: 24px;
		padding: 0 9px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.mini-seg button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
	}
	.stats {
		display: flex;
		gap: 8px;
		margin-left: auto;
		font: 600 12px var(--font-mono);
	}
	.stats .add {
		color: var(--sb-success);
	}
	.stats .del {
		color: var(--sb-error);
	}
	.text-btn {
		border: 0;
		background: transparent;
		color: var(--primary);
		font-size: 12.5px;
	}
	.drift-note {
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 9px 16px;
		border-bottom: 1px solid var(--border);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.drift-note i {
		width: 8px;
		height: 8px;
		flex: 0 0 8px;
		border-radius: 50%;
		background: var(--sb-text-faint);
	}
	.drift-note.direct i,
	.drift-note.compiled i {
		background: var(--sb-stale);
	}
	.drift-note.upstream i {
		border: 1.5px dashed var(--sb-stale);
		background: transparent;
	}
	.code.drift {
		display: block;
	}
	.no-diff {
		padding: 40px 20px;
		color: var(--muted-foreground);
		font: 13px var(--font-sans);
		text-align: center;
	}
	.dline {
		display: flex;
		min-width: max-content;
		padding-right: 20px;
	}
	.dline.add {
		background: color-mix(in srgb, var(--sb-success) 13%, transparent);
	}
	.dline.del {
		background: color-mix(in srgb, var(--sb-error) 12%, transparent);
	}
	.dline.empty {
		background: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in srgb, var(--sb-hover) 70%, transparent) 6px 7px);
	}
	.dline code {
		font: inherit;
		white-space: pre;
	}
	.code.wrap .dline code {
		white-space: pre-wrap;
		word-break: break-word;
	}
	.dline .ln {
		width: 40px;
		flex: 0 0 40px;
		padding-right: 10px;
		color: var(--sb-text-faint);
		text-align: right;
		user-select: none;
	}
	.sign {
		width: 18px;
		flex: 0 0 18px;
		color: var(--muted-foreground);
		text-align: center;
		user-select: none;
	}
	.dline.add .sign {
		color: var(--sb-success);
	}
	.dline.del .sign {
		color: var(--sb-error);
	}
	.fold {
		display: block;
		width: 100%;
		padding: 4px 0 4px 98px;
		border: 0;
		border-block: 1px solid var(--border-subtle);
		background: color-mix(in srgb, var(--sb-hover) 50%, transparent);
		color: var(--muted-foreground);
		font: 12px var(--font-sans);
		text-align: left;
	}
	.fold:hover {
		color: var(--primary);
	}
	.pane.split {
		min-width: 0;
	}
	.split-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
	}
	.split-row > .dline {
		min-width: 0;
		overflow: hidden;
	}
	.split-row > .dline code {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.code.wrap .split-row > .dline code {
		overflow: visible;
	}
	.split-row > .dline + .dline {
		border-left: 1px solid var(--border);
	}
</style>
