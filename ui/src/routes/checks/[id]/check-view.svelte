<script lang="ts">
	import LockIcon from '@lucide/svelte/icons/lock';
	import type { CheckEvidence } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import OutcomeStrip from '$lib/components/build-area/outcome-strip.svelte';
	import RunPanel from '$lib/run/run-panel.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeLabel, checkOutcomeTone } from '$lib/status';
	import { attachmentLabel, evaluationModeLabel, runScopePhaseLabel, shortRunId, thresholdLabel } from '$lib/build-area/labels';
	import { errorMessage } from '$lib/platform/labels';
	import { checkValue, outcomeOf } from '../utils';

	let { id }: { id: string } = $props();

	const res = createResource((client, scope) => client.getCheck(scope, id), ['checks', 'runs']);
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

	let runOpen = $state(false);
</script>

<AppTopbar title="Checks">
	{#if c}
		<GatedButton requires="run.adhoc" variant="default" onclick={() => (runOpen = true)}>▷ Run check</GatedButton>
	{/if}
</AppTopbar>

<RunPanel bind:open={runOpen} verb="audit" selection={c?.node ? [c.node] : []} />

<div class="flex min-h-0 flex-1 flex-col">
	<div class="head">
		<a href="/checks" class="back">← Checks</a>
		<code class="title">{c?.name ?? id}</code>
		{#if c}
			<StatusLabel tone={checkOutcomeTone[outcome]} label={checkOutcomeLabel[outcome]} />
			<span class="faint">{c.severity} severity · {runScopePhaseLabel[c.runScopePhase].toLowerCase()} · {evaluationModeLabel[c.evaluationMode].toLowerCase()}</span>
		{/if}
	</div>
	{#if res.error}<div class="msg err">Couldn't load this check: {res.error}</div>{:else if res.data === null}<div class="msg">There's no check <code>{id}</code> in the version deployed to {app.location?.name}.</div>{/if}

	{#if c}
		<div class="flex min-h-0 flex-1">
			<div class="main sb-scroll">
				{#if c.description}<p class="description">{c.description}</p>{/if}
				<Panel title="Recent results" note="Last {c.history.length} times it ran">
					<OutcomeStrip outcomes={[...c.history].reverse().map((e) => e.outcome)} tones={checkOutcomeTone} caption="Last {c.history.length} results" />
				</Panel>

				<Panel title="History" note="Read back from the warehouse, so results of builds from the command line show too">
					<table class="sb-list">
						<thead>
							<tr><th>Checked</th><th>Outcome</th><th class="num">{c.evaluationMode === 'measurement' ? 'Measured' : 'Violations'}</th><th>Run</th><th>Evidence</th></tr>
						</thead>
						<tbody>
							{#each c.history as e (e.id)}
								<tr class:selected={evidenceFor === e.id}>
									<td><span title={formatStamp(e.evaluatedAt)}>{formatAgo(e.evaluatedAt, app.now)}</span></td>
									<td>
										<StatusLabel tone={checkOutcomeTone[e.outcome]} label={checkOutcomeLabel[e.outcome]} />
										{#if e.executionError}<div class="cell-desc err">{e.executionError}</div>{/if}
									</td>
									<td class="num">{checkValue(e, c.evaluationMode)}</td>
									<td>
										{#if e.runId}<a class="mono link" href="/runs/{e.runId}">{e.runId}</a>
										{:else}<span class="faint" title="Built by sqb outside this server, e.g. from a laptop or CI (fingerprint run {e.fingerprintRunId})">from the command line · <code>{shortRunId(e.fingerprintRunId)}</code></span>{/if}
									</td>
									<td>
										{#if e.evidenceCount}
											<GatedButton requires="view.evidence" size="xs" busy={evidenceBusy === e.id} onclick={() => showEvidence(e.id)}>{e.evidenceCount}{e.evidenceTruncated ? '+' : ''} rows</GatedButton>
										{:else}<span class="faint">—</span>{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</Panel>

				{#if evidenceError}<div class="msg err">{evidenceError}</div>{/if}
				{#if evidence}
					<Panel title="Evidence" note="Sample rows stored with the result · this read is recorded in the audit log">
						{#snippet actions()}<button class="link" onclick={() => { evidence = null; evidenceFor = null; }}>Close</button>{/snippet}
						<table class="sb-list evidence">
							<thead><tr>{#each evidence.columns as col (col)}<th>{col}</th>{/each}</tr></thead>
							<tbody>
								{#each evidence.rows as row, i (i)}
									<tr>{#each row as v, j (j)}<td class="mono">{v === null ? 'NULL' : v}</td>{/each}</tr>
								{:else}
									<tr><td colspan={evidence.columns.length || 1} class="faint">No rows were stored.</td></tr>
								{/each}
							</tbody>
						</table>
						{#if evidence.truncated}<p class="faint note">More rows matched than were stored.</p>{/if}
					</Panel>
				{/if}

				<Panel title="SQL that ran" note="The latest time">
					{#if c.sql}
						<CodePreview file="{c.id}.sql" code={c.sql} maxLines={30} />
					{:else}
						<p class="faint locked"><LockIcon size={13} /> You don't have permission to see SQL (view.sql)</p>
					{/if}
				</Panel>
			</div>
			<aside class="rail sb-scroll">
				<h3>Definition</h3>
				<KeyValues
					rows={[
						{ label: 'Kind', value: c.kind === 'audit' ? `audit · ${c.definition ?? 'custom'}` : c.kind.replace('_', ' ') },
						{ label: 'Attached to', value: c.node ? `${c.node}${c.column ? `.${c.column}` : ''}` : attachmentLabel[c.attachment], mono: !!c.node },
						{ label: 'Target type', value: attachmentLabel[c.attachment] },
						{ label: 'Severity', value: c.severity },
						{ label: 'Checks', value: runScopePhaseLabel[c.runScopePhase], title: c.runScopePhase },
						{ label: 'Result type', value: evaluationModeLabel[c.evaluationMode], title: c.evaluationMode },
						{ label: 'Thresholds', value: c.thresholds.length ? c.thresholds.map(thresholdLabel).join(' · ') : '—' }
					]}
				/>
				{#if c.node}
					<h3>Node</h3>
					<p><a class="link mono" href="/catalog/{c.node}?tab=checks">{c.node}</a></p>
				{/if}
				{#if c.latest}
					<h3>Latest</h3>
					<KeyValues
						rows={[
							{ label: 'Outcome', value: checkOutcomeLabel[c.latest.outcome], tone: checkOutcomeTone[c.latest.outcome] },
							{ label: 'Result', value: checkValue(c.latest, c.evaluationMode) },
							{ label: 'Last checked', value: `${formatAgo(c.latest.evaluatedAt, app.now)} · ${formatStamp(c.latest.evaluatedAt)}` },
							{ label: 'Run', value: c.latest.runId ?? `from the command line · ${shortRunId(c.latest.fingerprintRunId)}`, mono: true }
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
	.main {
		min-width: 0;
		flex: 1;
		overflow: auto;
		padding: 18px 24px;
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
		border: 0;
		background: transparent;
		color: var(--primary);
		font-size: 12.5px;
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
	.evidence td {
		font-size: 12px;
	}
</style>
