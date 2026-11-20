<script lang="ts">
	// The plan's Renames and migrations section (CHI-1563): Renamed lists each renamed model as
	// old name → new name, the nodes the plan counts; Migration steps lists the steps on nodes counted
	// elsewhere, by kind, each naming its node and what it does. A row says only what its heading doesn't.
	// Each section's rows load from the server's plan a page at a time, as they are shown; the filter asks
	// the server for each section's rows by name (a renamed model by its old name too).
	import Button from '$lib/components/ui/button/button.svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import { plural } from '$lib/format/plural';
	import { to } from '$lib/paths';
	import type { PlanDocument, PlanModel, PlanPage, PlanPageSection, PlanSectionCount } from '$lib/api';
	import { STEP_KINDS, migrationSteps, renamedRow, stepRows, stepRowsOf, type RenamedRow, type StepRow } from './plan-steps';

	let {
		plan,
		counts,
		more,
		load
	}: {
		plan: PlanDocument;
		counts: PlanSectionCount[];
		more?: (section: PlanPageSection, limit: number) => Promise<void>;
		load?: (section: PlanPageSection, offset: number, limit: number, name: string) => Promise<PlanPage>;
	} = $props();

	/** Rows each "show more" adds. */
	const PAGE = 50;
	/** Rows shown at first: the renamed models, and each kind of step. */
	const FIRST: Record<string, number> = { renamed: 10, step: 5 };
	/** Rows of each section a filter reads. */
	const FILTER_READ = 200;
	/** How long the filter waits for typing to stop before asking the server. */
	const FILTER_WAIT_MS = 250;

	const sectionTotal = (s: PlanPageSection) => counts.find((c) => c.section === s)?.total ?? 0;
	/** A kind's steps: the migrations section also holds the renames' own migrations, which are not steps. */
	const otherSteps = $derived(STEP_KINDS.filter((k) => k.section !== 'migrations').reduce((n, k) => n + sectionTotal(k.section), 0));
	const total = (s: PlanPageSection) => (s === 'migrations' ? Math.max(0, migrationSteps(plan) - otherSteps) : sectionTotal(s));
	/** Rows of the section loaded so far, as the server pages them. */
	const read = (s: PlanPageSection) => (s === 'renamed' ? renamedModels.length : s === 'migrations' ? plan.migrations.length : stepRows(plan, s).length);

	const renamedModels = $derived(plan.models.filter((m) => m.reason === 'renamed' && m.action !== 'skip'));
	const renamed = $derived(renamedModels.map((m) => renamedRow(m)));
	const kinds = $derived(STEP_KINDS.map((k) => ({ ...k, total: total(k.section), rows: stepRows(plan, k.section) })).filter((k) => k.total > 0));
	const steps = $derived(kinds.reduce((n, k) => n + k.total, 0));
	const renamedTotal = $derived(total('renamed'));
	const listed = (s: PlanPageSection) => (s === 'renamed' ? renamed.length : stepRows(plan, s).length);

	let want = $state<Record<string, number>>({});
	const wanted = (s: PlanPageSection) => want[s] ?? (s === 'renamed' ? FIRST.renamed : FIRST.step);
	const sections = $derived<PlanPageSection[]>(['renamed', ...kinds.map((k) => k.section)]);

	let busy = $state(false);
	/** A failed page read: shown with Retry, and no more pages are read until then. */
	let pageError = $state<string | null>(null);
	/** A failed filter read, kept apart: a paging failure doesn't stop the filter, nor the other way round. */
	let filterError = $state<string | null>(null);
	// Another plan (a re-plan) starts with no failed reads.
	let erroredFor: unknown = null;
	$effect.pre(() => {
		if (plan.sections === erroredFor) return;
		erroredFor = plan.sections;
		pageError = null;
		filterError = null;
	});
	const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
	// Load what is shown, one page at a time, in order, until each section lists the rows it shows.
	$effect(() => {
		if (!more || busy || filtering || pageError) return;
		for (const s of sections) {
			if (listed(s) >= Math.min(wanted(s), total(s)) || read(s) >= sectionTotal(s)) continue;
			busy = true;
			more(s, PAGE)
				.catch((e: unknown) => (pageError = message(e)))
				.finally(() => (busy = false));
			return;
		}
	});
	const showMore = (s: PlanPageSection) => (want[s] = wanted(s) + PAGE);

	// ---- the name filter ----
	let filter = $state('');
	const wantedName = $derived(filter.trim().toLowerCase());
	const filtering = $derived(!!wantedName);
	/** The server finds rows by name, a section at a time; the default mock's whole plan, with every row, is filtered here. */
	const serverFilter = $derived(!!plan.sections && !!load);
	/** What the server found for the name asked in this plan, by section: the rows read and how many match. */
	let found = $state<{ name: string; sections: PlanDocument['sections']; pages: Partial<Record<PlanPageSection, { rows: unknown[]; total: number }>> } | null>(null);
	/** Whether `found` is for the name asked and the plan shown; another plan (a re-plan) asks again. */
	const current = (f: typeof found, name: string) => !!f && f.name === name && f.sections === plan.sections;
	$effect(() => {
		const name = wantedName;
		const ask = load;
		const sections = plan.sections;
		const all = ['renamed', ...STEP_KINDS.map((k) => k.section)].filter((s) => sectionTotal(s as PlanPageSection) > 0) as PlanPageSection[];
		if (!name || !serverFilter || !ask || filterError || current(found, name)) return;
		const timer = setTimeout(async () => {
			try {
				const pages = await Promise.all(all.map(async (s) => [s, await ask(s, 0, FILTER_READ, name)] as const));
				if (wantedName === name && plan.sections === sections) found = { name, sections, pages: Object.fromEntries(pages.map(([s, p]) => [s, { rows: p.rows, total: p.total }])) };
			} catch (e) {
				filterError = message(e);
			}
		}, FILTER_WAIT_MS);
		return () => clearTimeout(timer);
	});
	const matches = (text: string) => text.toLowerCase().includes(wantedName);
	/** The rows the filter finds, by part; null while the server is asked. */
	const filtered = $derived.by((): { renamed: RenamedRow[]; kinds: { section: PlanPageSection; title: string; rows: StepRow[] }[]; more: boolean } | null => {
		if (!filtering) return null;
		if (!serverFilter) {
			return {
				renamed: renamed.filter((r) => matches(`${r.to} ${r.from ?? ''}`)),
				kinds: kinds.map((k) => ({ section: k.section, title: k.title, rows: k.rows.filter((r) => matches(r.node)) })).filter((k) => k.rows.length),
				more: false
			};
		}
		if (!current(found, wantedName) || !found) return null;
		const page = (s: PlanPageSection) => found?.pages[s] ?? { rows: [], total: 0 };
		return {
			renamed: (page('renamed').rows as PlanModel[]).map((m) => renamedRow(m)),
			kinds: STEP_KINDS.map((k) => ({ section: k.section, title: k.title, rows: stepRowsOf(k.section, page(k.section).rows) })).filter((k) => k.rows.length),
			more: Object.values(found.pages).some((p) => p && p.total > p.rows.length)
		};
	});

	const href = (model: string) => to(`/catalog/model:${model}`);
	/** The distinct warnings of the rows shown, each said once under the heading, with how many rows carry it. */
	function warningsOf(rows: { warning: string | null }[]): [string, number][] {
		const seen = new Map<string, number>();
		for (const r of rows) if (r.warning) seen.set(r.warning, (seen.get(r.warning) ?? 0) + 1);
		return [...seen];
	}
</script>

{#snippet showMore_(s: PlanPageSection, shown: number)}
	{@const rest = total(s) - shown}
	{#if rest > 0}
		<div class="more"><Button variant="link" size="inline" class="text-[12px]" disabled={busy} onclick={() => showMore(s)}>{busy ? 'Loading…' : `Show more · ${rest.toLocaleString()} not shown`}</Button></div>
	{/if}
{/snippet}

{#snippet renamedRows(rows: RenamedRow[])}
	<div class="rows">
		{#each rows as r (r.to)}
			<div class="row">
				<span class="name">{#if r.from}<span class="from">{r.from}</span><span class="arrow">→</span>{/if}<a href={href(r.to)}>{r.to}</a></span>
				<span class="detail">{r.detail.join(' · ')}</span>
			</div>
		{/each}
	</div>
{/snippet}

{#snippet stepList(rows: StepRow[])}
	{#each warningsOf(rows) as [text, n] (text)}<p class="warn">⚠ {text}{n < rows.length ? ` (${n} of the ${rows.length} shown)` : ''}</p>{/each}
	<div class="rows">
		{#each rows as r, i (i)}
			<div class="row">
				<span class="name">{#if r.node.includes(' ') || r.node.includes(',')}{r.node}{:else}<a href={href(r.node)}>{r.node}</a>{/if}</span>
				<span class="detail"><span class="what">{r.what}</span>{#if r.detail.length}<span class="sep">·</span>{r.detail.join(' · ')}{/if}{#if r.warning && warningsOf(rows).length > 1}<span class="mark" title={r.warning}>⚠</span>{/if}</span>
			</div>
		{/each}
	</div>
{/snippet}

<div class="renames" data-testid="plan-renames">
	<header>
		<h3>Renames and migrations <span class="count">{[renamedTotal ? plural(renamedTotal, 'node') : null, steps ? plural(steps, 'migration step') : null].filter(Boolean).join(' · ')}</span></h3>
	</header>
	<label class="filter">
		<SearchIcon size={13} />
		<input bind:value={filter} placeholder="Filter renames and migrations" aria-label="Filter renames and migrations" />
		{#if filter}<Button variant="ghost" size="icon-xs" onclick={() => (filter = '')} aria-label="Clear the filter"><XIcon /></Button>{/if}
	</label>

	{#if filtering}
		{#if !filtered}
			<p class="hint">Finding renames and migrations…</p>
		{:else if !filtered.renamed.length && !filtered.kinds.length}
			<p class="hint">No renames or migrations match “{filter.trim()}”.</p>
		{:else}
			{#if filtered.renamed.length}
				<section class="grid" data-part="renamed">
					<h4>Renamed <span class="count">{filtered.renamed.length.toLocaleString()}</span></h4>
					{@render renamedRows(filtered.renamed)}
				</section>
			{/if}
			{#if filtered.kinds.length}
				<section class="grid" data-part="steps">
					<h4>Migration steps</h4>
					{#each filtered.kinds as k (k.section)}
						<div class="kind" data-kind={k.section}>
							<h5>{k.title} <span class="count">{k.rows.length.toLocaleString()}</span></h5>
							{@render stepList(k.rows)}
						</div>
					{/each}
				</section>
			{/if}
			{#if filtered.more}<p class="hint">The first {FILTER_READ} matches of each part; narrow the filter for the rest.</p>{/if}
		{/if}
	{:else}
		{#if renamedTotal}
			{@const shown = renamed.slice(0, wanted('renamed'))}
			<section class="grid" data-part="renamed">
				<h4>Renamed <span class="count">{renamedTotal.toLocaleString()}</span></h4>
				{@render renamedRows(shown)}
				{@render showMore_('renamed', shown.length)}
			</section>
		{/if}

		{#if steps}
			<section class="grid" data-part="steps">
				<h4>Migration steps <span class="count">{steps.toLocaleString()}</span></h4>
				{#each kinds as k (k.section)}
					{@const shown = k.rows.slice(0, wanted(k.section))}
					<div class="kind" data-kind={k.section}>
						<h5>{k.title} <span class="count">{k.total.toLocaleString()}</span></h5>
						{@render stepList(shown)}
						{@render showMore_(k.section, shown.length)}
					</div>
				{/each}
			</section>
		{/if}
	{/if}
	{#if filtering ? filterError : pageError}
		<p class="hint err" data-testid="renames-error">Couldn't load rows: {filtering ? filterError : pageError} <Button variant="link" size="inline" class="text-[12px]" onclick={() => (filtering ? (filterError = null) : (pageError = null))}>Retry</Button></p>
	{/if}
</div>

<style>
	/* Rows as the Changed list draws them: the name in mono, what happens muted beside it. */
	.renames {
		display: flex;
		min-width: 0;
		flex-direction: column;
		font-size: 12.5px;
	}
	header {
		margin-bottom: 10px;
	}
	/* The filter as the other plan sections draw theirs. */
	.filter {
		display: flex;
		max-width: 320px;
		height: 28px;
		align-items: center;
		gap: 7px;
		margin-bottom: 8px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--sb-text-faint);
	}
	.filter:focus-within {
		border-color: color-mix(in srgb, var(--primary) 55%, var(--border));
	}
	.filter input {
		min-width: 0;
		flex: 1;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
	}
	h3 {
		margin: 0;
		font-size: 14px;
		font-weight: 600;
	}
	h4 {
		margin: 4px 0 4px;
		color: var(--muted-foreground);
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
	section + section {
		margin-top: 22px;
	}
	h5 {
		margin: 12px 0 4px;
		font-size: 12.5px;
		font-weight: 600;
	}
	.count {
		color: var(--sb-text-faint);
		font-weight: 400;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0;
		text-transform: none;
	}
	/* One grid for a part's rows, every kind of step included, so the second column starts just after
	   the longest name and lines up all the way down; headings span it. */
	.grid {
		display: grid;
		grid-template-columns: minmax(0, max-content) minmax(0, 1fr);
	}
	.grid > *,
	.kind > * {
		grid-column: 1 / -1;
	}
	.kind,
	.rows,
	.row {
		display: contents;
	}
	.row > span {
		grid-column: auto;
		padding: 4px 0;
		border-bottom: 1px solid var(--border-subtle);
	}
	.row > .name {
		padding-right: 24px;
	}
	.name {
		display: flex;
		min-width: 0;
		gap: 8px;
		overflow: hidden;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.name a {
		overflow: hidden;
		color: var(--foreground);
		text-overflow: ellipsis;
	}
	.name a:hover {
		color: var(--primary);
	}
	.from {
		overflow: hidden;
		color: var(--muted-foreground);
		text-overflow: ellipsis;
	}
	.arrow {
		color: var(--sb-text-faint);
	}
	.detail {
		min-width: 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.what {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.sep {
		margin: 0 6px;
		color: var(--sb-text-faint);
	}
	.mark {
		margin-left: 6px;
		color: var(--sb-warning);
	}
	.warn {
		margin: 0 0 4px;
		color: color-mix(in srgb, var(--sb-warning) 80%, var(--foreground));
		font-size: 12px;
	}
	.more {
		margin-top: 6px;
	}
	.hint {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.err {
		color: var(--sb-error);
	}
</style>
