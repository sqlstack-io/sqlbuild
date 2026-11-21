<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import LockIcon from '@lucide/svelte/icons/lock';
	import type { Account, RoleAssignment, RoleDefinition } from '$lib/api';
	import Button from '$lib/components/ui/button/button.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import AssignmentList from '$lib/components/admin/assignment-list.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { to } from '$lib/paths';
	import { personHref } from '$lib/platform/access';
	import { accountSourceLabel, errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { toggleAccount } from './api';

	// A person at a glance beside the list: their roles and enable/disable; their full page keeps effective access.
	type Props = { account: Account; assignments: RoleAssignment[]; rolesByLocation: Map<string, RoleDefinition[]>; onchange: () => void; onclose: () => void };
	let { account, assignments, rolesByLocation, onchange, onclose }: Props = $props();

	let busy = $state(false);
	let error = $state<string | null>(null);
	async function toggle() {
		busy = true;
		error = null;
		try {
			if (await toggleAccount(account)) onchange();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<aside class="panel sb-scroll" aria-label="{account.displayName}" data-testid="person-panel">
	<header>
		<div class="min-w-0">
			<div class="title">{account.displayName}</div>
			<div class="muted"><code>{account.name}</code> · {account.source ? accountSourceLabel[account.source] : 'no password'}</div>
		</div>
		<Button variant="ghost" size="icon-sm" aria-label="Close" onclick={onclose}><XIcon size={15} /></Button>
	</header>
	<div class="status">
		<StatusLabel tone={account.disabled ? 'var(--sb-text-faint)' : 'var(--sb-success)'} label={account.disabled ? 'Disabled' : 'Active'} muted={account.disabled} />
		<span class="muted" title={formatStamp(account.lastSeenAt)}>Last active {account.lastSeenAt ? formatAgo(account.lastSeenAt, app.now) : 'never'}</span>
		<span class="ml-auto">
			{#if account.name === app.user?.name}
				<span class="muted">You</span>
			{:else}
				<GatedButton requires="user.manage" size="sm" variant={account.disabled ? 'outline' : 'destructive'} {busy} onclick={toggle}>{account.disabled ? 'Enable' : 'Disable'}</GatedButton>
			{/if}
		</span>
	</div>
	{#if error}<p class="err">{error}</p>{/if}
	<section>
		<h3>Roles</h3>
		{#if app.can('role.assign')}
			<AssignmentList principal={account} {assignments} {rolesByLocation} {onchange} />
		{:else}
			<p class="muted"><LockIcon size={12} class="inline" /> No access (role.assign)</p>
		{/if}
	</section>
	<a class="full" href={to(personHref(account))}>Open their page: effective access and activity →</a>
</aside>

<style>
	.panel {
		display: flex;
		width: 400px;
		min-height: 0;
		flex-shrink: 0;
		flex-direction: column;
		gap: 14px;
		overflow-y: auto;
		padding: 14px 18px 24px;
		border-left: 1px solid var(--border);
		background: var(--card);
	}
	header {
		display: flex;
		align-items: flex-start;
		gap: 8px;
	}
	header > div {
		flex: 1;
	}
	.title {
		font-family: var(--font-display);
		font-size: 16px;
		font-weight: 600;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	h3 {
		margin: 0 0 8px;
		font-size: 12px;
		font-weight: 600;
		letter-spacing: 0.02em;
		text-transform: uppercase;
		color: var(--muted-foreground);
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.err {
		margin: 0;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.full {
		color: var(--primary);
		font-size: 12.5px;
	}
</style>
