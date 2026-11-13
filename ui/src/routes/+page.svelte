<script lang="ts">
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import { goto } from '$app/navigation';
	import ActivityIcon from '@lucide/svelte/icons/activity';
	import BellOffIcon from '@lucide/svelte/icons/bell-off';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import AlertCircleIcon from '@lucide/svelte/icons/circle-alert';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import InfoIcon from '@lucide/svelte/icons/info';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import RunTimeline from '$lib/components/timeline/run-timeline.svelte';
	import { planHref } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo } from '$lib/format/time';
	import { runStatusLabel } from '$lib/status';
	import { attentionRank, attentionView, type AttentionTone } from '$lib/build-area/attention';
	import type { AttentionItem } from '$lib/api';
	import { formatCredits } from '$lib/build-area/labels';
	import { nodeChangeLine, summarizeChange } from '$lib/build-area/changes';
	import { pagesBeyondTarget } from '$lib/nav';

	const overview = createResource((client, scope) => client.overview(scope), ['runs', 'checks', 'node_status', 'snapshot', 'platform_events', 'triggers']);
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
	const viewed = $derived(
		(data?.attention ?? []).map((item) => ({ item, view: attentionView(item, app.now) })).sort((a, b) => attentionRank(a.view.tone) - attentionRank(b.view.tone))
	);
	const acknowledgementOf = (item: AttentionItem) => ('acknowledged' in item ? item.acknowledged : null);
	// Acknowledged problems no longer need attention: they leave the count and the banner's tone.
	const issues = $derived(viewed.filter(({ item }) => !acknowledgementOf(item)));
	const handled = $derived(viewed.filter(({ item }) => acknowledgementOf(item)));
	let showHandled = $state(false);
	let acknowledging = $state<string | null>(null);
	let acknowledgeError = $state<string | null>(null);
	async function acknowledge(runId: string, acknowledged: boolean) {
		const scope = app.scope;
		if (!scope || acknowledging) return;
		acknowledging = runId;
		acknowledgeError = null;
		try {
			await app.client.acknowledgeRun(scope, runId, acknowledged);
			await overview.refresh();
		} catch (error) {
			acknowledgeError = error instanceof Error ? error.message : String(error);
		} finally {
			acknowledging = null;
		}
	}
	const stale = $derived(data?.attention.find((i) => i.kind === 'stale_models'));
	const staleSelection = $derived(stale && stale.kind === 'stale_models' ? [...new Set([...stale.changed, ...stale.behind])] : []);
	// Until the first read settles there is nothing to judge, so the banner stays neutral rather than claiming health.
	const loaded = $derived(data !== null);
	const healthTone = $derived(
		!loaded ? 'var(--sb-text-faint)' : issues.some((i) => i.view.tone === 'error') ? 'var(--sb-error)' : issues.some((i) => i.view.tone === 'warning' || i.view.tone === 'stale') ? 'var(--sb-warning)' : 'var(--sb-success)'
	);
	const latest = $derived(data?.latestRun ?? null);
	const latestText = $derived(latest ? `Last run ${runStatusLabel[latest.status].toLowerCase()} ${formatAgo(latest.endedAt, app.now)}` : 'No runs yet');
	const m = $derived(data?.metrics);
	const finished = $derived(m ? m.runs24h.success + m.runs24h.warn + m.runs24h.partial + m.runs24h.failed : 0);
	const successRate = $derived(m && finished ? Math.round(((m.runs24h.success + m.runs24h.warn) / finished) * 100) : null);
	const costChange = $derived(m?.cost && m.cost.baseline ? Math.round(((m.cost.last24h - m.cost.baseline) / m.cost.baseline) * 100) : null);
	const groups = $derived(data?.groups ?? []);
	// Parts without a server read stay hidden rather than showing zeros, and wait for the read so they don't appear and vanish.
	const showGroups = $derived(!!data && data.groups !== null);
	const showTimeline = $derived(app.client.supports('timeline'));
	const canAcknowledge = $derived(app.client.supports('acknowledgeRun'));

</script>

<AppTopbar title="Overview">
	<Button variant="outline" size="sm" onclick={() => overview.refresh()}>↻ Refresh</Button>
</AppTopbar>


<div class="min-h-0 flex-1 overflow-y-auto px-5 pb-16 pt-[18px]">
	{#if overview.error}<div class="error-note mb-[14px]">{overview.error}</div>{/if}
	{#if partial}
		<section class="surface-panel partial">
			<h2>You can see only part of {app.location?.name} · {app.target?.name}</h2>
			<p>The Overview sums up the whole target, so it needs view on every node in it. An administrator can give you view on the rest.</p>
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
			<section class="health" style:--tone={healthTone}>
				<div class="health-head">
					<div class="health-icon">
						{#if loaded && issues.length}<AlertCircleIcon size={15} />{:else}<ActivityIcon size={15} />{/if}
					</div>
					<div class="health-copy">
						{#if !loaded}
							<strong>{app.location?.name}</strong>
							<span>{overview.error ? 'Couldn’t check its health' : 'Checking…'}</span>
						{:else}
							<strong>{issues.length ? `${app.location?.name} needs attention` : `${app.location?.name} is healthy`}</strong>
							<span>{issues.length ? `${issues.length} ${issues.length === 1 ? 'item' : 'items'} · ${latestText}` : latestText}</span>
						{/if}
					</div>
					{#if m}
						<div class="health-facts">
							{#if m.stale?.changed}<span><i class="status-dot stale"></i><b>{m.stale.changed}</b> changed</span>{/if}
							{#if m.stale?.behind}<span><i class="status-dot warning"></i><b>{m.stale.behind}</b> behind</span>{/if}
							{#if m.checks.failing}<span><i class="status-dot error"></i><b>{m.checks.failing}</b> failing {m.checks.failing === 1 ? 'check' : 'checks'}</span>{/if}
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
								<a class="issue-title" href={to(view.href)}>{view.title}</a>
								<span class="issue-detail" title={view.detail}>{view.detail}</span>
								<small>{view.since}</small>
								<span class="issue-actions">
									{#if item.kind === 'stale_models'}
										<GatedButton requires="run.adhoc" size="xs" variant="default" onclick={() => goto(to(planHref(staleSelection)))}>Rebuild {staleSelection.length}</GatedButton>
									{/if}
									{#if 'acknowledged' in item && canAcknowledge}
										<button class="ack" title="Mark as handled for everyone; recorded in the audit log" disabled={acknowledging === item.run.id} onclick={() => acknowledge(item.run.id, true)}>
											<BellOffIcon size={13} />Acknowledge
										</button>
									{/if}
									<a href={to(view.href)}>{view.action} →</a>
								</span>
							</div>
						{/each}
					</div>
				{/if}
				{#if handled.length}
					<div class="handled">
						<button class="handled-toggle" aria-expanded={showHandled} onclick={() => (showHandled = !showHandled)}>
							<ChevronRightIcon size={13} class={showHandled ? 'open' : ''} />Acknowledged · {handled.length}
						</button>
						{#if showHandled}
							{#each handled as { item, view } (view.key)}
								{@const ack = acknowledgementOf(item)}
								<div class="issue muted">
									<span class="issue-icon info"><BellOffIcon size={13} /></span>
									<a class="issue-title" href={to(view.href)}>{view.title}</a>
									<span class="issue-detail">{ack ? `Acknowledged by ${ack.by} · ${formatAgo(ack.at, app.now)}` : ''}</span>
									<small>{view.since}</small>
									<span class="issue-actions">
										{#if 'acknowledged' in item && canAcknowledge}
											<button class="undo" disabled={acknowledging === item.run.id} onclick={() => acknowledge(item.run.id, false)}>Undo</button>
										{/if}
										<a href={to(view.href)}>{view.action} →</a>
									</span>
								</div>
							{/each}
						{/if}
					</div>
				{/if}
				{#if acknowledgeError}<p class="ack-error">{acknowledgeError}</p>{/if}
			</section>

			<section class="metric-strip" style:--metrics={3 + (m?.stale ? 1 : 0) + (m?.cost ? 1 : 0)}>
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
						<span class:warning-note={m.checks.warning + m.checks.failing > 0}>{m.checks.failing} failing · {m.checks.warning} warning</span>
					</div>
					{#if m.stale}
						<div class="metric">
							<span class="metric-label">Stale</span>
							<div><strong class="stale-value">{m.stale.nodes}</strong><small>nodes</small></div>
							<span>{m.stale.changed} changed · {m.stale.behind} behind{#if m.stale.sources !== null} · {m.stale.sources} {m.stale.sources === 1 ? 'source' : 'sources'} late{/if}</span>
						</div>
					{/if}
					{#if m.cost}
						<div class="metric">
							<span class="metric-label">Cost · 24h</span>
							<div><strong>{formatCredits(m.cost.last24h)}</strong></div>
							<span class:warning-note={(costChange ?? 0) >= 20}>{costChange === null ? '—' : `${costChange >= 0 ? '+' : ''}${costChange}%`} vs {m.cost.baselineDays}-day average</span>
						</div>
					{/if}
				{/if}
			</section>

			{#if showTimeline}
				<section class="surface-panel flex min-w-0 flex-col">
					<div class="panel-heading">
						<div><h2>Activity</h2><span>Past runs, and when schedules will run next</span></div>
						<a href={to(`/runs?view=timeline`)}>Open full timeline →</a>
					</div>
					<RunTimeline scroll={false} stickyTop={0} />
				</section>
			{/if}

			<div class="lower-grid" class:single={!showGroups}>
				{#if showGroups}<section class="surface-panel">
					<div class="panel-heading">
						<div><h2>Build status by group</h2><span>Current, stale and failed nodes · {app.location?.name}</span></div>
						<a href={to(`/catalog`)}>Open catalog →</a>
					</div>
					<div class="groups">
						<div class="group-row head"><span>Group</span><span></span><span>Current</span><span>Changed</span><span>Behind</span><span>Failed</span></div>
						{#each groups as g (g.group)}
							<a class="group-row" href={to(`/catalog?group=${g.group}`)}>
								<span>{g.group}</span>
								<span class="group-bar" title="{g.current} of {g.total} current">
									<i class="fresh" style:width="{(g.current / g.total) * 100}%"></i>
									<i class="stale-segment" style:width="{(g.changed / g.total) * 100}%"></i>
									<i class="fail-segment" style:width="{(g.failed / g.total) * 100}%"></i>
								</span>
								<b>{g.current}/{g.total}</b>
								<b class:stale-text={g.changed}>{g.changed || '—'}</b>
								<b class:warn-text={g.behind}>{g.behind || '—'}</b>
								<b class:error-text={g.failed}>{g.failed || '—'}</b>
							</a>
						{:else}
							<div class="empty">{#if overview.data === undefined && !overview.error}<Loading />{:else}Nothing deployed yet. Deploy the project to see its models here.{/if}</div>
						{/each}
					</div>
				</section>{/if}
				<section class="surface-panel">
					<div class="panel-heading">
						<div><h2>Recent changes</h2><span>Recent deploys</span></div>
						<a href={to(`/changes`)}>All changes →</a>
					</div>
					<div>
						{#each data?.recentChanges ?? [] as c (c.snapshotId)}
							<a class="change" href={to(`/changes#gen-${c.generation}`)}>
								<span class="change-rev" title={c.gitRevision ? `Commit ${c.gitRevision}` : 'Published outside a git checkout'}><code>{c.gitRevision?.slice(0, 7) ?? '—'}</code><small>version {c.generation}</small></span>
								<span class="change-copy">
									<strong>{summarizeChange(c)}</strong>
									<span>{nodeChangeLine(c.nodes)}</span>
								</span>
								<small>{c.gitAuthor ? `${c.gitAuthor} · ` : ''}{formatAgo(c.publishedAt, app.now)}</small>
							</a>
						{:else}
							<div class="empty">{#if overview.data === undefined && !overview.error}<Loading />{:else}Nothing deployed yet.{/if}</div>
						{/each}
					</div>
				</section>
			</div>
		</div>
	{/if}
</div>

<style>
	.health, .metric-strip, .surface-panel { box-shadow: var(--panel-shadow); border: 1px solid var(--panel-border); border-radius: 2px; background: var(--card); }
	.health { overflow: hidden; border-color: color-mix(in srgb, var(--tone) 30%, var(--border)); border-left: 3px solid var(--tone); }
	.health-head { display: flex; min-height: 49px; align-items: center; padding: 8px 13px; }
	.issues { border-top: 1px solid color-mix(in srgb, var(--tone) 18%, var(--border)); }
	.issue { display: grid; grid-template-columns: 22px auto minmax(0, 1fr) auto auto; align-items: center; gap: 10px; min-height: 40px; padding: 0 14px 0 17px; border-top: 1px solid var(--inner-line); font-size: 12.5px; }
	.issue:first-child { border-top: 0; }
	.issue:hover { background: var(--sb-hover); }
	.issue-title { font-weight: 600; white-space: nowrap; }
	.issue-title:hover { text-decoration: underline; }
	.issue-detail { overflow: hidden; color: var(--muted-foreground); text-overflow: ellipsis; white-space: nowrap; }
	.issue small { color: var(--sb-text-faint); font-size: 12px; white-space: nowrap; }
	.issue-actions { display: flex; min-width: 120px; align-items: center; justify-content: flex-end; gap: 10px; }
	.issue-actions a { color: var(--primary); font-weight: 500; white-space: nowrap; }
	.ack { display: inline-flex; align-items: center; gap: 5px; height: 22px; padding: 0 8px; border: 1px solid var(--border); border-radius: 2px; color: var(--muted-foreground); font-size: 12px; white-space: nowrap; }
	.ack:hover:not(:disabled) { border-color: var(--border-strong); color: var(--foreground); }
	.ack:disabled, .undo:disabled { opacity: 0.5; }
	.handled { border-top: 1px solid var(--inner-line); }
	.handled-toggle { display: flex; align-items: center; gap: 6px; height: 34px; padding: 0 14px 0 17px; color: var(--muted-foreground); font-size: 12px; }
	.handled-toggle:hover { color: var(--foreground); }
	.handled-toggle :global(svg) { transition: transform 120ms; }
	.handled-toggle :global(svg.open) { transform: rotate(90deg); }
	.issue.muted .issue-title { color: var(--muted-foreground); font-weight: 500; }
	.undo { color: var(--muted-foreground); font-size: 12px; }
	.undo:hover:not(:disabled) { color: var(--foreground); }
	.ack-error { margin: 0; padding: 8px 17px; border-top: 1px solid var(--inner-line); color: var(--sb-error); font-size: 12px; }
	.issue-icon { display: grid; width: 22px; height: 22px; place-items: center; border-radius: 2px; }
	.issue-icon.error { color: var(--sb-error); background: color-mix(in srgb, var(--sb-error) 12%, transparent); }
	.issue-icon.warning { color: var(--sb-warning); background: color-mix(in srgb, var(--sb-warning) 12%, transparent); }
	.issue-icon.stale { color: var(--sb-stale); background: color-mix(in srgb, var(--sb-stale) 12%, transparent); }
	.issue-icon.info { color: var(--muted-foreground); background: var(--sb-hover); }
	.change { display: grid; grid-template-columns: 72px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 10px 14px; border-bottom: 1px solid var(--inner-line); }
	.change:last-child { border-bottom: 0; }
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
	.health-facts span { display: flex; align-items: center; gap: 6px; }
	.health-facts b { color: var(--foreground); font-weight: 500; }
	.health-head > a, .panel-heading > a { margin-left: auto; color: var(--primary); font: 500 12.5px var(--font-sans); }
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
	.lower-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(360px, 0.9fr); gap: 14px; align-items: start; }
	.lower-grid.single { grid-template-columns: 1fr; }
	.panel-heading { display: flex; min-height: 53px; align-items: center; padding: 0 14px 0 16px; border-bottom: 1px solid var(--border); }
	.panel-heading > div:first-child { margin-right: auto; }
	.panel-heading h2 { margin: 0; font: 600 13.5px var(--font-sans); }
	.panel-heading h2 + span { display: block; margin-top: 2px; color: var(--muted-foreground); font-size: 12px; }
	.status-dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; }
	.warning { background: var(--sb-warning); }
	.error { background: var(--sb-error); }
	.stale { background: var(--sb-stale); }
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
	.warn-text { color: var(--sb-warning) !important; }
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
