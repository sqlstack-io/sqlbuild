<script lang="ts">
	// The plan as an index and a pane: the plan's groups listed on the left with their counts, one shown
	// on the right at a time, its rows a page at a time. Large sections' rows load from the server's
	// plan as they are shown; none loads until its group is opened.
	import Button from '$lib/components/ui/button/button.svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import { to } from '$lib/paths';
	import type { PlanDocument, PlanPageSection } from '$lib/api';
	import { warningText } from './plan-text';
	import { AFTER_DIVIDER, diffPreview, planGroups, planSummaryLine, selectedGroup, type PlanGroupKey, type PlanRow } from './plan-index';

	/**
	 * `section` is the group shown, kept in the address by the page; `more` loads the next page of a
	 * large section, of `limit` rows; `refresh` words what refreshes a held-back job's nodes.
	 */
	let {
		plan,
		section = null,
		onselect,
		more,
		refresh = (job: string) => `job ${job}`
	}: {
		plan: PlanDocument;
		section?: string | null;
		onselect: (key: PlanGroupKey) => void;
		more?: (section: PlanPageSection, limit: number) => Promise<void>;
		refresh?: (job: string) => string;
	} = $props();

	/** Rows a group shows at first, and each "Show more" adds. */
	const PAGE = 50;
	const groups = $derived(planGroups(plan, refresh));
	const group = $derived(selectedGroup(groups, section));
	const warningGroups = $derived(group?.warningGroups ?? []);

	/** Per group: rows shown, the filter, and which rows' diffs and warning groups are open. */
	let shownOf = $state<Record<string, number>>({});
	let filterOf = $state<Record<string, string>>({});
	let openDiff = $state<Record<string, 'preview' | 'full'>>({});
	let openWarnings = $state<Record<string, boolean>>({});
	let loading = $state(false);
	let loadError = $state<string | null>(null);

	const shown = $derived(group ? (shownOf[group.key] ?? PAGE) : PAGE);
	const filter = $derived(group ? (filterOf[group.key] ?? '') : '');
	const filtered = $derived(group ? (filter ? group.rows.filter((r) => r.name.toLowerCase().includes(filter.toLowerCase())) : group.rows) : []);
	const visible = $derived(filtered.slice(0, shown));
	/** The next server page the group needs to show `shown` rows: its first section with rows still to load. */
	const nextPage = $derived(group?.pages.find((p) => p.loaded < p.total) ?? null);
	const allLoaded = $derived(!group || group.rows.length >= group.count);

	// Load pages until the group holds the rows it shows, one at a time; a filter waits for Show more.
	$effect(() => {
		const g = group;
		const want = shown;
		const page = nextPage;
		if (!g || !page || !more || loading || filter || g.rows.length >= Math.min(want, g.count)) return;
		loading = true;
		loadError = null;
		more(page.section, PAGE)
			.catch((e: unknown) => (loadError = e instanceof Error ? e.message : String(e)))
			.finally(() => (loading = false));
	});

	function showMore() {
		if (!group) return;
		shownOf[group.key] = shown + PAGE;
	}
	const diffKey = (r: PlanRow) => `${group?.key}:${r.name}`;
	function toggleDiff(r: PlanRow) {
		const key = diffKey(r);
		if (openDiff[key]) delete openDiff[key];
		else openDiff[key] = 'preview';
	}
	/** The index's divider goes before the first quiet group, unless nothing comes before it. */
	const dividerBefore = $derived(groups.findIndex((g) => g.key === AFTER_DIVIDER || g.key === 'unchanged' || g.key === 'skipped'));
	const href = (r: PlanRow) => (r.linked && !/^\d+$/.test(r.name) ? to(`/catalog/model:${r.name}`) : null);
	const remaining = $derived(group ? (filter ? filtered.length - visible.length : group.count - visible.length) : 0);
</script>

<div class="plan-browser">
	<nav class="index" aria-label="Plan sections">
		<p class="summary">{planSummaryLine(groups)}</p>
		{#each groups as g, i (g.key)}
			{#if i === dividerBefore && i > 0}<hr />{/if}
			<!-- raw-button: a table-of-contents entry with its count, highlighted while its section is shown -->
			<button class="entry" class:selected={g.key === group?.key} aria-current={g.key === group?.key ? 'true' : undefined} data-section={g.key} onclick={() => onselect(g.key)}>
				<span class="title">{g.title}</span>
				<span class="count">{g.count.toLocaleString()}</span>
			</button>
		{/each}
	</nav>

	{#if group}
		<section class="pane" data-section={group.key}>
			<header>
				<h3>{group.title} <span class="count">{group.count.toLocaleString()}</span></h3>
				{#if group.subtitle}<p class="subtitle">{group.subtitle}</p>{/if}
			</header>

			{#if group.key === 'warnings'}
				{#each warningGroups as w (w.template)}
					<div class="warning-group">
						<div class="row">
							<span class="template">{w.template}</span>
							<span class="detail">{w.label}</span>
							<Button variant="link" size="inline" class="text-[11.5px]" onclick={() => (openWarnings[w.template] = !openWarnings[w.template])}>{openWarnings[w.template] ? 'hide' : 'show'}</Button>
						</div>
						{#if openWarnings[w.template]}
							<ul class="warning-list">
								{#each w.warnings as one, i (i)}<li class:err={one.severity === 'error'}>{#if one.code}<code>{one.code}</code> {/if}{one.message}</li>{/each}
							</ul>
						{/if}
					</div>
				{/each}
			{:else}
				<label class="filter">
					<SearchIcon size={13} />
					<input
						value={filter}
						oninput={(e) => (filterOf[group.key] = e.currentTarget.value)}
						placeholder="Filter {group.title.toLowerCase()}"
						aria-label="Filter {group.title.toLowerCase()}"
					/>
					{#if filter}<Button variant="ghost" size="icon-xs" onclick={() => (filterOf[group.key] = '')} aria-label="Clear the filter"><XIcon /></Button>{/if}
				</label>
				{#if filter && !allLoaded}<p class="hint">Filtering the {group.rows.length.toLocaleString()} rows loaded of {group.count.toLocaleString()}.</p>{/if}

				<div class="rows">
					{#each visible as r (r.name + r.action)}
						{@const link = href(r)}
						{@const open = openDiff[diffKey(r)]}
						<div class="plan-row">
							<div class="row">
								{#if link}<a class="name" href={link} title={r.name}>{r.name}</a>{:else}<span class="name" title={r.name}>{r.name}</span>{/if}
								<span class="action">{r.action}</span>
								<span class="detail">{r.detail}{#if r.diff}{r.detail ? ' · ' : ''}<Button variant="link" size="inline" class="text-[11.5px]" onclick={() => toggleDiff(r)}>{open ? 'hide diff' : 'diff'}</Button>{/if}</span>
							</div>
							{#each r.notes as note, i (i)}<div class="note">{note}</div>{/each}
							{#each r.warnings as w, i (i)}<p class="row-warning" class:err={w.severity === 'error'}>{warningText(w)}</p>{/each}
							{#if open && r.diff}
								{@const preview = open === 'full' ? { lines: r.diff, cut: false } : diffPreview(r.diff)}
								<pre class="diff">{#each preview.lines as l, i (i)}<span class={l.cls}>{l.text}</span>{'\n'}{/each}</pre>
								{#if preview.cut}<Button variant="link" size="inline" class="diff-more text-[11.5px]" onclick={() => (openDiff[diffKey(r)] = 'full')}>open the full diff</Button>{/if}
							{/if}
						</div>
					{/each}
					{#if filter && !filtered.length}<p class="hint">No {group.title.toLowerCase()} match “{filter}”.</p>{/if}
				</div>
				{#if loadError}<p class="hint err">Couldn't load more rows: {loadError}</p>{/if}
				{#if remaining > 0}
					<Button variant="link" size="inline" class="show-more mt-2 self-start text-[12px]" disabled={loading} onclick={showMore}>
						{loading ? 'Loading…' : `Show more · ${remaining.toLocaleString()} not shown`}
					</Button>
				{:else if loading}<p class="hint">Loading…</p>{/if}
			{/if}
		</section>
	{/if}
</div>

<style>
	.plan-browser {
		display: grid;
		grid-template-columns: 230px minmax(0, 1fr);
		gap: 24px;
		align-items: start;
		font-size: 12.5px;
	}
	.index {
		position: sticky;
		top: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.summary {
		margin: 0 0 10px;
		color: var(--muted-foreground);
		font-size: 12px;
		line-height: 1.45;
	}
	.index hr {
		margin: 6px 0;
		border: 0;
		border-top: 1px solid var(--border);
	}
	.entry {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 8px;
		padding: 5px 8px;
		border-left: 2px solid transparent;
		border-radius: 2px;
		color: var(--muted-foreground);
		cursor: pointer;
		text-align: left;
	}
	.entry:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.entry.selected {
		border-left-color: var(--primary);
		background: color-mix(in srgb, var(--primary) 12%, transparent);
		color: var(--foreground);
		font-weight: 500;
	}
	.count {
		color: var(--sb-text-faint);
		font-variant-numeric: tabular-nums;
		font-weight: 400;
	}
	.pane {
		display: flex;
		min-width: 0;
		flex-direction: column;
	}
	h3 {
		margin: 0;
		font-size: 14px;
		font-weight: 600;
	}
	.subtitle {
		margin: 2px 0 0;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	header {
		margin-bottom: 10px;
	}
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
	.plan-row,
	.warning-group {
		padding: 4px 0;
		border-bottom: 1px solid var(--border-subtle);
	}
	.row {
		display: grid;
		grid-template-columns: minmax(0, 2.2fr) minmax(0, 1fr) minmax(0, 2.4fr);
		gap: 12px;
		align-items: baseline;
	}
	.warning-group .row {
		grid-template-columns: minmax(0, 1fr) auto auto;
	}
	.name {
		overflow: hidden;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	a.name:hover {
		color: var(--primary);
	}
	.action {
		overflow: hidden;
		color: var(--muted-foreground);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.detail {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.note {
		padding-left: 16px;
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 17px;
	}
	.template {
		color: var(--foreground);
	}
	.warning-list {
		margin: 4px 0 2px;
		padding-left: 16px;
		color: var(--muted-foreground);
		font-size: 12px;
		line-height: 1.5;
	}
	.warning-list li.err,
	.row-warning.err {
		border-left-color: var(--sb-error);
	}
	.row-warning {
		margin: 4px 0 2px 16px;
		padding: 4px 8px;
		border-left: 2px solid var(--sb-warning);
		background: color-mix(in srgb, var(--sb-warning) 8%, transparent);
		font-size: 12px;
	}
	.diff {
		margin: 4px 0 2px 16px;
		padding: 6px 10px;
		overflow-x: auto;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 16px;
	}
	.diff .add {
		color: var(--sb-success);
	}
	.diff .del {
		color: var(--sb-error);
	}
	.diff .hunk {
		color: var(--sb-text-faint);
	}
	.plan-row :global(.diff-more) {
		margin-left: 16px;
	}
	.hint {
		margin: 6px 0;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.hint.err {
		color: var(--sb-error);
	}
	code {
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 11px;
	}
</style>
