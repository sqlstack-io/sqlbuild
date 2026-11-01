<script lang="ts">
	import GitCompareIcon from '@lucide/svelte/icons/git-compare';
	import type { CatalogNodeDetail } from '$lib/api';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import BuildSource from '$lib/components/build-area/build-source.svelte';
	import OutcomeStrip from '$lib/components/build-area/outcome-strip.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeLabel, checkOutcomeTone, nodeOutcomeTone, staleReasonDetail, staleReasonLabel } from '$lib/status';
	import { formatCredits, formatMs, formatNumber, kindLine, relationName, runScopePhaseLabel } from '$lib/build-area/labels';
	import { driftLabel, driftOf, type Tab } from './utils';

	let { node, onTab, onSql }: { node: CatalogNodeDetail; onTab: (tab: Tab) => void; onSql: (mode: 'raw' | 'compiled' | 'drift') => void } = $props();

	let sqlMode = $state<'raw' | 'compiled'>('raw');
	const drift = $derived(driftOf(node));
	const source = $derived(node.kind === 'source');
	const unique = $derived(node.sql?.raw.match(/unique_key\s+([^,\n]+)/)?.[1] ?? null);
	const freshnessTone = $derived(!node.sourceFreshness ? 'var(--sb-text-faint)' : node.sourceFreshness.status === 'pass' ? 'var(--sb-success)' : node.sourceFreshness.status === 'warn' ? 'var(--sb-warning)' : 'var(--sb-error)');
	const minutes = (s: number | null) => (s === null ? '—' : s >= 3600 && s % 3600 === 0 ? `${s / 3600}h` : `${Math.round(s / 60)}m`);
</script>

<div class="overview">
	<main class="overview-main">
		{#if source}
			<section class="panel">
				<div class="status-top">
					<StatusLabel tone={freshnessTone} label={node.sourceFreshness ? `Freshness ${node.sourceFreshness.status}` : 'No freshness check'} />
					{#if node.sourceFreshness}<span class="muted" title={formatStamp(node.sourceFreshness.loadedAt)}>newest data loaded {formatAgo(node.sourceFreshness.loadedAt, app.now)}</span>{/if}
				</div>
				{#if node.sourceFreshness}
					<div class="stat-grid">
						<div><span>Warn after</span><b>{minutes(node.sourceFreshness.warnAfterSeconds)}</b></div>
						<div><span>Error after</span><b>{minutes(node.sourceFreshness.errorAfterSeconds)}</b></div>
						<div><span>Checked</span><b>{formatAgo(node.sourceFreshness.checkedAt, app.now)}</b></div>
						<div><span>Checks</span><b>{node.checks.total ? `${node.checks.passing}/${node.checks.total}` : '—'}</b></div>
					</div>
				{/if}
			</section>
		{:else}
			<section class="panel">
				<div class="status-top">
					{#if node.lastOutcome}<StatusLabel tone={nodeOutcomeTone[node.lastOutcome]} label="Last build {node.lastOutcome}" />{:else}<StatusLabel tone="var(--sb-text-faint)" label="Not built" muted />{/if}
					<span class="muted">Last built: <BuildSource build={node.lastBuild} /></span>
				</div>
				<div class="stat-grid" class:five={!!node.cost}>
					<div><span>Rows</span><b>{formatNumber(node.lastRows)}</b>{#if node.rowsDelta}<em class:up={node.rowsDelta > 0}>{node.rowsDelta > 0 ? '+' : ''}{formatNumber(node.rowsDelta)}</em>{/if}</div>
					<div><span>Duration</span><b>{formatMs(node.lastDurationMs)}</b></div>
					<div><span>Checks</span><b>{node.checks.total ? `${node.checks.passing}/${node.checks.total}` : '—'}</b></div>
					<div><span>Builds · 40</span><b>{node.recentOutcomes.filter((o) => o === 'failed').length} failed</b></div>
					{#if node.cost}<div><span>Cost per build</span><b>{formatCredits(node.cost.perBuild)}</b><em>{formatCredits(node.cost.last24h)} / 24h</em></div>{/if}
				</div>
				{#if node.recentOutcomes.length}
					<div class="strip"><OutcomeStrip outcomes={node.recentOutcomes} tones={nodeOutcomeTone} caption="Last {node.recentOutcomes.length} builds" /></div>
				{/if}
				{#each node.stale as r, i (i)}
					<button class="drift-hint {r.reason}" onclick={() => (r.reason === 'behind' ? onTab('lineage') : node.sql ? onSql('drift') : onTab('history'))}>
						<GitCompareIcon size={14} />
						<span><b>Stale · {staleReasonLabel(r)}</b> · {staleReasonDetail(r)}</span>
						<em>{r.reason === 'behind' ? 'View lineage →' : 'View drift →'}</em>
					</button>
				{/each}
			</section>
		{/if}

		{#if node.description}
			<section class="section">
				<h3>Description</h3>
				<p class="description">{node.description}</p>
			</section>
		{/if}

		{#if node.sql}
			<section class="section">
				<CodePreview file="{node.key}.sql" code={node.sql.raw} compiled={node.sql.compiled} bind:mode={sqlMode} maxLines={16} onOpen={() => onSql(sqlMode)} />
			</section>
		{:else if node.code}
			<section class="section">
				<CodePreview file={node.code.file} code={node.code.source} language="text" maxLines={16} onOpen={() => onTab('sql')} />
			</section>
		{/if}

		{#if node.checkResults.length}
			<section class="section">
				<div class="section-head"><h3>Checks</h3><button class="link" onclick={() => onTab('checks')}>{node.checks.passing} of {node.checks.total} passing →</button></div>
				<div class="rows">
					{#each node.checkResults as c (c.id)}
						{@const o = c.latest?.outcome ?? 'skipped'}
						<a class="row" href="/checks/{c.id}"><StatusLabel tone={checkOutcomeTone[o]} label={checkOutcomeLabel[o]} /><code>{c.name}</code><span class="muted ml-auto">{runScopePhaseLabel[c.runScopePhase]} · {c.severity}</span></a>
					{/each}
				</div>
			</section>
		{/if}

		{#if node.columns.length}
			<section class="section">
				<div class="section-head"><h3>Columns</h3><button class="link" onclick={() => onTab('columns')}>All {node.columns.length} →</button></div>
				<div class="rows">
					{#each node.columns.slice(0, 5) as c (c.name)}
						<div class="row"><code>{c.name}</code><span class="muted">{c.description}</span><code class="muted ml-auto">{c.type}</code></div>
					{/each}
				</div>
			</section>
		{/if}

		<section class="section deps">
			<div>
				<h3>Upstream <span>{node.upstream.length}</span></h3>
				{#each node.upstream as u (u)}<a href="/catalog/{u}">{u}</a>{:else}<span class="muted">None</span>{/each}
			</div>
			<div>
				<h3>Downstream <span>{node.downstream.length}</span></h3>
				{#each node.downstream as d (d)}<a href="/catalog/{d}">{d}</a>{:else}<span class="muted">None</span>{/each}
			</div>
		</section>
	</main>

	<aside class="overview-rail">
		<section>
			<h3>Definition</h3>
			<dl>
				<div><dt>Kind</dt><dd>{kindLine(node)}</dd></div>
				{#if unique}<div><dt>Unique key</dt><dd class="mono">{unique}</dd></div>{/if}
				<div><dt>Group</dt><dd>{node.group}</dd></div>
				<div><dt>Owner</dt><dd>{node.owner.join(', ') || '—'}</dd></div>
				<div><dt>Path</dt><dd class="mono" title={node.path}>{node.path}</dd></div>
				<div><dt>Relation</dt><dd class="mono" title={relationName(node.relation)}>{relationName(node.relation)}</dd></div>
				{#if node.lastBuild}<div><dt>Build version</dt><dd class="mono" title="Fingerprint of the last build in this location">{node.lastBuild.fingerprint.slice(0, 12)}</dd></div>{/if}
				{#if drift}<div><dt>Drift</dt><dd style:color="var(--sb-stale)">{driftLabel[drift]}</dd></div>{/if}
			</dl>
		</section>
		{#if node.tags.length}
			<section>
				<h3>Tags</h3>
				<div class="flex flex-wrap gap-1">{#each node.tags as t (t)}<span class="sb-tag">{t}</span>{/each}</div>
			</section>
		{/if}
		<section>
			<h3>Jobs</h3>
			{#each node.jobs as j (j.key)}
				<a class="job-row" href="/jobs/{j.key}"><span>{j.key}</span><small>{j.kind === 'group' ? 'group' : 'job'}{j.triggerIds.length ? ` · ${j.triggerIds.length} ${j.triggerIds.length === 1 ? 'trigger' : 'triggers'}` : ' · manual'}</small></a>
			{:else}
				<p class="muted">No job selects this node.</p>
			{/each}
		</section>
	</aside>
</div>

<style>
	.overview { display: grid; grid-template-columns: minmax(0, 1fr) 320px; min-height: 100%; }
	.overview-main { min-width: 0; padding: 20px 24px 48px; }
	.panel { padding: 16px; border: 1px solid var(--panel-border); box-shadow: var(--panel-shadow); border-radius: 2px; background: var(--card); }
	.status-top { display: flex; align-items: center; gap: 14px; font-size: 12.5px; }
	.stat-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; margin-top: 14px; }
	.stat-grid.five { grid-template-columns: repeat(5, 1fr); }
	.stat-grid span { display: block; color: var(--muted-foreground); font-size: 12px; }
	.stat-grid b { font-size: 18px; font-weight: 600; font-variant-numeric: tabular-nums; }
	.stat-grid em { margin-left: 6px; color: var(--muted-foreground); font-size: 11.5px; font-style: normal; }
	.stat-grid em.up { color: var(--sb-success); }
	.strip { margin-top: 16px; }
	.section { margin-top: 24px; }
	.section h3, .overview-rail h3 { margin: 0 0 8px; font-size: 13px; font-weight: 600; }
	.section h3 span { color: var(--muted-foreground); font-weight: 400; }
	.section-head { display: flex; align-items: baseline; margin-bottom: 8px; }
	.section-head h3 { margin: 0; }
	.section-head .link { margin-left: auto; font-size: 12.5px; }
	.link { border: 0; background: transparent; color: var(--primary); }
	.description { max-width: 88ch; margin: 0; color: var(--muted-foreground); font-size: 13px; line-height: 1.55; }
	.rows { border: 1px solid var(--border); border-radius: 2px; }
	.row { display: flex; align-items: center; gap: 14px; padding: 9px 14px; border-bottom: 1px solid var(--border); font-size: 12.5px; }
	a.row:hover { background: var(--sb-hover); }
	.row:last-child { border-bottom: 0; }
	.row code { font: 12px var(--font-mono); }
	.muted { color: var(--muted-foreground); }
	.ml-auto { margin-left: auto; }
	.deps { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; }
	.deps a { display: block; padding: 3px 0; color: var(--primary); font-size: 12.5px; }
	.deps a:hover { text-decoration: underline; }
	.overview-rail { padding: 20px; border-left: 1px solid var(--border); font-size: 12.5px; }
	.overview-rail section + section { margin-top: 22px; }
	.overview-rail dl { margin: 0; }
	.overview-rail dl > div { display: flex; gap: 12px; padding: 5px 0; }
	.overview-rail dt { flex: 0 0 auto; color: var(--muted-foreground); }
	.overview-rail dd { margin: 0 0 0 auto; overflow: hidden; text-align: right; text-overflow: ellipsis; white-space: nowrap; }
	.mono { font: 12px var(--font-mono); }
	.job-row { display: flex; align-items: baseline; padding: 4px 0; }
	.job-row span { color: var(--primary); }
	.job-row small { margin-left: auto; color: var(--muted-foreground); font-size: 12px; }
	.drift-hint { display: flex; width: 100%; align-items: center; gap: 9px; margin-top: 12px; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--sb-stale) 35%, var(--border)); border-radius: 2px; background: transparent; color: var(--foreground); font-size: 12.5px; text-align: left; }
	.drift-hint:hover { background: var(--sb-hover); }
	.drift-hint :global(svg) { flex: 0 0 auto; color: var(--sb-stale); }
	.drift-hint.changed_upstream { border-style: dashed; }
	.drift-hint.behind { border-color: color-mix(in srgb, var(--sb-warning) 35%, var(--border)); }
	.drift-hint.behind :global(svg) { color: var(--sb-warning); }
	.drift-hint em { margin-left: auto; color: var(--primary); font-style: normal; white-space: nowrap; }
</style>
