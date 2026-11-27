<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { to } from '$lib/paths';
	import type { PlatformEventLink, PlatformEventReaction, PlatformEventRecord } from '$lib/api';
	import { formatAgo, formatDuration, formatStamp } from '$lib/format/time';
	import ViewError from './view-error.svelte';
	import { eventKindLabel, eventLinkHref, messageParts, eventLinkLabel, reactionHref, reactionLabel, reactionTone, severityTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import type { Resource } from '$lib/state/resource.svelte';

	// Problems recorded as platform events, with what they affected and which sensors reacted.
	let {
		events,
		resource,
		empty,
		currentLocationId = null
	}: {
		events: PlatformEventRecord[];
		resource: Resource<unknown>;
		empty: string;
		/** On a location's own page, links back to that location are left out. */
		currentLocationId?: string | null;
	} = $props();

	/** Scoped links switch the selected location before navigating. */
	function follow(scope: { locationId: string }) {
		if (app.locationId !== scope.locationId) app.selectLocation(scope.locationId);
	}

	function followLink(link: PlatformEventLink) {
		if (link.kind !== 'location') follow(link);
	}

	function reactionTitle(r: PlatformEventReaction): string {
		const attempts = r.attempts > 1 ? ` after ${r.attempts} tries` : '';
		const at = r.at ? ` · ${formatStamp(r.at)}` : '';
		const evaluation = r.evaluationId ? `\nevaluation ${r.evaluationId}` : '';
		return `Sensor ${r.triggerName}: ${reactionLabel[r.outcome]}${attempts}${at}${evaluation}`;
	}

	const shownLinks = (e: PlatformEventRecord) => e.links.filter((l) => !(l.kind === 'location' && l.id === currentLocationId));
	// Columns nothing fills are left out, such as reactions where the server doesn't record them.
	const showAffected = $derived(events.some((e) => shownLinks(e).length));
	const showReactions = $derived(events.some((e) => e.reactions.length));

	const payload = (p: Record<string, string>) =>
		Object.entries(p)
			.map(([k, v]) => `${k}: ${v}`)
			.join('\n');
	const tableColumns = $derived<TableColumn[]>([
		{ id: 'problem', label: 'Problem', minWidth: 260, truncate: false },
		{ id: 'when', label: 'When', width: '150px' },
		...(showAffected ? [{ id: 'affected', label: 'Affected', width: '22%', truncate: false }] : []),
		...(showReactions ? [{ id: 'reactions', label: 'Sensors that reacted', width: '22%', truncate: false, title: 'Sensors that react to these problems, e.g. to post a message' }] : [])
	]);
</script>

<DataTable class="problems static" embedded columns={tableColumns} rows={events} key={(e) => e.id} {resource} {empty}>
	{#snippet row(e)}
			{@const parts = messageParts(e.message)}
			<tr>
				<td>
					<div class="cell-name">
						<span class="dot" style:background={e.active ? severityTone[e.severity] : 'var(--sb-text-faint)'} title={e.severity}></span>
						<div class="min-w-0">
							<strong>{eventKindLabel[e.kind]}</strong>{#if e.active}<span class="open">· ongoing</span>{/if}
							<div class="cell-desc wrap">{parts.summary}</div>
							{#if parts.error}<ViewError error={parts.error} />{/if}
							{#if Object.keys(e.payload).length}
								<details>
									<summary>Details</summary>
									<pre>{`event ${e.kind} · ${e.id}\n${payload(e.payload)}`}</pre>
								</details>
							{/if}
						</div>
					</div>
				</td>
				<td class="nowrap">
					<time title={formatStamp(e.occurredAt)}>{formatAgo(e.occurredAt, app.now)}</time>
					<div class="cell-desc">
						{#if e.active}still open{:else if e.resolvedAt}fixed after {formatDuration(e.occurredAt, e.resolvedAt, app.now)}{:else}fixed{/if}
					</div>
				</td>
				{#if showAffected}
				<td>
					{#each shownLinks(e) as link (eventLinkHref(link))}
						<a class="link stacked" href={to(eventLinkHref(link))} onclick={() => followLink(link)}>{eventLinkLabel(link, app.bootstrap?.locations)}</a>
					{:else}
						<span class="muted">—</span>
					{/each}
				</td>
				{/if}
				{#if showReactions}
				<td>
					{#each e.reactions as r (r.triggerId)}
						<a class="link stacked reaction" href={to(reactionHref(r))} onclick={() => follow(r)} title={reactionTitle(r)}>
							<span class="dot" style:background={reactionTone[r.outcome]}></span>{r.triggerName}
							<span class="muted">{reactionLabel[r.outcome]}</span>
						</a>
					{:else}
						<span class="muted" title="No sensor reacts to this kind of problem">—</span>
					{/each}
				</td>
				{/if}
			</tr>
	{/snippet}
</DataTable>

<style>
	:global(.sb-list.problems tbody tr) {
		cursor: default;
	}
	:global(.problems) td {
		vertical-align: top;
	}
	.dot {
		width: 8px;
		height: 8px;
		flex: 0 0 8px;
		margin-top: 5px;
		border-radius: 50%;
	}
	.cell-name {
		align-items: flex-start !important;
	}
	.wrap {
		white-space: normal !important;
	}
	.open {
		margin-left: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.nowrap {
		white-space: nowrap;
	}
	time {
		color: var(--muted-foreground);
	}
	.stacked {
		display: block;
		white-space: nowrap;
	}
	.reaction {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.reaction .dot {
		margin-top: 0;
	}
	details {
		margin-top: 2px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	summary {
		cursor: pointer;
	}
	pre {
		margin-top: 4px;
		padding: 6px 8px;
		border-radius: 2px;
		background: var(--sb-inset);
		font-family: var(--font-mono);
		font-size: 11.5px;
		white-space: pre-wrap;
	}
</style>
