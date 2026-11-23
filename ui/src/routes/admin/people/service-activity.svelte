<script lang="ts">
	import type { AuditEntry } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { to } from '$lib/paths';
	import { auditActionLabel } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	// What a service account did lately, as plain lines; the audit log has the rest.
	let { actor }: { actor: string } = $props();
	const LIMIT = 5;

	const entries = createResource((client) => (app.can('audit.view') ? client.listAuditEntries({ actor, limit: LIMIT }) : Promise.resolve([] as AuditEntry[])));
	const auditHref = $derived(to(`/admin/audit?actor=${encodeURIComponent(actor)}`));
</script>

{#if !app.can('audit.view')}
	<p class="muted">Seeing activity needs “See the audit log” (audit.view).</p>
{:else if entries.data?.length}
	<ul data-testid="service-activity">
		{#each entries.data as e (e.id)}
			<li>
				<span class="when" title={formatStamp(e.at)}>{formatAgo(e.at, app.now)}</span>
				<a class="what" href={auditHref} title={e.summary || e.action}>
					{auditActionLabel(e.action)}{#if e.scope.subject}{' '}<span class="subject">{e.scope.subject}</span>{/if}
				</a>
				{#if e.outcome === 'denied'}<span class="tag">denied</span>{/if}
			</li>
		{/each}
	</ul>
	<a class="all" href={auditHref}>All activity →</a>
{:else if entries.data}
	<p class="muted">Nothing yet.</p>
{/if}

<style>
	ul {
		display: flex;
		flex-direction: column;
		gap: 7px;
		margin: 0 0 10px;
		padding: 0;
		list-style: none;
		font-size: 12.5px;
	}
	li {
		display: flex;
		align-items: baseline;
		gap: 10px;
		min-width: 0;
	}
	.when {
		width: 52px;
		flex-shrink: 0;
		color: var(--muted-foreground);
	}
	.what {
		overflow: hidden;
		min-width: 0;
		color: var(--foreground);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.what:hover {
		text-decoration: underline;
		text-underline-offset: 2px;
	}
	.subject {
		color: var(--muted-foreground);
	}
	.tag {
		flex-shrink: 0;
		padding: 0 5px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		color: var(--muted-foreground);
		font-size: 11px;
	}
	.muted {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.all {
		color: var(--primary);
		font-size: 12.5px;
	}
</style>
