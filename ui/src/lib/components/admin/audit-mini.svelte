<script lang="ts">
	import { to } from '$lib/paths';
	import type { AuditEntry } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkHref } from '$lib/platform/access';
	import { auditActionLabel } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	// The latest audit entries by one account, with denials linking to the checker.
	let { actor, limit = 8 }: { actor: string; limit?: number } = $props();

	const entries = createResource((client) => (app.can('audit.view') ? client.listAuditEntries({ actor, limit }) : Promise.resolve([] as AuditEntry[])));
</script>

{#if !app.can('audit.view')}
	<p class="muted">Requires “See the audit log” (audit.view).</p>
{:else if entries.data?.length}
	<ul>
		{#each entries.data as e (e.id)}
			<li>
				<span class="when" title={formatStamp(e.at)}>{formatAgo(e.at, app.now)}</span>
				<span class:denied={e.outcome === 'denied'}>
					{e.outcome === 'denied' ? 'Denied: ' : ''}<span title={e.action}>{auditActionLabel(e.action)}</span>{#if e.scope.subject}&nbsp;· <code>{e.scope.subject}</code>{/if}
				</span>
				{#if e.decisionId && app.client.supports('lookupDecision')}
					<a href={to(checkHref({ decision: e.decisionId }))} title="Check this request again">Check this</a>
				{/if}
			</li>
		{/each}
	</ul>
	<a class="all" href={to(`/admin/audit?actor=${encodeURIComponent(actor)}`)}>All their activity →</a>
{:else if entries.data}
	<p class="muted">Nothing recorded yet.</p>
{:else if entries.error}
	<p class="muted">Couldn't load: {entries.error}</p>
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	ul {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 12.5px;
	}
	li {
		display: flex;
		flex-wrap: wrap;
		gap: 2px 8px;
	}
	.when {
		min-width: 56px;
		color: var(--sb-text-faint);
	}
	.denied {
		color: var(--sb-error);
	}
	a {
		color: var(--primary);
		font-size: 12px;
	}
	.all {
		display: inline-block;
		margin-top: 10px;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	code {
		min-width: 0;
		font-family: var(--font-mono);
		font-size: 11.5px;
		overflow-wrap: anywhere;
	}
</style>
