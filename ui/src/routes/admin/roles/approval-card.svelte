<script lang="ts">
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import PermissionName from '$lib/components/admin/permission-name.svelte';
	import type { RoleApproval } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { personHref, roleHref } from '$lib/platform/access';
	import { locationName } from '$lib/platform/labels';
	import { isRisky } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';
	import type { RolesState } from './state.svelte';
	import { lineDiff, plural } from './utils';

	let { approval, rs }: { approval: RoleApproval; rs: RolesState } = $props();

	const locations = $derived(app.bootstrap?.locations ?? []);
	const where = $derived(locationName(locations, approval.locationId));
	const diff = $derived(lineDiff(approval.definitionBefore, approval.definitionAfter));
	const own = $derived(approval.gitAuthorAccount !== null && approval.gitAuthorAccount === app.user?.name);
	const risky = $derived(approval.impact.some((i) => isRisky(i.action) && i.principals.length));

	let note = $state('');
	const needsNote = $derived(!note.trim());
</script>

<article class="approval">
	<header>
		<a class="role" href={roleHref(approval.role)}>{approval.role}</a>
		<span class="muted" title="Snapshot generation {approval.snapshotGeneration}">
			{where} · version {approval.snapshotGeneration} · commit <code>{approval.gitRevision}</code> by {approval.gitAuthor} ·
			<span title={formatStamp(approval.proposedAt)}>deployed {formatAgo(approval.proposedAt, app.now)}</span>
		</span>
		{#if risky}<span class="risky"><TriangleAlertIcon size={12} /> Includes risky actions</span>{/if}
	</header>

	<section>
		<h3>If approved</h3>
		<ul class="impact">
			{#each approval.impact as i (i.action + i.selector)}
				<li class:hot={isRisky(i.action) && i.principals.length > 0}>
					<span class="count">+{plural(i.principals.length, 'person', 'people')}</span>
					can <PermissionName action={i.action} risk />
					on <code>{i.selector === '*' ? 'every node' : i.selector}</code> in {where}
					{#if i.principals.length}
						<span class="who">
							({#each i.principals as p, n (p.name)}{#if n},&nbsp;{/if}<a href={personHref(p)}>{p.displayName}</a>{/each})
						</span>
					{:else}
						<span class="who">(everyone with the role already has it)</span>
					{/if}
				</li>
			{:else}
				<li class="muted">Nobody holds this role here yet, so nobody gains anything today.</li>
			{/each}
		</ul>
	</section>

	{#if approval.removed.length}
		<section>
			<h3>Already applied in the same deploy</h3>
			<ul class="applied">
				{#each approval.removed as g (g.nodes.join('|') + g.permissions.join('|'))}
					{#each g.permissions as p (p)}
						<li>No longer <PermissionName action={p} /> on <code>{g.nodes.join(', ')}</code> <span class="muted">(taking actions away needs no approval)</span></li>
					{/each}
				{/each}
			</ul>
		</section>
	{/if}

	<section>
		<h3>Definition</h3>
		<div class="diff" aria-label="Proposed definition change">
			{#each diff as line, n (n)}
				<div class="line {line.op}"><span>{line.op === 'add' ? '+' : line.op === 'del' ? '-' : ' '}</span>{line.text}</div>
			{/each}
		</div>
	</section>

	<div class="decide">
		<textarea class="field" rows="2" placeholder="Why approve or reject? Required; recorded in the audit log." bind:value={note} aria-label="Comment"></textarea>
		<div class="buttons">
			<GatedButton
				requires="role.assign"
				variant="default"
				busy={rs.busy === approval.id}
				disabled={needsNote || own}
				title={own ? `You committed ${approval.gitRevision}; another admin has to approve it` : needsNote ? 'Add a comment first' : undefined}
				onclick={() => rs.decide(approval, 'approve', note)}>Approve</GatedButton
			>
			<GatedButton requires="role.assign" variant="destructive" busy={rs.busy === approval.id} disabled={needsNote} title={needsNote ? 'Add a comment first' : undefined} onclick={() => rs.decide(approval, 'reject', note)}
				>Reject</GatedButton
			>
		</div>
	</div>
	{#if own}<p class="own">You committed this change, so you can't approve it. Ask another admin; you can still reject it.</p>{/if}
	{#if rs.errorOf(approval.id)}<p class="err">{rs.errorOf(approval.id)}</p>{/if}
</article>

<style>
	.approval {
		margin-bottom: 18px;
		padding: 12px 14px;
		border: 1px solid color-mix(in srgb, var(--sb-warning) 35%, var(--border));
		border-radius: 2px;
		background: var(--card);
		font-size: 12.5px;
	}
	header {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 10px;
		margin-bottom: 6px;
	}
	.role {
		color: var(--primary);
		font-size: 13.5px;
		font-weight: 600;
	}
	.risky {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--sb-warning);
		font-size: 12px;
	}
	h3 {
		margin: 12px 0 5px;
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 500;
	}
	.impact,
	.applied {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.impact li {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 3px 6px;
		padding: 3px 8px;
		border-left: 2px solid var(--border);
	}
	.impact li.hot {
		border-left-color: var(--sb-warning);
		background: color-mix(in srgb, var(--sb-warning) 7%, transparent);
	}
	.count {
		font-weight: 600;
	}
	.who {
		color: var(--muted-foreground);
	}
	.who a {
		color: var(--primary);
	}
	.applied li {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 3px 6px;
		color: var(--muted-foreground);
	}
	.diff {
		padding: 6px 0;
		overflow-x: auto;
		border-radius: 2px;
		background: var(--sb-inset);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.line {
		padding: 1px 10px;
		white-space: pre;
	}
	.line span {
		display: inline-block;
		width: 16px;
		color: var(--sb-text-faint);
	}
	.line.same {
		color: var(--muted-foreground);
	}
	.line.add {
		background: color-mix(in srgb, var(--sb-success) 12%, transparent);
	}
	.line.del {
		background: color-mix(in srgb, var(--sb-error) 12%, transparent);
	}
	.decide {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		margin-top: 12px;
	}
	.field {
		flex: 1;
		padding: 6px 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
		resize: vertical;
	}
	.buttons {
		display: flex;
		gap: 8px;
	}
	.own {
		margin-top: 6px;
		color: var(--sb-warning);
		font-size: 12px;
	}
	.err {
		margin-top: 6px;
		color: var(--sb-error);
	}
	.muted {
		color: var(--muted-foreground);
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
</style>
