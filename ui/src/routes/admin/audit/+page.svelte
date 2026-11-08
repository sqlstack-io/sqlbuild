<script lang="ts">
	import EyeIcon from '@lucide/svelte/icons/eye';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import { page } from '$app/state';
	import CopyButton from '$lib/components/copy-button.svelte';
	import type { AuditOutcome } from '$lib/api';
	import { checkHref, personHref } from '$lib/platform/access';
	import { auditActionLabel } from '$lib/platform/permission-catalog';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { formatAssignmentScope } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	let actor = $state(page.url.searchParams.get('actor') ?? '');
	let action = $state('');
	let outcome = $state<AuditOutcome | 'any'>('any');
	let evidenceOnly = $state(false);

	const entries = createResource((client) => {
		const query = { actor: actor || undefined, action: action || undefined, outcome: outcome === 'any' ? undefined : outcome, evidenceOnly, limit: 500 };
		return app.can('audit.view') ? client.listAuditEntries(query) : Promise.resolve([]);
	});
	// Unfiltered once, for the actor list.
	const everyone = createResource((client) => (app.can('audit.view') ? client.listAuditEntries({ limit: 500 }) : Promise.resolve([])));

	const actors = $derived([...new Set((everyone.data ?? []).map((e) => e.actor.name))].sort());
	const actions = $derived([...new Set((everyone.data ?? []).map((e) => e.action))].sort());
	const rows = $derived(entries.data ?? []);
	const locations = $derived(app.bootstrap?.locations ?? []);
	// The person page and the access checker both read people.
	const linksPeople = $derived(app.client.supports('listAccounts'));
</script>

<AppTopbar title="Audit log">
	<Button
		variant="outline"
		size="sm"
		onclick={() => {
			entries.refresh();
			everyone.refresh();
		}}>↻ Refresh</Button
	>
</AppTopbar>

{#if !app.can('audit.view')}
	<NoAccess what="the audit log" requires="audit.view" />
{:else}
	<div class="toolbar">
		<select class="field" bind:value={actor} aria-label="Actor">
			<option value="">Anyone</option>
			{#each actors as a (a)}<option value={a}>{a}</option>{/each}
		</select>
		<select class="field" bind:value={action} aria-label="Action">
			<option value="">Any action</option>
			{#each actions as a (a)}<option value={a}>{auditActionLabel(a)} ({a})</option>{/each}
		</select>
		<SegmentedFilter
			bind:value={outcome}
			options={[
				{ value: 'any', label: 'All' },
				{ value: 'allowed', label: 'Allowed' },
				{ value: 'denied', label: 'Denied' }
			]}
		/>
		<label class="check">
			<input type="checkbox" class="sb-check" bind:checked={evidenceOnly} />
			Only reads of sample rows
		</label>
		<span class="total">{rows.length} entries</span>
	</div>

	<div class="sb-list-wrap">
		<table class="sb-list static">
			<thead>
				<tr>
					<th>When</th>
					<th>Who</th>
					<th title="The action the request needed">Action</th>
					<th>Where</th>
					<th>Result</th>
					<th>Allowed by role</th>
					<th>What happened</th>
				</tr>
			</thead>
			<tbody>
				{#each rows as e (e.id)}
					<tr>
						<td><RelativeTime value={formatAgo(e.at, app.now)} exact={formatStamp(e.at)} /></td>
						<td>
							{#if e.actor.kind === 'user' && linksPeople}<a class="link" href={personHref(e.actor)}>{e.actor.name}</a>{:else}{e.actor.name}{/if}
							<div class="cell-desc">{e.ip}</div>
						</td>
						<td>
							<span title={e.action}>{auditActionLabel(e.action)}</span>
							{#if e.evidenceRead}<span class="evidence" title="They looked at stored sample or failing rows (evidence)"><EyeIcon size={12} /> read sample rows</span>{/if}
						</td>
						<td class="muted where">
							{formatAssignmentScope(e.scope, locations)}
							{#if e.scope.subject}<div class="cell-desc clip" title={e.scope.subject}><code>{e.scope.subject}</code></div>{/if}
						</td>
						<td>
							<StatusLabel tone={e.outcome === 'denied' ? 'var(--sb-error)' : 'var(--sb-success)'} label={e.outcome === 'denied' ? 'Denied' : 'Allowed'} />
							{#if e.decisionId}
								<div class="decision">
									<code class="clip" title="Decision ID {e.decisionId}, as shown in the error message">{e.decisionId}</code>
									<CopyButton value={e.decisionId} label="Copy decision ID" />
									{#if linksPeople}<a class="link" href={checkHref({ decision: e.decisionId })}>Check this</a>{/if}
								</div>
							{/if}
						</td>
						<td class="muted">{e.grantingRole ?? '—'}</td>
						<td class="detail">
							{e.summary}
							<div class="cell-desc clip" title="Request {e.requestId}">
								{#if e.before !== null || e.after !== null}<code>{e.before ?? '∅'} → {e.after ?? '∅'}</code> ·{/if}
								<code>{e.requestId}</code>
							</div>
						</td>
					</tr>
				{:else}
					<LoadRow colspan={7} resource={entries} empty="Nothing matches these filters." />
				{/each}
			</tbody>
		</table>
	</div>
{/if}

<style>
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	.toolbar {
		display: flex;
		height: 50px;
		flex: 0 0 50px;
		align-items: center;
		gap: 8px;
		padding: 0 20px;
		border-bottom: 1px solid var(--border);
	}
	.field {
		height: 30px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.check {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		margin-left: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.total {
		margin-left: auto;
		font-size: 12px;
		font-weight: 500;
	}
	.evidence {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		margin-left: 6px;
		color: var(--sb-warning);
		font-size: 11.5px;
	}
	.detail {
		max-width: 320px;
	}
	.where .clip {
		max-width: 200px;
	}
	.decision .clip {
		max-width: 110px;
	}
	.detail .clip {
		max-width: 300px;
	}
	.clip {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.decision {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-top: 2px;
		font-size: 12px;
		white-space: nowrap;
	}
</style>
