<script lang="ts">
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import PlayIcon from '@lucide/svelte/icons/play';
	import { goto } from '$app/navigation';
	import LockIcon from '@lucide/svelte/icons/lock';
	import type { CheckEvidence, CheckSql } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import OutcomeStrip from '$lib/components/build-area/outcome-strip.svelte';
	import { planHref } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeLabel, checkOutcomeTone } from '$lib/status';
	import { attachmentLabel, evaluationModeHelp, evaluationModeLabel, runScopePhaseHelp, runScopePhaseLabel, shortRunId, thresholdLabel } from '$lib/build-area/labels';
	import { errorMessage } from '$lib/platform/labels';
	import { cellText, checkValue, kindLabel, outcomeOf } from '../utils';

	let { id }: { id: string } = $props();

	const res = createResource((client, scope) => client.getCheck(scope, id), ['checks', 'runs', 'node_status']);
	const c = $derived(res.data ?? null);
	const outcome = $derived(c ? outcomeOf(c) : 'skipped');

	let evidence = $state<CheckEvidence | null>(null);
	let evidenceFor = $state<string | null>(null);
	let evidenceBusy = $state<string | null>(null);
	let evidenceError = $state<string | null>(null);

	async function showEvidence(evaluationId: string) {
		const scope = app.scope;
		if (!scope) return;
		evidenceBusy = evaluationId;
		evidenceError = null;
		try {
			evidence = await app.client.getCheckEvidence(scope, id, evaluationId);
			evidenceFor = evaluationId;
		} catch (error) {
			evidenceError = errorMessage(error);
		} finally {
			evidenceBusy = null;
		}
	}

	let sql = $state<CheckSql | null>(null);
	let sqlBusy = $state(false);
	let sqlError = $state<string | null>(null);

	async function showSql() {
		const scope = app.scope;
		if (!scope) return;
		sqlBusy = true;
		sqlError = null;
		try {
			sql = await app.client.getCheckSql(scope, id);
		} catch (error) {
			sqlError = errorMessage(error);
		} finally {
			sqlBusy = false;
		}
	}

	const historyColumns = $derived<TableColumn[]>([
		{ id: 'checked', label: 'Checked', width: '120px' },
		{ id: 'outcome', label: 'Outcome', width: '130px' },
		{ id: 'value', label: c?.evaluationMode === 'measurement' ? 'Measured' : 'Violations', width: '110px', align: 'right' },
		{ id: 'run', label: 'Run', minWidth: 160 },
		{ id: 'evidence', label: 'Evidence', width: '120px', truncate: false }
	]);
</script>

<AppTopbar title="Checks" subject="{c?.name ?? id} · Check">
	{#if c && app.client.supports('launchAdhocRun')}
		<GatedButton size="bar" requires="run.adhoc" variant="default" onclick={() => goto(to(planHref(c?.node ? [c.node] : [], 'audit')))}><PlayIcon class="size-3.5 fill-current stroke-none" />Run check</GatedButton>
	{/if}
</AppTopbar>


<div class="flex min-h-0 flex-1 flex-col">
	<div class="head">
		<a href={to(`/checks`)} class="back">← Checks</a>
		<code class="title">{c?.name ?? id}</code>
		{#if c}
			<StatusLabel tone={checkOutcomeTone[outcome]} label={checkOutcomeLabel[outcome]} />
			<span class="faint" title="{c.severity} severity · {runScopePhaseHelp[c.runScopePhase]} · {evaluationModeHelp[c.evaluationMode]}">{c.severity} · {runScopePhaseLabel[c.runScopePhase].toLowerCase()} · {evaluationModeLabel[c.evaluationMode].toLowerCase()}</span>
		{/if}
	</div>
	{#if res.error}<div class="msg err">Couldn't load this check: {res.error}</div>{:else if res.data === null}<div class="msg">There's no check <code>{id}</code> in the version deployed to {app.location?.name}.</div>{/if}

	{#if c}
		<div class="flex min-h-0 flex-1">
			<PageBody padding="18px 24px">
				{#if c.description}<p class="description">{c.description}</p>{/if}
				<Panel title="Recent results" note="Last {c.history.length}">
					<OutcomeStrip outcomes={[...c.history].reverse().map((e) => e.outcome)} tones={checkOutcomeTone} caption="Last {c.history.length} results" />
				</Panel>

				<Panel title="History" help="Read back from the warehouse, so results of builds from the command line show too.">
					<DataTable class="static" embedded columns={historyColumns} rows={c.history} key={(e) => e.id} empty="No results recorded yet.">
						{#snippet row(e)}
								<tr class:selected={evidenceFor === e.id}>
									<td><span title={formatStamp(e.evaluatedAt)}>{formatAgo(e.evaluatedAt, app.now)}</span></td>
									<td>
										<StatusLabel tone={checkOutcomeTone[e.outcome]} label={checkOutcomeLabel[e.outcome]} />
									</td>
									<td class="num">{checkValue(e, c.evaluationMode)}</td>
									<td>
										{#if e.runId}<a class="mono link" href={to(`/runs/${e.runId}`)}>{e.runId}</a>
										{:else}<span class="faint" title="The sqb run that evaluated it: {e.fingerprintRunId}">sqb run <code>{shortRunId(e.fingerprintRunId)}</code></span>{/if}
									</td>
									<td>
										{#if e.evidenceCount}
											<GatedButton requires="view.evidence" size="sm" busy={evidenceBusy === e.id} onclick={() => showEvidence(e.id)}>{e.evidenceCount}{e.evidenceTruncated ? '+' : ''} rows</GatedButton>
										{:else}<span class="faint">—</span>{/if}
									</td>
								</tr>
						{/snippet}
					</DataTable>
				</Panel>

				{#if evidenceError}<div class="msg err">{evidenceError}</div>{/if}
				{#if evidence}
					<Panel title="Evidence" note="Sample rows" audited>
						{#snippet actions()}<Button variant="link" size="inline" onclick={() => { evidence = null; evidenceFor = null; }}>Close</Button>{/snippet}
						<DataTable class="static evidence" embedded columns={evidence.columns.map((col, i) => ({ id: `${i}`, label: col, minWidth: 100 }))} rows={evidence.rows.map((cells, i) => ({ cells, i }))} key={(r) => r.i} empty="No rows were stored.">
							{#snippet row(r)}
								<tr>{#each r.cells as v, j (j)}<td class="mono">{cellText(v)}</td>{/each}</tr>
							{/snippet}
						</DataTable>
						{#if evidence.truncated}<p class="faint note" title="More rows matched than were stored">Truncated</p>{/if}
					</Panel>
				{/if}

				<Panel title="SQL that ran" note="Last run" audited>
					{#if sql}
						{#if sql.executionError}<div class="msg err">{sql.executionError}</div>{/if}
						{#if sql.evidenceError}<div class="msg err">Evidence couldn't be collected: {sql.evidenceError}</div>{/if}
						{#each [sql.executedSql, sql.measurementSql, sql.evidenceSql].filter((text, i, all) => text && all.indexOf(text) === i) as text, i (i)}
							<CodePreview file="{c.id}.sql" code={text ?? ''} maxLines={30} />
						{:else}
							<p class="faint">No SQL was stored with the latest result.</p>
						{/each}
					{:else if !app.can('view.sql')}
						<p class="faint locked"><LockIcon size={13} /> No access (view.sql)</p>
					{:else}
						<GatedButton requires="view.sql" size="sm" busy={sqlBusy} onclick={showSql} hint="Includes execution errors · viewing is audited">Show SQL</GatedButton>
						{#if sqlError}<div class="msg err">{sqlError}</div>{/if}
					{/if}
				</Panel>
			</PageBody>
			<aside class="rail sb-scroll">
				<h3>Definition</h3>
				<KeyValues
					rows={[
						{ label: 'Kind', value: kindLabel(c) },
						{ label: 'Attached to', value: c.node ? `${c.node}${c.column ? `.${c.column}` : ''}` : attachmentLabel[c.attachment], mono: !!c.node },
						{ label: 'Target type', value: attachmentLabel[c.attachment] },
						{ label: 'Severity', value: c.severity },
						{ label: 'Scope', value: runScopePhaseLabel[c.runScopePhase], title: c.runScopePhase, help: runScopePhaseHelp[c.runScopePhase] },
						{ label: 'Result type', value: evaluationModeLabel[c.evaluationMode], title: c.evaluationMode, help: evaluationModeHelp[c.evaluationMode] },
						{ label: 'Thresholds', value: c.thresholds.length ? c.thresholds.map(thresholdLabel).join(' · ') : '—' }
					]}
				/>
				{#if c.node}
					<h3>Node</h3>
					<p>{#if app.client.supports('getCatalogNode')}<a class="link mono" href={to(`/catalog/${c.node}?tab=checks`)}>{c.node}</a>{:else}<code>{c.node}</code>{/if}</p>
				{/if}
				{#if c.latest}
					<h3>Latest</h3>
					<KeyValues
						rows={[
							{ label: 'Outcome', value: checkOutcomeLabel[c.latest.outcome], tone: checkOutcomeTone[c.latest.outcome] },
							{ label: 'Result', value: checkValue(c.latest, c.evaluationMode) },
							{ label: 'Last checked', value: `${formatAgo(c.latest.evaluatedAt, app.now)} · ${formatStamp(c.latest.evaluatedAt)}` },
							{ label: 'Run', value: c.latest.runId ?? `sqb run ${shortRunId(c.latest.fingerprintRunId)}`, title: c.latest.fingerprintRunId, mono: true }
						]}
					/>
				{/if}
			</aside>
		</div>
	{/if}
</div>

<style>
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 24px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.back {
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.title {
		font: 600 14px var(--font-mono);
	}
	.faint {
		color: var(--muted-foreground);
	}
	.msg {
		padding: 14px 24px;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.err {
		color: var(--sb-error);
	}
	.description {
		margin: 0 0 18px;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.rail {
		width: 340px;
		flex-shrink: 0;
		overflow: auto;
		padding: 18px 20px;
		border-left: 1px solid var(--border);
		font-size: 12.5px;
	}
	h3 {
		margin: 20px 0 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	h3:first-child {
		margin-top: 0;
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.link {
		color: var(--primary);
	}
	tr.selected td {
		background: var(--sb-hover);
	}
	.locked {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 12.5px;
	}
	.note {
		margin-top: 8px;
		font-size: 12px;
	}
	:global(.evidence) td {
		font-size: 12px;
	}
</style>
