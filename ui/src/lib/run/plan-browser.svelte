<script lang="ts">
	// The plan as an index and a pane: the plan's groups listed on the left with their counts, one shown
	// on the right at a time, its rows a page at a time. Large sections' rows load from the server's
	// plan as they are shown; none loads until its group is opened.
	import Button from '$lib/components/ui/button/button.svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import { to } from '$lib/paths';
	import { mergePlanPage, type PlanDiff, type PlanDocument, type PlanPage, type PlanPageSection, type PlanWarning } from '$lib/api';
	import { warningText } from './plan-text';
	import { plural } from '$lib/format/plural';
	import { planSectionCounts, planTotals } from './plan-totals';
	import { migrationSteps } from './plan-steps';
	import { WARNINGS_SHOWN, diffPreview, planGroups, selectedGroup, withoutRows, type PlanGroupKey, type PlanRow, type WarningGroup } from './plan-index';
	import PlanRenames from './plan-renames.svelte';
	import PlanCantRun from './plan-cant-run.svelte';
	import { cantRunKeys, sinkBlocks, willRunCount } from './cant-run';

	/**
	 * `section` is the group shown, kept in the address by the page; `more` adds the next page of a
	 * section to `plan`, of `limit` rows; `load` reads a page of rows whose name contains `name`, for the
	 * filter; `diff` reads a model's diff, null when the server has none to give; `refresh` words what
	 * refreshes a held-back job's nodes.
	 * The index groups the sections under Will run and Won't run, each heading the nodes counted once
	 * (plan-totals.ts), with warnings apart.
	 */
	let {
		plan,
		section = null,
		onselect,
		more,
		load,
		diff,
		refresh = (job: string) => `job ${job}`,
		kindOf,
		upstreamOf
	}: {
		plan: PlanDocument;
		section?: string | null;
		onselect: (key: PlanGroupKey | typeof CANT_RUN) => void;
		more?: (section: PlanPageSection, limit: number) => Promise<void>;
		load?: (section: PlanPageSection, offset: number, limit: number, name: string) => Promise<PlanPage>;
		diff?: (model: string) => Promise<PlanDiff | null>;
		refresh?: (job: string) => string;
		/** A node's kind and its upstream keys, from the catalog, for the Can't run pane. */
		kindOf?: (key: string) => string;
		upstreamOf?: (key: string) => string[];
	} = $props();

	/** The pane of nodes a provider's missing secrets stop; shown first when the plan has any. */
	const CANT_RUN = 'cant_run';
	const cantRun = $derived(plan.cant_run ?? []);
	const blocked = $derived(cantRunKeys(cantRun));
	/** How many nodes can't run: all of them when a sink's provider is missing a secret. */
	const cantCount = $derived(sinkBlocks(cantRun) ? 'all' : blocked.length.toLocaleString());
	const cantRunShown = $derived(cantRun.length > 0 && (section === null || section === CANT_RUN));

	/** Rows a group shows at first, and each "Show more" adds. */
	const PAGE = 50;
	/** How long the filter waits for typing to stop before asking the server. */
	const FILTER_WAIT_MS = 250;
	/** The most warnings one read brings, as the server pages them. */
	const WARNINGS_READ = 1000;

	/** Diffs read on demand, by model; null when the server has none to give. */
	let diffs = $state.raw(new Map<string, PlanDiff | null>());
	const reading = new Set<string>();
	function readDiff(model: string) {
		if (!diff || diffs.has(model) || reading.has(model)) return;
		reading.add(model);
		diff(model)
			.catch(() => null)
			.then((d) => (diffs = new Map(diffs).set(model, d)))
			.finally(() => reading.delete(model));
	}

	const groups = $derived(planGroups(plan, refresh, diffs));
	const group = $derived(selectedGroup(groups, section));
	/** The Renames and migrations pane lists renamed models and migration steps apart, and loads its own sections. */
	const renamesPane = $derived(group?.key === 'renames');
	const warningGroups = $derived(group?.warningGroups ?? []);
	/** A server summary's groups come counted; each one's warnings are read when it opens, by where they start. */
	let readWarnings = $state<Record<number, PlanWarning[]>>({});
	/** Which warning groups are open, and which show all their warnings, by where their warnings start. */
	let openWarnings = $state<Record<number, boolean>>({});
	let allWarnings = $state<Record<number, boolean>>({});
	/** The groups being read, by where their warnings start, one read each, and the summary groups they were read for. */
	const readingWarnings = new Set<number>();
	let readFor: unknown = null;
	// Another plan's groups start closed and unread; a page of this plan's rows keeps the same groups.
	$effect.pre(() => {
		if (plan.warning_groups === readFor) return;
		readFor = plan.warning_groups;
		readWarnings = {};
		readingWarnings.clear();
		openWarnings = {};
		allWarnings = {};
	});
	function readGroup(w: WarningGroup, limit: number) {
		const want = Math.min(limit, w.count);
		const have = readWarnings[w.offset]?.length ?? 0;
		if (w.warnings.length || !load || have >= want || readingWarnings.has(w.offset)) return;
		const read = readWarnings;
		readingWarnings.add(w.offset);
		load('warnings', w.offset + have, want - have, '')
			.then((page) => {
				// A read another plan replaced lands nowhere.
				if (read !== readWarnings) return;
				readingWarnings.delete(w.offset);
				readWarnings[w.offset] = [...(readWarnings[w.offset] ?? []), ...(page.rows as PlanWarning[])];
				// "show all" asked while the first rows were read.
				if (page.rows.length) readGroup(w, allWarnings[w.offset] ? WARNINGS_READ : limit);
			})
			.catch((e: unknown) => {
				if (read === readWarnings) readingWarnings.delete(w.offset);
				if (read === readWarnings) loadErrors.warnings = e instanceof Error ? e.message : String(e);
			});
	}
	function toggleWarnings(w: WarningGroup) {
		openWarnings[w.offset] = !openWarnings[w.offset];
		if (openWarnings[w.offset]) readGroup(w, WARNINGS_SHOWN);
	}
	function showAllWarnings(w: WarningGroup) {
		allWarnings[w.offset] = true;
		readGroup(w, WARNINGS_READ);
	}
	const warningsOf = (w: WarningGroup) => (w.warnings.length ? w.warnings : (readWarnings[w.offset] ?? []));

	/** Per group: rows shown, the filter typed and the one asked for, and which diffs are open. */
	let shownOf = $state<Record<string, number>>({});
	let filterOf = $state<Record<string, string>>({});
	let askedOf = $state<Record<string, string>>({});
	let openDiff = $state<Record<string, 'preview' | 'full'>>({});
	let loading = $state(false);
	/** Failed reads, by group: shown there with Retry, and that group reads nothing more until then. */
	let loadErrors = $state<Record<string, string>>({});
	const failed = (e: unknown) => (e instanceof Error ? e.message : String(e));
	// Another plan (a re-plan) starts with no failed reads: its rows are read from scratch.
	let erroredFor: unknown = null;
	$effect.pre(() => {
		if (plan.sections === erroredFor) return;
		erroredFor = plan.sections;
		loadErrors = {};
	});
	/** Reads a group's rows again after a failure; a filter's view starts over. */
	function retry(key: string) {
		delete loadErrors[key];
		if (view?.key === key) view = null;
		if (key === 'warnings') for (const w of warningGroups) if (openWarnings[w.offset]) readGroup(w, allWarnings[w.offset] ? WARNINGS_READ : WARNINGS_SHOWN);
	}

	const shown = $derived(group ? (shownOf[group.key] ?? PAGE) : PAGE);
	const filter = $derived(group ? (filterOf[group.key] ?? '') : '');
	/** The server finds rows by name, a page at a time; the default mock's whole plan, with every row, is filtered here. */
	const serverFilter = $derived(!!plan.sections && !!load);
	const asked = $derived(group ? (askedOf[group.key] ?? '') : '');

	// The filter asks the server once typing stops.
	$effect(() => {
		const key = group?.key;
		const text = filter.trim();
		if (!key || !serverFilter || text === (askedOf[key] ?? '')) return;
		const timer = setTimeout(() => {
			askedOf[key] = text;
			shownOf[key] = PAGE;
		}, FILTER_WAIT_MS);
		return () => clearTimeout(timer);
	});

	/**
	 * A filtered view of a group: the plan's facts with only the rows found by name, each section's size
	 * the number of matches (unknown until its first page is read).
	 */
	type View = { key: string; name: string; doc: PlanDocument; matches: Partial<Record<PlanPageSection, number>>; sections: PlanDocument['sections'] };
	let view = $state.raw<View | null>(null);
	/** Only a server summary is filtered by the server, so a view's document is always a summary. */
	const viewDoc = (matches: View['matches'], doc: PlanDocument): PlanDocument =>
		doc.sections ? { ...doc, sections: (plan.sections ?? []).map((c) => ({ section: c.section, total: matches[c.section] ?? 0, notes: [] })) } : doc;

	// A new filter starts its view: the first page of each of the group's sections, for how many each matches.
	$effect(() => {
		const g = group;
		const name = asked;
		if (!g || !serverFilter || !load || !name) return;
		if (view && view.key === g.key && view.name === name && view.sections === plan.sections) return;
		const start: View = { key: g.key, name, doc: viewDoc({}, withoutRows(plan)), matches: {}, sections: plan.sections };
		view = start;
		loading = true;
		delete loadErrors[g.key];
		(async () => {
			let current = start;
			for (const s of g.sections) {
				const page = await load(s, 0, PAGE, name);
				if (view !== current) return;
				const matches = { ...current.matches, [s]: page.total };
				current = { ...current, matches, doc: viewDoc(matches, mergePlanPage(current.doc, s, page)) };
				view = current;
			}
		})()
			.catch((e: unknown) => (loadErrors[g.key] = failed(e)))
			.finally(() => (loading = false));
	});

	const filtering = $derived(!!filter.trim());
	/** The group as filtered: by the server's view of it, or here for a whole plan. */
	const shownGroup = $derived.by(() => {
		if (!group || !filtering) return group;
		if (!serverFilter) return { ...group, rows: group.rows.filter((r) => r.name.toLowerCase().includes(filter.trim().toLowerCase())) };
		if (!view || view.key !== group.key || view.name !== asked || asked !== filter.trim()) return null;
		const found = planGroups(view.doc, refresh, diffs).find((g) => g.key === group.key);
		return found ?? { ...group, count: 0, rows: [], pages: [] };
	});
	const rowCount = $derived(shownGroup ? (filtering && !serverFilter ? shownGroup.rows.length : shownGroup.count) : 0);
	const visible = $derived(shownGroup?.rows.slice(0, shown) ?? []);
	/** The next server page the group needs to show `shown` rows: its first section with rows still to load. */
	const nextPage = $derived(shownGroup?.pages.find((p) => p.loaded < p.total) ?? null);

	// Load pages until the group holds the rows it shows, one at a time; warnings are read by group.
	$effect(() => {
		const g = shownGroup;
		const page = nextPage;
		// After a failed read nothing more is read until Retry, so a lasting failure doesn't repeat its request.
		if (!g || !page || loading || loadErrors[g.key] || renamesPane || g.key === 'warnings' || g.rows.length >= Math.min(shown, g.count)) return;
		if (filtering && !serverFilter) return;
		const current = view;
		const step = filtering
			? load && current
				? load(page.section, page.loaded, PAGE, current.name).then((rows) => {
						if (view !== current) return;
						view = { ...current, doc: viewDoc(current.matches, mergePlanPage(current.doc, page.section, rows)) };
					})
				: null
			: (more?.(page.section, PAGE) ?? null);
		if (!step) return;
		loading = true;
		step.catch((e: unknown) => (loadErrors[g.key] = failed(e))).finally(() => (loading = false));
	});

	function showMore() {
		if (!group) return;
		shownOf[group.key] = shown + PAGE;
	}
	const diffKey = (r: PlanRow) => `${group?.key}:${r.name}`;
	function toggleDiff(r: PlanRow) {
		const key = diffKey(r);
		if (openDiff[key]) delete openDiff[key];
		else {
			openDiff[key] = 'preview';
			if (r.fetch === 'diff') readDiff(r.name);
		}
	}
	const href = (r: PlanRow) => (r.linked && !/^\d+$/.test(r.name) ? to(`/catalog/model:${r.name}`) : null);
	const remaining = $derived(rowCount - visible.length);

	// The index under Will run and Won't run, each entry counting nodes once (plan-totals.ts), warnings apart.
	const counts = $derived(planSectionCounts(plan));
	const totals = $derived(planTotals(counts, migrationSteps(plan)));
	const WONT_GROUPS = new Set<PlanGroupKey>(['held_back', 'skipped']);
	const FUNCTION_SECTIONS = new Set<PlanPageSection>(['functions', 'changed_functions']);
	const runGroups = $derived(groups.filter((g) => !WONT_GROUPS.has(g.key) && g.key !== 'warnings'));
	const wontGroups = $derived(groups.filter((g) => WONT_GROUPS.has(g.key)));
	const warningGroup = $derived(groups.find((g) => g.key === 'warnings'));
	/** A group's nodes, as its heading counts them; warnings count themselves. */
	const nodesOf = (g: (typeof groups)[number]) => [...totals.runParts, ...totals.wontParts].filter((p) => p.group === g.key).reduce((n, p) => n + p.count, 0);
	/** A group's functions, which are listed but are not nodes the build runs. */
	const functionsOf = (g: (typeof groups)[number]) => counts.filter((c) => FUNCTION_SECTIONS.has(c.section) && g.sections.includes(c.section)).reduce((n, c) => n + c.total, 0);
	/**
	 * What an index entry counts: its nodes, or when it has none, muted, what it lists instead: the
	 * Renames group's migration steps, or a group's functions.
	 */
	function entryCount(g: (typeof groups)[number]): { text: string; muted: boolean } {
		if (g.key === 'warnings') return { text: g.count.toLocaleString(), muted: false };
		const nodes = nodesOf(g);
		if (nodes) return { text: nodes.toLocaleString(), muted: false };
		if (g.key === 'renames' && totals.steps) return { text: plural(totals.steps, 'step'), muted: true };
		const functions = functionsOf(g);
		return { text: functions ? plural(functions, 'function') : '', muted: true };
	}
	/** A pane's count: its nodes, and its functions after them, muted. */
	const paneCount = (g: (typeof groups)[number]) => ({ nodes: nodesOf(g), functions: functionsOf(g) });
</script>

{#snippet entry(g: (typeof groups)[number], title = g.title)}
	{@const selected = g.key === group?.key}
	{@const count = entryCount(g)}
	<!-- raw-button: a table-of-contents entry with its count, highlighted while its section is shown -->
	<button class="entry" class:selected aria-current={selected ? 'true' : undefined} data-section={g.key} onclick={() => onselect(g.key)}>
		<span class="title">{title}</span>
		<span class="count" class:muted={count.muted}>{count.text}</span>
	</button>
{/snippet}

<div class="plan-browser">
	<nav class="index" aria-label="Plan sections">
		{#if runGroups.length}
			<div class="heading" data-testid="index-will-run"><span>Will run</span><b>{willRunCount(totals.run, cantRun).toLocaleString()}</b></div>
			{#each runGroups as g (g.key)}{@render entry(g)}{/each}
		{/if}
		{#if wontGroups.length}
			<div class="heading wont" data-testid="index-wont-run"><span>Won’t run</span><b>{totals.wont.toLocaleString()}</b></div>
			{#each wontGroups as g (g.key)}{@render entry(g)}{/each}
		{/if}
		{#if cantRun.length}
			<div class="heading cant" data-testid="index-cant-run"><span>Can’t run</span><b>{cantCount}</b></div>
			<!-- raw-button: a table-of-contents entry with its count, highlighted while its section is shown -->
			<button class="entry" class:selected={cantRunShown} aria-current={cantRunShown ? 'true' : undefined} data-section={CANT_RUN} onclick={() => onselect(CANT_RUN)}>
				<span class="title">Missing secret</span>
				<span class="count">{cantCount}</span>
			</button>
		{/if}
		{#if warningGroup}
			{#if runGroups.length || wontGroups.length}<hr />{/if}
			{@render entry(warningGroup)}
		{/if}
	</nav>

	{#if cantRunShown}
		<section class="pane" data-section={CANT_RUN}>
			<header><h3>Can’t run <span class="count">{sinkBlocks(cantRun) ? 'every node' : blocked.length === 1 ? '1 node' : `${blocked.length.toLocaleString()} nodes`}</span></h3></header>
			<PlanCantRun entries={cantRun} {kindOf} {upstreamOf} />
		</section>
	{:else if group}
		<section class="pane" data-section={group.key}>
			{#if renamesPane}
				<PlanRenames {plan} {counts} {more} {load} />
			{:else}
			{@const c = group.key === 'warnings' ? null : paneCount(group)}
			<header>
				<h3>{group.title} <span class="count">{#if c}{#if c.nodes || !c.functions}{plural(c.nodes, 'node')}{/if}{#if c.functions}<span class="muted">{c.nodes ? ' · ' : ''}{plural(c.functions, 'function')}</span>{/if}{:else}{group.count.toLocaleString()}{/if}</span></h3>
				{#if group.subtitle}<p class="subtitle">{group.subtitle}</p>{/if}
			</header>

			{#if group.key === 'warnings'}
				{#each warningGroups as w (w.offset)}
					<div class="warning-group">
						<div class="row">
							<span class="template">{w.template}</span>
							<span class="detail">{w.label}</span>
							<Button variant="link" size="inline" class="text-[11.5px]" onclick={() => toggleWarnings(w)}>{openWarnings[w.offset] ? 'hide' : 'show'}</Button>
						</div>
						{#if openWarnings[w.offset]}
							{@const all = allWarnings[w.offset]}
							{@const listed = warningsOf(w)}
							<ul class="warning-list">
								{#each all ? listed : listed.slice(0, WARNINGS_SHOWN) as one, i (i)}<li class:err={one.severity === 'error'}>{#if one.code}<code>{one.code}</code> {/if}{one.message}</li>{/each}
							</ul>
							{#if all && listed.length >= WARNINGS_READ && listed.length < w.count}
								<p class="hint">The first {listed.length.toLocaleString()} of {w.count.toLocaleString()}.</p>
							{:else if !all && w.count > WARNINGS_SHOWN}
								<Button variant="link" size="inline" class="ml-4 text-[11.5px]" onclick={() => showAllWarnings(w)}>show all {w.count.toLocaleString()}</Button>
							{/if}
						{/if}
					</div>
				{/each}
				{#if loadErrors.warnings}<p class="hint err" data-testid="plan-rows-error">Couldn't read the warnings: {loadErrors.warnings} <Button variant="link" size="inline" class="text-[12px]" onclick={() => retry('warnings')}>Retry</Button></p>{/if}
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

				<div class="rows">
					{#each visible as r (r.name + r.action)}
						{@const link = href(r)}
						{@const open = openDiff[diffKey(r)]}
						<div class="plan-row">
							<div class="row">
								{#if link}<a class="name" href={link} title={r.name}>{r.name}</a>{:else}<span class="name" title={r.name}>{r.name}</span>{/if}
								<span class="action">{r.action}</span>
								<span class="detail">{r.detail}{#if r.diff || r.fetch}{r.detail ? ' · ' : ''}<Button variant="link" size="inline" class="text-[11.5px]" onclick={() => toggleDiff(r)}>{open ? 'hide diff' : 'diff'}</Button>{/if}</span>
							</div>
							{#each r.notes as note, i (i)}<div class="note">{note}</div>{/each}
							{#each r.warnings as w, i (i)}<p class="row-warning" class:err={w.severity === 'error'}>{warningText(w)}</p>{/each}
							{#if open && r.diff}
								{@const preview = open === 'full' ? { lines: r.diff, cut: false, more: 0 } : diffPreview(r.diff)}
								<pre class="diff">{#each preview.lines as l, i (i)}<span class={l.cls}>{l.text}</span>{'\n'}{/each}</pre>
								{#if preview.cut}<Button variant="link" size="inline" class="diff-more text-[11.5px]" onclick={() => (openDiff[diffKey(r)] = 'full')}>open the full diff ({preview.more.toLocaleString()} more lines)</Button>{/if}
							{:else if open && r.fetch === 'diff'}
								<p class="diff-note">Reading the diff…</p>
							{:else if open && r.fetch === 'unavailable'}
								<p class="diff-note">diff not available</p>
							{/if}
						</div>
					{/each}
					{#if filtering && !shownGroup}<p class="hint">Finding {group.title.toLowerCase()}…</p>
					{:else if filtering && !rowCount && !loading}<p class="hint">No {group.title.toLowerCase()} match “{filter.trim()}”.</p>{/if}
				</div>
				{#if loadErrors[group.key]}<p class="hint err" data-testid="plan-rows-error">Couldn't load more rows: {loadErrors[group.key]} <Button variant="link" size="inline" class="text-[12px]" onclick={() => retry(group.key)}>Retry</Button></p>{/if}
				{#if remaining > 0}
					<Button variant="link" size="inline" class="show-more mt-2 self-start text-[12px]" disabled={loading} onclick={showMore}>
						{loading ? 'Loading…' : `Show more · ${remaining.toLocaleString()} not shown`}
					</Button>
				{:else if loading}<p class="hint">Loading…</p>{/if}
			{/if}
			{/if}
		</section>
	{/if}
</div>

<style>
	/* The pane stops at 1,100 px, as wide lines read badly; the page's toolbar ends with it. */
	.plan-browser {
		display: grid;
		max-width: calc(230px + 24px + 1100px);
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
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		margin: 2px 0 3px;
		padding: 0 8px 0 10px;
		color: var(--muted-foreground);
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
	.heading b {
		color: var(--foreground);
		font-size: 13px;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0;
	}
	.heading.wont {
		margin-top: 14px;
	}
	.heading.wont b {
		color: var(--muted-foreground);
	}
	.heading.cant {
		margin-top: 14px;
		color: var(--sb-error);
	}
	.heading.cant b {
		color: var(--sb-error);
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
	.count.muted,
	.count .muted {
		color: color-mix(in srgb, var(--sb-text-faint) 70%, transparent);
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
	.diff-note {
		margin: 4px 0 2px 16px;
		color: var(--sb-text-faint);
		font-size: 12px;
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
