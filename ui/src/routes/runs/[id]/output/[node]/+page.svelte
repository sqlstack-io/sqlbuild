<script lang="ts">
	import { page } from '$app/state';
	import { tick, untrack } from 'svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import PageBody from '$lib/components/page-body.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import SearchIcon from '@lucide/svelte/icons/search';
	import type { NodeOutputIndex, NodeOutputPage, NodeOutputRow } from '$lib/api';
	import { to } from '$lib/paths';
	import { app } from '$lib/state/app.svelte';
	import { nodeLabel, shortRunId } from '$lib/operate/labels';

	// A node's whole output, read a page at a time from the output store: tail,
	// jump to a line, page, search the whole output, jump between errors and warnings from the index,
	// similar lines folded, and a download. The browser only ever holds one page.
	const runId = $derived(page.params.id ?? '');
	const nodeKey = $derived(page.params.node ?? '');
	const ROWS = 200;
	const fmt = (n: number) => n.toLocaleString('en-GB');

	let index = $state<NodeOutputIndex | null>(null);
	let view = $state<NodeOutputPage | null>(null);
	let loading = $state(false);
	let failed = $state<string | null>(null);
	let collapse = $state(true);
	/** The line to point at: a search hit, a jump target, or an error or warning. */
	let target = $state<number | null>(null);
	let lineInput = $state('');
	let query = $state('');
	/** Where a search stands: this match of how many when the output was searched whole, else just its line. */
	function hitText(h: { count: number | null; line: number | null; rank: number | null; continueFrom: number | null }): string {
		if (h.line == null && h.continueFrom != null) return `No match up to line ${fmt(h.continueFrom)} · search again to go on`;
		if (h.line == null) return 'No matches';
		if (h.count != null && h.rank != null) return `hit ${fmt(h.rank)} of ${fmt(h.count)} · line ${fmt(h.line)}`;
		return `match at line ${fmt(h.line)}`;
	}
	let hits = $state<{ query: string; count: number | null; line: number | null; rank: number | null; continueFrom: number | null } | null>(null);
	let searching = $state(false);
	let stream = $state<'all' | 'stdout' | 'stderr'>('all');
	let scroller = $state<HTMLDivElement>();
	const supported = $derived(!!app.client.getNodeOutputPage && !!app.client.getNodeOutputIndex);

	async function readIndex() {
		if (!app.scope || !app.client.getNodeOutputIndex) return null;
		index = await app.client.getNodeOutputIndex(app.scope, runId, nodeKey);
		return index;
	}
	async function load(from: number, direction: 'forward' | 'backward' = 'forward', point: number | null = null) {
		if (!app.scope || !app.client.getNodeOutputPage) return;
		loading = true;
		failed = null;
		try {
			// A jump's target opens the folded block it lands in.
			view = await app.client.getNodeOutputPage(app.scope, runId, nodeKey, { from, rows: ROWS, direction, collapse, stream, around: point ?? undefined });
			target = point;
			await tick();
			// The tail sits at the bottom; a jump puts its line in the middle of the view.
			const marked = scroller?.querySelector('.target');
			if (marked) marked.scrollIntoView({ block: 'center' });
			else if (scroller) scroller.scrollTop = direction === 'backward' ? scroller.scrollHeight : 0;
		} catch (e) {
			failed = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}
	// Opens on the tail, the newest lines, unless the link names a place: `?at=error` opens at the error
	// line with the lines before it in view and the error marked; `?line=N` at that line.
	$effect(() => {
		void runId;
		void nodeKey;
		const at = page.url.searchParams.get('at');
		const line = Number(page.url.searchParams.get('line') ?? 0);
		untrack(async () => {
			const ix = await readIndex();
			if (!ix) return;
			const errors = ix.marks.filter((m) => m.level === 'error');
			const target = at === 'error' && errors.length ? errors[errors.length - 1].line : line > 0 ? Math.min(line, ix.total) : null;
			if (target != null) await load(Math.max(1, target - 45), 'forward', target);
			else await load(ix.total + 1, 'backward');
		});
	});
	// While the node still runs, the index grows; refresh it so the counts and Tail stay current.
	$effect(() => {
		if (!index?.live) return;
		const t = setInterval(() => void readIndex(), 5_000);
		return () => clearInterval(t);
	});

	const total = $derived(index?.total ?? 0);
	const anchor = $derived(target ?? view?.from ?? 1);
	function jumpToLine() {
		const n = Number(lineInput.replace(/[^\d]/g, ''));
		if (!n) return;
		const line = Math.min(Math.max(1, n), total);
		void load(Math.max(1, line - 3), 'forward', line);
	}
	let searchAbort: AbortController | null = null;
	async function search(direction: 'forward' | 'backward') {
		if (!app.scope || !app.client.searchNodeOutput || !query.trim()) return;
		// A new search replaces one still going.
		searchAbort?.abort();
		const abort = (searchAbort = new AbortController());
		searching = true;
		const again = hits?.query === query;
		const from = again && hits!.continueFrom != null ? hits!.continueFrom : again && hits!.line != null ? hits!.line + (direction === 'forward' ? 1 : -1) : direction === 'forward' ? (view?.from ?? 1) : (view?.to ?? total);
		let res;
		try {
			res = await app.client.searchNodeOutput(app.scope, runId, nodeKey, query.trim(), from, direction, abort.signal);
		} catch (error) {
			if (abort.signal.aborted) return;
			throw error;
		} finally {
			if (searchAbort === abort) searching = false;
		}
		hits = { query, count: res.count, line: res.line, rank: res.rank, continueFrom: res.continueFrom ?? null };
		if (res.line != null) await load(Math.max(1, res.line - 3), 'forward', res.line);
	}
	function mark(level: 'error' | 'warn', direction: 'forward' | 'backward') {
		const marks = (index?.marks ?? []).filter((m) => m.level === level);
		const next = direction === 'forward' ? marks.find((m) => m.line > anchor) : [...marks].reverse().find((m) => m.line < anchor);
		if (next) void load(Math.max(1, next.line - 3), 'forward', next.line);
	}
	const errors = $derived((index?.marks ?? []).filter((m) => m.level === 'error').length);
	const warnings = $derived((index?.marks ?? []).filter((m) => m.level === 'warn').length);
	// Download is a plain link: the server streams the whole output as a file.
	let downloadUrl = $state<string | null>(null);
	$effect(() => {
		if (app.scope && app.client.nodeOutputDownloadUrl) downloadUrl = app.client.nodeOutputDownloadUrl(app.scope, runId, nodeKey);
	});
	const has = (r: NodeOutputRow, n: number | null) => n != null && (r.kind === 'line' ? r.line.n === n : n >= r.from && n < r.from + r.count);
	const mb = (bytes: number) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : bytes >= 1e6 ? `${(bytes / 1e6).toFixed(0)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} KB`);
	/** A search word highlighted in a line. */
	function parts(text: string): { t: string; hit: boolean }[] {
		const q = hits?.query.trim().toLowerCase();
		if (!q) return [{ t: text, hit: false }];
		const out: { t: string; hit: boolean }[] = [];
		let i = 0;
		const lower = text.toLowerCase();
		for (let j = lower.indexOf(q); j >= 0; j = lower.indexOf(q, j + q.length)) {
			if (j > i) out.push({ t: text.slice(i, j), hit: false });
			out.push({ t: text.slice(j, j + q.length), hit: true });
			i = j + q.length;
		}
		if (i < text.length) out.push({ t: text.slice(i), hit: false });
		return out;
	}
</script>

<AppTopbar title="Runs" />

<div class="viewer">
	<div class="head">
		<nav class="crumbs" aria-label="Breadcrumb">
			<a href={to('/runs')}>Runs</a><span>/</span>
			<a class="mono" href={to(`/runs/${runId}`)}>{shortRunId(runId)}</a><span>/</span>
			<span class="mono">{nodeLabel(nodeKey)}</span>
		</nav>
		<h2><span class="mono">{nodeLabel(nodeKey)}</span> output</h2>
		{#if index}
			<div class="facts">
				<span>{fmt(index.total)} lines</span><span class="sep">·</span><span>{mb(index.bytes)}</span><span class="sep">·</span>
				<span>{fmt(index.stdout)} stdout, {fmt(index.stderr)} stderr</span>
				{#if errors}<span class="sep">·</span><span class="err">{errors} errors</span>{/if}
				{#if warnings}<span class="sep">·</span><span class="warn">{warnings} warnings</span>{/if}
				{#if index.live}<span class="sep">·</span><span class="live">still writing</span>{/if}
			</div>
		{/if}
	</div>

	{#if !supported}
		<p class="empty">This server doesn't serve paged output yet.</p>
	{:else}
		<div class="toolbar">
			<SegmentedFilter
				label="Stream"
				bind:value={stream}
				onselect={(v) => ((stream = v), load(1))}
				options={[
					{ value: 'all', label: 'All' },
					{ value: 'stdout', label: 'stdout' },
					{ value: 'stderr', label: 'stderr' }
				]}
			/>
			<span class="divider"></span>
			<Button size="sm" variant="ghost" onclick={() => load(1)} disabled={loading}>Start</Button>
			<Button size="sm" variant="ghost" onclick={() => view && load(view.from, 'backward')} disabled={loading || !view || view.from <= 1}>‹ Previous</Button>
			<Button size="sm" variant="ghost" onclick={() => view && load(view.to)} disabled={loading || !view || view.to > total}>Next ›</Button>
			<Button size="sm" variant="ghost" onclick={async () => load(((await readIndex())?.total ?? total) + 1, 'backward')} disabled={loading}>Tail</Button>
			<form class="line" onsubmit={(e) => (e.preventDefault(), jumpToLine())}>
				<input bind:value={lineInput} placeholder="Line…" aria-label="Jump to line" inputmode="numeric" />
			</form>
			<span class="divider"></span>
			<form class="search" onsubmit={(e) => (e.preventDefault(), search('forward'))}>
				<SearchIcon size={13} />
				<input bind:value={query} placeholder="Search the whole output" aria-label="Search the whole output" />
			</form>
			{#if searching}<span class="faint">Searching {fmt(total)} lines…</span>
			{:else if hits && hits.query === query}<span class="faint">{hitText(hits)}</span>{/if}
			<Button size="sm" variant="ghost" onclick={() => search('backward')} disabled={!query.trim() || searching} title="Previous match">↑</Button>
			<Button size="sm" variant="ghost" onclick={() => search('forward')} disabled={!query.trim() || searching} title="Next match">↓</Button>
			<span class="divider"></span>
			<Button size="sm" variant="ghost" class="err" onclick={() => mark('error', 'forward')} disabled={!errors} title="Next error, from the index">Next error ↓</Button>
			<Button size="sm" variant="ghost" class="warn" onclick={() => mark('warn', 'forward')} disabled={!warnings} title="Next warning, from the index">Next warning ↓</Button>
			<Button size="sm" variant="ghost" onclick={() => mark('warn', 'backward')} disabled={!warnings} title="Previous warning">↑</Button>
			<span class="grow"></span>
			<label class="fold"><input type="checkbox" class="sb-check" bind:checked={collapse} onchange={() => view && load(view.from, 'forward', target)} /> Fold similar lines</label>
			{#if downloadUrl}<a class="download" href={downloadUrl} download="{runId}-{nodeKey}.log" title="Download the whole output as a file"><DownloadIcon size={14} />Download</a>{/if}
		</div>
		{#if view}<div class="where">Lines {fmt(view.from)}–{fmt(Math.max(view.from, view.to - 1))} of {fmt(total)}{#if loading}<span class="faint"> · loading…</span>{/if}</div>{/if}

		<PageBody padding="0" bind:scroller>
			{#if failed}
				<p class="empty">Couldn't read this output: {failed}</p>
			{:else if view}
				<div class="lines">
					{#each view.rows as r (r.kind === 'line' ? r.line.n : `s${r.from}`)}
						{#if r.kind === 'line'}
							<div class="ln {r.line.level}" class:stderr={r.line.stream === 'stderr'} class:target={r.line.n === target}>
								<span class="no">{fmt(r.line.n)}</span>
								<pre>{#each parts(r.line.text) as p, i (i)}{#if p.hit}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}</pre>
							</div>
						{:else}
							<!-- raw-button: a folded run of similar lines, which opens them unfolded from its first line -->
							<button class="ln similar" class:target={has(r, target)} onclick={() => ((collapse = false), load(r.from, 'forward', has(r, target) ? target : null))} title="Show these lines unfolded">
								<span class="no">{fmt(r.from)}</span>
								<pre><b>{fmt(r.count)} similar lines:</b> {r.sample.text}</pre>
							</button>
						{/if}
					{/each}
				</div>
			{:else if loading}
				<p class="empty">Reading the output…</p>
			{/if}
		</PageBody>
	{/if}
</div>

<style>
	.viewer {
		display: flex;
		min-height: 0;
		flex: 1;
		flex-direction: column;
	}
	.head {
		padding: 16px 24px 12px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.crumbs {
		display: flex;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.crumbs a:hover {
		color: var(--primary);
	}
	h2 {
		margin-top: 4px;
		font-family: var(--font-display);
		font-size: 18px;
		font-weight: 600;
	}
	.facts {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.sep,
	.faint {
		color: var(--sb-text-faint);
	}
	.err,
	.toolbar :global(.err) {
		color: var(--sb-error);
	}
	.warn,
	.toolbar :global(.warn) {
		color: var(--sb-warning);
	}
	.live {
		color: var(--sb-running);
	}
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px 4px;
		padding: 10px 20px 6px;
		font-size: 12px;
	}
	.divider {
		width: 1px;
		height: 18px;
		margin: 0 6px;
		background: var(--border-subtle);
	}
	.download {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 3px 8px;
		color: var(--muted-foreground);
	}
	.download:hover {
		color: var(--foreground);
	}
	.grow {
		flex: 1;
	}
	.line input {
		width: 90px;
	}
	.line input,
	.search {
		padding: 3px 8px;
		border: 1px solid var(--border-subtle);
		border-radius: 2px;
		background: transparent;
		font-size: 12px;
	}
	.search {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--sb-text-faint);
	}
	.search input {
		width: 220px;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
	}
	.fold {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		margin-right: 8px;
		color: var(--muted-foreground);
	}
	.where {
		padding: 0 24px 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.lines {
		padding: 4px 0 24px;
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.ln {
		display: flex;
		width: 100%;
		gap: 14px;
		padding: 1px 24px;
		text-align: left;
	}
	.ln:hover {
		background: var(--sb-hover);
	}
	.no {
		flex: none;
		width: 84px;
		color: var(--sb-text-faint);
		text-align: right;
		user-select: none;
	}
	pre {
		margin: 0;
		min-width: 0;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.ln.stderr {
		background: color-mix(in srgb, var(--sb-warning) 5%, transparent);
	}
	.ln.warn pre {
		color: color-mix(in srgb, var(--sb-warning) 75%, var(--foreground));
	}
	.ln.error pre {
		color: var(--sb-error);
	}
	.ln.target {
		box-shadow: inset 2px 0 0 var(--primary);
		background: color-mix(in srgb, var(--primary) 10%, transparent);
	}
	.ln.similar pre {
		color: var(--muted-foreground);
	}
	.ln.similar b {
		color: var(--sb-text-faint);
		font-weight: 500;
	}
	mark {
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-warning) 40%, transparent);
		color: inherit;
	}
	.empty {
		padding: 32px;
		color: var(--sb-text-faint);
		font-size: 12.5px;
		text-align: center;
	}
</style>
