<script lang="ts">
	import { untrack } from 'svelte';
	import { newRequestId } from '$lib/browser/request-id';
	import { plural, noun } from '$lib/format/plural';
	import HelpTip from '$lib/components/help-tip.svelte';
	import PageBody from '$lib/components/page-body.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import { goto } from '$app/navigation';
	import HammerIcon from '@lucide/svelte/icons/hammer';
	import BellOffIcon from '@lucide/svelte/icons/bell-off';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import AlertCircleIcon from '@lucide/svelte/icons/circle-alert';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import InfoIcon from '@lucide/svelte/icons/info';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import RunTimeline from '$lib/components/timeline/run-timeline.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { catalogNodes, useCatalogNodes } from '$lib/state/catalog.svelte';
	import { folderCatalogHref, folderSelections } from '$lib/selection/folders';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { runStatusLabel } from '$lib/status';
	import { attentionViews, type AttentionTone } from '$lib/build-area/attention';
	import type { AttentionItem, Overview } from '$lib/api';
	import { CLEARED_REASON, acknowledgementOf, clearedWithdrawalId, judgedRead, snoozeWords, withdrawalsDue, type JudgedRead } from '$lib/build-area/attention-conditions';
	import { nodeChangeLine, summarizeChange } from '$lib/build-area/changes';
	import { cappedFolders, quietDeployLine, recentDeployRows } from '$lib/build-area/overview-panels';
	import { pagesBeyondTarget } from '$lib/nav';
	import { CREDIT_PRICE_HINT, formatQueryCost, reportsCost } from '$lib/operate/cost';

	const catalog = useCatalogNodes();
	/** Each top-level folder as a selection, for the folder rows' links to the catalog. */
	const folderSelection = $derived(folderSelections(catalog.data ?? []));
	// Reads again when the shared listing changes, which node status changes update; until it is held
	// the overview waits for it rather than listing every node a second time.
	// Each read is noted with its scope and whether node state had settled (no sync running, the last one
	// succeeded) when it began, so cleared acknowledgements are judged only from a read that can show them.
	let lastRead = $state.raw<JudgedRead<Overview> | null>(null);
	const overview = createResource((client, scope) => {
		const nodes = catalog.data;
		if (!nodes && !catalog.error) return new Promise<never>(() => {});
		// The shared listing can still hold the previous target's nodes for a moment after a switch.
		if (nodes && catalogNodes.view.scopeKey !== scope.locationId) return new Promise<never>(() => {});
		const nodesSettled = untrack(() => {
			const sync = app.warehouseSync;
			return !client.supports('getWarehouseSync') || (sync !== null && !sync.syncing && !sync.lastError);
		});
		return client.overview(scope, nodes).then((read) => {
			if (!read) return read;
			const readId = newRequestId();
			const marked = { ...read, readId };
			lastRead = { overview: marked, readId, scopeKey: scope.locationId, nodesSettled };
			return marked;
		});
	}, ['runs', 'checks', 'snapshot', 'platform_events', 'triggers']);
	const data = $derived(overview.data ?? null);
	// Null data means the person may view only part of the target, so the page offers others instead.
	// A reload keeps the previous data, so only a settled, error-free null counts.
	const partial = $derived(overview.data === null && !overview.loading && !overview.error);
	const otherPages = $derived(pagesBeyondTarget(app.client, app.capabilities));

	const attentionIcon: Record<AttentionTone, typeof AlertCircleIcon> = {
		error: AlertCircleIcon,
		warning: TriangleAlertIcon,
		stale: HistoryIcon,
		info: InfoIcon
	};
	const viewed = $derived(attentionViews(data?.attention ?? [], app.now, catalogNodes.view.changedBuild));
	// A run is acknowledged for good; an ongoing condition until it changes (see attention-conditions).
	const conditionFor = (item: AttentionItem) => ('condition' in item ? (item.condition ?? null) : null);
	// The server's snooze: 24 hours unless it says otherwise; null when acknowledgements have no time limit.
	const snoozeSeconds = $derived(data?.attentionSnoozeSeconds === undefined ? 86_400 : data.attentionSnoozeSeconds);
	const ACK_HELP = $derived({
		run: 'Hide this from the attention list for everyone viewing this target. It stays under Acknowledged, where anyone can undo it.',
		condition: `Hide this from the attention list for everyone viewing this target, until it changes: it gets worse, or clears and comes back${snoozeSeconds === null ? '' : `, or after ${snoozeWords(snoozeSeconds)}`}. It stays under Acknowledged, where anyone can undo it.`
	});
	const handledLabel = (until: string | null | undefined) =>
		until === undefined ? 'for good' : until === null ? 'hidden until it changes' : `hidden until it changes, or until ${formatStamp(until).slice(5, 16)}`;
	const canAck = (item: AttentionItem) => ('acknowledged' in item ? canAcknowledge : !!conditionFor(item) && canAcknowledgeConditions);
	// Acknowledged problems no longer need attention: they leave the count and the banner's tone.
	// Acknowledged or undone here, and saved, while the Overview reads again: the item moves at once rather
	// than after the whole Overview is rebuilt, which on a large project takes a while.
	let settling = $state<ReadonlyMap<string, boolean>>(new Map());
	const isHandled = (item: AttentionItem, key: string) => settling.get(key) ?? !!acknowledgementOf(item);
	const issues = $derived(viewed.filter(({ item, view }) => !isHandled(item, view.key)));
	const handled = $derived(viewed.filter(({ item, view }) => isHandled(item, view.key)));
	let showHandled = $state(false);
	let acknowledging = $state<string | null>(null);
	let acknowledgeError = $state<string | null>(null);
	/** Acknowledges an item, or undoes that: a run by its ID, a condition as it looks now. */
	async function acknowledge(item: AttentionItem, key: string, acknowledged: boolean) {
		const scope = app.scope;
		const condition = conditionFor(item);
		if (!scope || acknowledging) return;
		acknowledging = key;
		acknowledgeError = null;
		try {
			if ('acknowledged' in item) await app.client.acknowledgeRun(scope, item.run.id, acknowledged);
			else if (condition) await app.client.acknowledgeCondition(scope, condition.key, condition.shape, acknowledged);
		} catch (error) {
			// Only the acknowledgement itself failing is an error; it then stays where it was.
			acknowledgeError = error instanceof Error ? error.message : String(error);
			return;
		} finally {
			acknowledging = null;
		}
		// Saved: it moves now, and stays moved until the Overview has been read again. A failed read is the
		// Overview's own error, not the acknowledgement's.
		settling = new Map([...settling, [key, acknowledged]]);
		overview.refresh();
	}
	// An acknowledged condition that has cleared is withdrawn, so if it comes back it shows as new. Only from
	// a complete read (the overview lists only what it could see in full) and never while node state is
	// syncing or its last sync failed, when nodes out of date may just not be known yet. Every viewer sends
	// the same event, which the server applies once; runs are acknowledged for good and never withdrawn.
	// The first read on opening the page usually begins before the sync status arrives, so it can't be
	// judged for nodes out of date; once node state is known to have settled, that read is taken again once.
	const reread = new WeakSet<object>();
	$effect(() => {
		const read = lastRead;
		const sync = app.warehouseSync;
		if (!read || read.nodesSettled || read.scopeKey !== app.scope?.locationId) return;
		if (!sync || sync.syncing || sync.lastError || reread.has(read)) return;
		reread.add(read);
		untrack(() => overview.refresh());
	});
	const withdrawn = new Set<string>();
	$effect(() => {
		const scope = app.scope;
		const read = judgedRead(lastRead, { shown: data, scopeKey: scope?.locationId ?? null, failed: !!overview.error });
		const due = read ? withdrawalsDue(read.overview.clearedAcknowledgements, { failed: false, syncing: !read.nodesSettled }) : [];
		if (!scope || !canAcknowledgeConditions || !due.length) return;
		untrack(() => {
			for (const ack of due) {
				const eventId = clearedWithdrawalId(ack);
				if (withdrawn.has(eventId)) continue;
				withdrawn.add(eventId);
				const automatic = { eventId, reason: CLEARED_REASON, withdraws: ack.eventId };
				void app.client.acknowledgeCondition(scope, ack.key, ack.shape, false, automatic).catch(() => withdrawn.delete(eventId));
			}
		});
	});
	// A new read of the Overview holds every saved acknowledgement, so nothing is settling any more.
	$effect(() => {
		void data;
		untrack(() => {
			if (settling.size) settling = new Map();
		});
	});
	const staleListed = $derived(issues.some(({ item }) => item.kind === 'stale_models'));
	// Until the first read settles there is nothing to judge, so no attention panel or line shows.
	const loaded = $derived(data !== null);
	// The panel shows only while something needs attention; its edge takes the worst item's tone.
	const healthTone = $derived(
		issues.some((i) => i.view.tone === 'error') ? 'var(--sb-error)' : issues.some((i) => i.view.tone === 'warning') ? 'var(--sb-warning)' : 'var(--sb-stale)'
	);
	const latest = $derived(data?.latestRun ?? null);
	const latestText = $derived(latest ? `Last run ${runStatusLabel[latest.status].toLowerCase()} ${formatAgo(latest.endedAt, app.now)}` : 'No runs yet');
	const m = $derived(data?.metrics);
	const finished = $derived(m ? m.runs24h.success + m.runs24h.warn + m.runs24h.partial + m.runs24h.failed : 0);
	const successRate = $derived(m && finished ? Math.round(((m.runs24h.success + m.runs24h.warn) / finished) * 100) : null);
	// The last 7 days' query cost, for locations whose adapter reports cost and once any run has some.
	const showCost = $derived(reportsCost(app.location?.target.adapter) && app.client.supports('getCostSummary'));
	const costWeek = createResource((client, scope) => (showCost ? client.getCostSummary(scope, 7).catch(() => null) : Promise.resolve(null)), ['runs', 'warehouse_sync', 'snapshot']);
	const weekCost = $derived(costWeek.data?.runs ? costWeek.data : null);
	const groups = $derived(data?.groups ?? []);
	// At most eight folders, most urgent first; the catalog's folder view lists them all.
	const folders = $derived(cappedFolders(groups));
	const ALL_FOLDERS_HREF = '/catalog?view=folders';
	const deployRows = $derived(recentDeployRows(data?.recentChanges ?? []));
	// Parts without a server read stay hidden rather than showing zeros, and wait for the read so they don't appear and vanish.
	const showGroups = $derived(!!data?.groups?.length);
	const showTimeline = $derived(app.client.supports('timeline'));
	const canAcknowledge = $derived(app.client.supports('acknowledgeRun'));
	const canAcknowledgeConditions = $derived(app.client.supports('acknowledgeCondition'));
	const FAILING_CHECKS_HREF = '/checks?outcome=failing';

</script>

<!-- The alerts below list every platform problem, so the banner would only repeat them. -->
<AppTopbar title="Overview" events={() => false}>
	<RefreshButton onclick={() => overview.refresh()} />
</AppTopbar>


{#snippet handledRows()}
	<div class="handled-rows">
	{#each handled as { item, view } (view.key)}
		{@const ack = acknowledgementOf(item) ?? { by: 'you', at: new Date(app.now).toISOString(), until: 'acknowledged' in item ? undefined : snoozeSeconds === null ? null : new Date(app.now + snoozeSeconds * 1000).toISOString() }}
		<div class="issue muted">
			<span class="issue-icon info"><BellOffIcon size={13} /></span>
			<div class="issue-copy">
				<a class="issue-title" href={to(view.href)}>{view.title}</a>
				<p class="issue-detail">{ack ? `Acknowledged by ${ack.by} · ${formatAgo(ack.at, app.now)} · ${handledLabel(ack.until)}` : ''}</p>
			</div>
			<small>{view.since}</small>
			<span class="issue-ack">
				{#if canAck(item)}
					<Button variant="ghost" size="sm" disabled={acknowledging === view.key} onclick={() => acknowledge(item, view.key, false)} data-testid="attention-undo">Undo</Button>
				{/if}
			</span>
			<span class="issue-actions"><a href={to(view.href)}>{view.action} →</a></span>
		</div>
	{/each}
	</div>
{/snippet}

<!-- Padding sits on an inner box: sticky headers stick below a scroller's own padding, so rows would show through above them. -->
<PageBody padding="18px 20px 64px">
	{#if overview.error}<div class="error-note mb-[14px]">{overview.error}</div>{/if}
	{#if partial}
		<section class="surface-panel partial">
			<h2>You can see only part of {app.location?.name} · {app.target?.name}</h2>
			<p>Needs view on every node in this target. Ask an administrator.</p>
			{#if otherPages.length}
				<p>Pages you can use:</p>
				<ul>
					{#each otherPages as item (item.href)}<li><a href={to(item.href)}>{item.label} →</a></li>{/each}
				</ul>
			{/if}
			{#if (app.bootstrap?.locations.length ?? 0) > 1}<p>Or choose another location from the menu at the top of the sidebar.</p>{/if}
		</section>
	{:else}
		<div class="flex flex-col gap-[14px]">
			<!-- Attention: the panel while something needs it; with only acknowledged items, a slim line that opens them; otherwise nothing, as the header and cards already say all is well. -->
			{#if loaded && issues.length}
				<section class="health" style:--tone={healthTone}>
					<div class="health-head">
						<div class="health-icon">
							<AlertCircleIcon size={15} />
						</div>
						<div class="health-copy">
							<strong>{app.location?.name} needs attention</strong>
							<span>{issues.length > 1 ? `${issues.length} items · ${latestText}` : latestText}</span>
						</div>
						{#if m}
							<div class="health-facts">
								<!-- The out-of-date alert below already counts these. -->
								{#if m.stale?.changed && !staleListed}<span><i class="status-dot stale"></i><b>{m.stale.changed}</b> changed</span>{/if}
								{#if m.stale?.behind && !staleListed}<span><i class="status-dot behind"></i><b>{m.stale.behind}</b> behind</span>{/if}
								{#if m.checks.failing}<a class="fact-link" href={to(FAILING_CHECKS_HREF)}><i class="status-dot error"></i><b>{m.checks.failing}</b> failing {noun(m.checks.failing, 'check')}</a>{/if}
							</div>
						{/if}
						{#if latest}<a href={to(`/runs/${latest.id}`)}>View latest run →</a>{/if}
					</div>
					{#if issues.length}
						<div class="issues">
							{#each issues as { item, view } (view.key)}
								{@const Glyph = attentionIcon[view.tone]}
								<div class="issue">
									<span class="issue-icon {view.tone}"><Glyph size={14} /></span>
									<div class="issue-copy">
										<a class="issue-title" href={to(view.href)}>{view.title}</a>
										<p class="issue-detail">{view.detail}</p>
										{#if conditionFor(item)?.lapsed}
											{@const lapsed = conditionFor(item)!.lapsed!}
											<p class="lapsed" data-testid="attention-lapsed">Back on the list: {lapsed.reason} (acknowledged by {lapsed.by} {formatAgo(lapsed.at, app.now)}).</p>
										{/if}
										{#if view.consequences.length}
											<ul class="consequences" aria-label="Caused by this">
												{#each view.consequences as c, i (i)}<li><a href={to(c.href)}>{c.text}</a></li>{/each}
											</ul>
										{/if}
									</div>
									<small>{view.since}</small>
									<span class="issue-ack">
										{#if canAck(item)}
											<!-- Short text, always visible, stronger while the row is hovered; the tooltip says what it does. -->
											<Button variant="ghost" size="sm" class="ack" title={'acknowledged' in item ? ACK_HELP.run : ACK_HELP.condition} aria-label="Acknowledge" disabled={acknowledging === view.key} onclick={() => acknowledge(item, view.key, true)} data-testid="attention-ack">
												<BellOffIcon />Ack
											</Button>
										{/if}
									</span>
									<span class="issue-actions">
										{#if view.build}
											{@const build = view.build}
											<GatedButton requires="run.adhoc" size="sm" variant={build.variant} hint={build.tooltip ?? undefined} onclick={() => goto(to(build.href))}
												>{#if build.variant === 'stale'}<HammerIcon class="size-3.5" />{/if}<span>{build.label}{#if build.extra}<span class="label-extra">{build.extra}</span>{/if}</span></GatedButton
											>
										{/if}
										<a href={to(view.href)}>{view.action} →</a>
									</span>
								</div>
							{/each}
						</div>
					{/if}
					{#if handled.length}
						<div class="handled">
							<DisclosureButton open={showHandled} onclick={() => (showHandled = !showHandled)} class="h-[34px] pr-3.5 pl-[17px] text-[12px]">Acknowledged ({handled.length})</DisclosureButton>
							{#if showHandled}{@render handledRows()}{/if}
						</div>
					{/if}
					{#if acknowledgeError}<p class="ack-error">{acknowledgeError}</p>{/if}
				</section>
			{:else if loaded && handled.length}
				<section class="quiet" data-testid="attention-quiet">
					<div class="quiet-head">
						<span>Nothing needs attention</span>
						<span class="sep">·</span>
						<DisclosureButton open={showHandled} onclick={() => (showHandled = !showHandled)} class="h-[30px] px-1.5 text-[12px]">Acknowledged ({handled.length})</DisclosureButton>
					</div>
					{#if showHandled}{@render handledRows()}{/if}
					{#if acknowledgeError}<p class="ack-error">{acknowledgeError}</p>{/if}
				</section>
			{:else if acknowledgeError}
				<p class="ack-error standalone">{acknowledgeError}</p>
			{/if}

			<section class="metric-strip" style:--metrics={3 + (m?.stale ? 1 : 0) + (weekCost ? 1 : 0)}>
				{#if m}
					<div class="metric">
						<span class="metric-label">Models</span>
						<div><strong>{m.models.total}</strong><small>deployed</small></div>
						<span class:success-note={m.models.addedThisWeek > 0}>{m.models.addedThisWeek || m.models.removedThisWeek ? `+${m.models.addedThisWeek} / −${m.models.removedThisWeek} this week` : 'No changes this week'}</span>
					</div>
					<div class="metric">
						<span class="metric-label">Runs · 24h</span>
						<div><strong>{m.runs24h.total}</strong><small>{successRate === null ? 'none finished' : `${successRate}% succeeded`}</small></div>
						<span class:warning-note={m.runs24h.partial + m.runs24h.failed > 0}>{m.runs24h.partial} partial · {m.runs24h.failed} failed · {m.runs24h.active} active</span>
					</div>
					<div class="metric">
						<span class="metric-label">Checks passing</span>
						<div><strong>{m.checks.passing}</strong><small>of {m.checks.total}</small></div>
						<span class:warning-note={m.checks.warning + m.checks.failing > 0}>{#if m.checks.failing}<a class="metric-link" href={to(FAILING_CHECKS_HREF)}>{m.checks.failing} failing</a>{:else}0 failing{/if} · {m.checks.warning} warning</span>
					</div>
					{#if m.stale}
						<div class="metric">
							<span class="metric-label">Stale</span>
							<div><strong class="stale-value">{m.stale.nodes}</strong><small>nodes</small></div>
							<span>{m.stale.changed} changed · {m.stale.behind} behind{#if m.stale.sources !== null} · {plural(m.stale.sources, 'source')} late{/if}</span>
						</div>
					{/if}
					{#if weekCost}
						<div class="metric" data-testid="cost-card">
							<span class="metric-label">Cost · 7 days{#if weekCost.usd === null}<HelpTip text={CREDIT_PRICE_HINT} label="Why credits" />{/if}</span>
							<div><strong>{formatQueryCost(weekCost)}</strong></div>
							<span>{plural(weekCost.runs, 'run')} · <a class="metric-link" href={to('/cost')}>By job and folder</a></span>
						</div>
					{/if}
				{/if}
			</section>

			<div class="lower-grid" class:single={!showGroups}>
				{#if showGroups}<section class="surface-panel">
					<div class="panel-heading">
						<div><h2>Build status by folder</h2></div>
						<a href={to(`/catalog`)}>Open catalog →</a>
					</div>
					<div class="groups">
						<div class="group-row head"><span>Folder</span><span></span><span>Current</span><span>Changed</span><span>Behind</span><span>Failed</span></div>
						{#each folders.rows as g (g.group)}
							<a class="group-row" href={to(folderCatalogHref(folderSelection.get(g.group) ?? ''))} title="Open {g.group} in the catalog">
								<span>{g.group}</span>
								<span class="group-bar" title="{g.current} of {g.total} current">
									<i class="fresh" style:width="{(g.current / g.total) * 100}%"></i>
									<i class="stale-segment" style:width="{(g.changed / g.total) * 100}%"></i>
									<i class="fail-segment" style:width="{(g.failed / g.total) * 100}%"></i>
								</span>
								<b>{g.current}/{g.total}</b>
								<b class:stale-text={g.changed}>{g.changed || '—'}</b>
								<b class:behind-text={g.behind}>{g.behind || '—'}</b>
								<b class:error-text={g.failed}>{g.failed || '—'}</b>
							</a>
						{:else}
							<div class="empty">{#if overview.data === undefined && !overview.error}<Loading />{:else}Nothing deployed yet.{/if}</div>
						{/each}
						{#if folders.total > folders.rows.length}
							<a class="all-folders" href={to(ALL_FOLDERS_HREF)} data-testid="all-folders">All {folders.total} folders ({folders.attention} need attention) →</a>
						{/if}
					</div>
				</section>{/if}
				<section class="surface-panel">
					<div class="panel-heading">
						<div><h2>Recent deploys</h2></div>
						<a href={to(`/changes`)}>All changes →</a>
					</div>
					<div>
						{#each deployRows as row (row.key)}
							{#if row.kind === 'quiet'}
								<div class="change quiet">{quietDeployLine(row, app.now)}</div>
							{:else}
							{@const c = row.change}
							<a class="change" href={to(`/changes#gen-${c.generation}`)}>
								<span class="change-rev" title={c.gitRevision ? `Commit ${c.gitRevision}` : 'Published outside a git checkout'}><code>{c.gitRevision?.slice(0, 7) ?? '—'}</code><small>version {c.generation}</small></span>
								<span class="change-copy">
									<strong>{summarizeChange(c)}</strong>
									<span>{nodeChangeLine(c.nodes)}</span>
								</span>
								<small>{c.gitAuthor ? `${c.gitAuthor} · ` : ''}{formatAgo(c.publishedAt, app.now)}</small>
							</a>
							{/if}
						{:else}
							<div class="empty">{#if overview.data === undefined && !overview.error}<Loading />{:else}Nothing deployed yet.{/if}</div>
						{/each}
					</div>
				</section>
			</div>

			{#if showTimeline}
				<section class="surface-panel flex min-w-0 flex-col">
					<div class="panel-heading">
						<div><h2>Activity <HelpTip text="Past runs and the next scheduled times" /></h2><span>Past and upcoming</span></div>
						<a href={to(`/runs?view=timeline`)}>Open full timeline →</a>
					</div>
					<RunTimeline scroll={false} stickyTop={0} foldIdle />
				</section>
			{/if}

		</div>
	{/if}
</PageBody>

<style>
	.health, .metric-strip, .surface-panel { box-shadow: var(--panel-shadow); border: 1px solid var(--panel-border); border-radius: 2px; background: var(--card); }
	.health { overflow: hidden; border-color: color-mix(in srgb, var(--tone) 30%, var(--border)); border-left: 3px solid var(--tone); }
	.health-head { display: flex; min-height: 49px; align-items: center; padding: 8px 13px; }
	.issues { border-top: 1px solid color-mix(in srgb, var(--tone) 18%, var(--border)); }
	/* Rows share the columns, so times, Acknowledge and links line up down the list. */
	/* The icon track is max-content so it also holds the rows' left padding, which a subgrid adds to its edge track. */
	.issues, .handled-rows { display: grid; grid-template-columns: max-content minmax(0, 1fr) auto auto auto; column-gap: 10px; }
	.issue { display: grid; grid-column: 1 / -1; grid-template-columns: subgrid; align-items: start; padding: 9px 14px 9px 17px; border-top: 1px solid var(--inner-line); font-size: 12.5px; }
	.issue:first-child { border-top: 0; }
	.issue:hover { background: var(--sb-hover); }
	.issue > :not(.issue-copy, .issue-icon) { min-height: 22px; display: flex; align-items: center; }
	.issue-copy { min-width: 0; padding-top: 2px; }
	.issue-title { font-weight: 600; }
	.issue-title:hover { text-decoration: underline; }
	.issue-detail { margin: 2px 0 0; color: var(--muted-foreground); overflow-wrap: anywhere; }
	.consequences { margin: 4px 0 0; padding: 0; list-style: none; }
	.consequences li { position: relative; padding-left: 14px; color: var(--muted-foreground); overflow-wrap: anywhere; }
	.consequences li::before { position: absolute; left: 2px; content: '↳'; color: var(--sb-text-faint); }
	.consequences a:hover { color: var(--foreground); text-decoration: underline; }
	.issue small { justify-content: flex-end; color: var(--sb-text-faint); font-size: 12px; white-space: nowrap; }
	.issue-ack { justify-content: flex-end; }
	.issue-actions { justify-content: flex-end; gap: 10px; }
	.issue-actions a { color: var(--primary); font-weight: 500; white-space: nowrap; }
	.handled { border-top: 1px solid var(--inner-line); }
	.all-folders { align-self: flex-start; margin: 6px 16px 4px; color: var(--primary); font-size: 12px; font-weight: 500; }
	.all-folders:hover { text-decoration: underline; }
	.lapsed { margin: 4px 0 0; color: var(--sb-warning); font-size: 12px; }
	/* Ack: muted text with a bell, always visible; on row hover it gains a border and full colour. */
	.issue :global(.ack) { height: 24px; padding: 0 8px; border: 1px solid transparent; color: var(--muted-foreground); font-size: 12px; font-weight: 500; }
	.issue:hover :global(.ack), .issue :global(.ack:focus-visible) { border-color: var(--border); background: var(--sb-hover); color: var(--foreground); }
	.issue :global(.ack:hover) { border-color: var(--muted-foreground); }
	.issue.muted .issue-title { color: var(--muted-foreground); font-weight: 500; }
	.ack-error { margin: 0; padding: 8px 17px; border-top: 1px solid var(--inner-line); color: var(--sb-error); font-size: 12px; }
	.ack-error.standalone { border-top: 0; padding: 0; }
	/* Only acknowledged items: a neutral line, not a panel. */
	.quiet { border: 1px solid var(--panel-border); border-radius: 2px; background: var(--card); }
	.quiet-head { display: flex; min-height: 32px; align-items: center; gap: 6px; padding: 0 8px 0 13px; color: var(--muted-foreground); font-size: 12px; }
	.quiet-head .sep { color: var(--sb-text-faint); }
	.quiet .handled-rows { border-top: 1px solid var(--inner-line); }
	.issue-icon { display: grid; width: 22px; height: 22px; place-items: center; border-radius: 2px; }
	.issue-icon.error { color: var(--sb-error); background: color-mix(in srgb, var(--sb-error) 12%, transparent); }
	.issue-icon.warning { color: var(--sb-warning); background: color-mix(in srgb, var(--sb-warning) 12%, transparent); }
	.issue-icon.stale { color: var(--sb-stale); background: color-mix(in srgb, var(--sb-stale) 12%, transparent); }
	.issue-icon.info { color: var(--muted-foreground); background: var(--sb-hover); }
	.change { display: grid; grid-template-columns: 72px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 10px 14px; border-bottom: 1px solid var(--inner-line); }
	.change:last-child { border-bottom: 0; }
	.change.quiet { display: block; padding: 7px 14px 7px 96px; color: var(--sb-text-faint); font-size: 12px; }
	.change:hover { background: var(--sb-hover); }
	.change-rev code { display: block; color: var(--primary); font: 12px var(--font-mono); }
	.change-rev small { color: var(--sb-text-faint); font-size: 11px; }
	.change-copy { min-width: 0; }
	.change-copy strong, .change-copy span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.change-copy strong { font-size: 12.5px; font-weight: 600; }
	.change-copy span { margin-top: 2px; color: var(--muted-foreground); font-size: 12px; }
	.change small { color: var(--sb-text-faint); font-size: 12px; white-space: nowrap; }
	.health-icon { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 2px; color: var(--tone); background: color-mix(in srgb, var(--tone) 12%, transparent); }
	.health-copy { margin-left: 11px; }
	.health-copy strong, .health-copy span { display: block; }
	.health-copy strong { font-size: 13px; }
	.health-copy span { margin-top: 2px; color: var(--muted-foreground); font-size: 12px; }
	.health-facts { display: flex; gap: 20px; margin-left: 44px; color: var(--muted-foreground); font: 12px var(--font-sans); }
	.health-facts span, .health-facts .fact-link { display: flex; align-items: center; gap: 6px; }
	.health-facts .fact-link:hover, .metric-link:hover { color: var(--foreground); text-decoration: underline; }
	.health-facts b { color: var(--foreground); font-weight: 500; }
	.health-head > a, .panel-heading > a { margin-left: auto; color: var(--primary); font: 500 12.5px var(--font-sans); white-space: nowrap; }
	.metric-strip { display: grid; min-height: 98px; grid-template-columns: repeat(var(--metrics), minmax(0, 1fr)); overflow: hidden; }
	.metric { min-width: 0; padding: 15px 16px 14px; border-right: 1px solid var(--border); }
	.metric:last-child { border-right: 0; }
	.metric-label { display: block; color: var(--muted-foreground); font: 500 12px var(--font-sans); }
	.metric > div { display: flex; align-items: baseline; gap: 8px; margin-top: 7px; }
	.metric strong { font: 600 24px/1 var(--font-sans); letter-spacing: -.02em; font-variant-numeric: tabular-nums; }
	.metric small { color: var(--muted-foreground); font-size: 12px; }
	.metric > span:last-child { display: block; margin-top: 9px; overflow: hidden; color: var(--sb-text-faint); font: 12px var(--font-sans); text-overflow: ellipsis; white-space: nowrap; }
	.metric .success-note { color: var(--sb-success); }
	.metric .warning-note { color: var(--sb-warning); }
	.stale-value { color: var(--sb-stale); }
	.lower-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(360px, 0.9fr); gap: 14px; align-items: stretch; }
	.lower-grid.single { grid-template-columns: 1fr; }
	.panel-heading { display: flex; min-height: 53px; align-items: center; padding: 0 14px 0 16px; border-bottom: 1px solid var(--border); }
	.panel-heading > div:first-child { margin-right: auto; }
	.panel-heading h2 { margin: 0; font: 600 13.5px var(--font-sans); }
	.panel-heading h2 + span { display: block; margin-top: 2px; color: var(--muted-foreground); font-size: 12px; }
	.status-dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; }
	.warning { background: var(--sb-warning); }
	.error { background: var(--sb-error); }
	.stale { background: var(--sb-stale); }
	.behind { box-shadow: inset 0 0 0 1.5px var(--sb-stale); background: color-mix(in srgb, var(--sb-stale) 25%, transparent); }
	.groups { padding: 6px 0 8px; }
	.group-row { display: grid; grid-template-columns: 110px minmax(80px, 1fr) 60px 64px 56px 52px; align-items: center; gap: 12px; padding: 7px 16px; font-size: 12.5px; }
	a.group-row:hover { background: var(--sb-hover); }
	.group-row.head { color: var(--sb-text-faint); font-size: 11.5px; }
	.group-row > span:first-child { color: var(--muted-foreground); }
	.group-row b { color: var(--foreground); font-weight: 500; text-align: right; font-variant-numeric: tabular-nums; }
	.group-row.head span:not(:first-child) { text-align: right; }
	.group-bar { display: flex; height: 7px; overflow: hidden; border-radius: 2px; background: var(--sb-hover); }
	.group-bar .fresh { background: var(--sb-success); }
	.stale-segment { background: var(--sb-stale); }
	.fail-segment { background: var(--sb-error); }
	.stale-text { color: var(--sb-stale) !important; }
	.behind-text { color: var(--sb-behind) !important; }
	.error-text { color: var(--sb-error) !important; }
	.empty { padding: 18px 16px; color: var(--muted-foreground); font-size: 12.5px; }
	.partial { max-width: 640px; padding: 18px 20px; font-size: 13px; }
	.partial h2 { margin-bottom: 8px; font-size: 14px; font-weight: 600; }
	.partial p { margin-top: 6px; color: var(--muted-foreground); }
	.partial ul { display: flex; flex-wrap: wrap; gap: 6px 16px; margin-top: 6px; }
	.partial a { color: var(--primary); font-weight: 500; }
	.error-note { padding: 10px 14px; border: 1px solid var(--sb-error); color: var(--sb-error); font-size: 12.5px; }
	@media (max-width: 1250px) {
		.lower-grid { grid-template-columns: 1fr; }
		.health-facts { display: none; }
		.metric { padding-right: 12px; padding-left: 12px; }
	}
</style>
