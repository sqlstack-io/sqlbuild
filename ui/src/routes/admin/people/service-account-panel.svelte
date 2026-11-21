<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import type { BuiltInRole, ServiceAccount, ServiceAccountKey } from '$lib/api';
	import ConfirmDialog from '$lib/components/platform/confirm-dialog.svelte';
	import MenuSelect from '$lib/components/platform/menu-select.svelte';
	import Button, { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { formatIn, formatStamp } from '$lib/format/time';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import RotateKeyDialog from './rotate-key-dialog.svelte';
	import ServiceActivity from './service-activity.svelte';
	import type { ServiceAccountsState } from './service-accounts.svelte';
	import { BUILTIN_ROLES, keyFacts, serviceState, statusLine } from './utils';

	// One service account beside the list. The resting view shows state; actions live in menus and dialogs.
	let { account, sa }: { account: ServiceAccount; sa: ServiceAccountsState } = $props();

	const ROLE_OPTIONS = BUILTIN_ROLES.map((r) => ({ value: r, label: r }));
	const canManage = $derived(app.can('user.manage'));
	const canAssign = $derived(app.can('role.assign'));
	const status = $derived(serviceState(account));

	type Confirm = { title: string; body: string; confirm: string; destructive: boolean; run: () => Promise<unknown> };
	let confirming = $state<Confirm | null>(null);
	let rotating = $state(false);
	let roleError = $state<string | null>(null);
	const dialogOpen = $derived(confirming !== null || rotating);

	function toggle() {
		const keys = account.keys.length === 1 ? 'Its key stops' : `Its ${account.keys.length} keys stop`;
		confirming = account.disabled
			? {
					title: `Enable ${account.name}?`,
					body: `Its ${account.keys.length === 1 ? 'key works' : 'keys work'} again at once. If a key may have leaked, rotate it as well.`,
					confirm: 'Enable',
					destructive: false,
					run: () => app.client.setServiceAccountDisabled(account.name, false).then(() => sa.list.refresh())
				}
			: {
					title: `Disable ${account.name}?`,
					body: `${keys} working at once: anything using ${account.keys.length === 1 ? 'it' : 'them'} is refused until you enable it again.`,
					confirm: 'Disable',
					destructive: true,
					run: () => app.client.setServiceAccountDisabled(account.name, true).then(() => sa.list.refresh())
				};
	}

	function stop(key: ServiceAccountKey) {
		const current = key.expiresAt === null;
		confirming = {
			title: `Stop ${key.prefix}… now?`,
			body: current
				? `Anything using this key is refused from now on, and ${account.name} has no working key until you rotate.`
				: 'Anything still using this old key is refused from now on.',
			confirm: 'Stop key',
			destructive: true,
			run: () => app.client.revokeServiceAccountKey(account.name, key.id).then(() => sa.list.refresh())
		};
	}

	/** Escape closes the panel, but not when it closes a menu or a dialog first. */
	function escape(e: KeyboardEvent) {
		if (e.key !== 'Escape' || e.defaultPrevented || dialogOpen) return;
		if ((e.target as Element | null)?.closest?.('[role="menu"], [role="dialog"]')) return;
		sa.close();
	}

	async function setRole(role: BuiltInRole) {
		roleError = null;
		const apply = () => app.client.setServiceAccountRole(account.name, role).then(() => sa.list.refresh());
		if (role === 'admin') {
			confirming = {
				title: `Make ${account.name} an admin?`,
				body: 'Anyone holding its key can then manage people, roles, locations and the audit log everywhere.',
				confirm: 'Make admin',
				destructive: true,
				run: apply
			};
			return;
		}
		try {
			await apply();
		} catch (e) {
			roleError = errorMessage(e);
		}
	}
</script>

<svelte:window onkeydown={escape} />

<aside class="panel sb-scroll" aria-label={account.name} data-testid="service-account-panel">
	<header>
		<div class="min-w-0 flex-1">
			<div class="title">{account.name}</div>
			{#if account.description}<div class="muted">{account.description}</div>{/if}
			<div class="status" data-testid="service-status"><i class="dot {status}"></i>{statusLine(account, app.now)}</div>
		</div>
		{#if canManage}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger class={buttonVariants({ variant: 'ghost', size: 'icon-sm' })} aria-label="More actions for {account.name}" data-testid="service-menu"><EllipsisIcon size={15} /></DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" class="w-44">
					<DropdownMenu.Item onSelect={toggle}>{account.disabled ? 'Enable…' : 'Disable…'}</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		{/if}
		<Button variant="ghost" size="icon-sm" aria-label="Close" onclick={() => sa.close()}><XIcon size={15} /></Button>
	</header>

	<section>
		<h3>Role</h3>
		{#if canAssign && !account.disabled}
			<MenuSelect value={account.role} options={ROLE_OPTIONS} onchange={setRole} label="Role of {account.name}" testid="service-role" />
			<span class="muted"> everywhere</span>
		{:else}
			<p class="line">{account.role} <span class="muted">everywhere</span></p>
		{/if}
		{#if roleError}<p class="err" role="alert">{roleError}</p>{/if}
	</section>

	<section>
		<h3>API keys</h3>
		<ul class="keys">
			{#each account.keys as k (k.id)}
				<li data-testid="service-key">
					<code title="{k.origin === 'imported' ? 'Imported with the CLI' : 'Created'} {formatStamp(k.createdAt)} by {k.createdBy.displayName}">{k.prefix}…</code>
					<span class="muted facts">{keyFacts(k, app.now)}</span>
					{#if k.expiresAt}<span class="tag" title="Stops {formatStamp(k.expiresAt)}">stops {formatIn(k.expiresAt, app.now)}</span>{/if}
					{#if canManage}
						<DropdownMenu.Root>
							<DropdownMenu.Trigger class="{buttonVariants({ variant: 'ghost', size: 'icon-xs' })} ml-auto" aria-label="Actions for key {k.prefix}" data-testid="key-menu"><EllipsisIcon size={13} /></DropdownMenu.Trigger>
							<DropdownMenu.Content align="end" class="w-40">
								<DropdownMenu.Item variant="destructive" onSelect={() => stop(k)}>Stop now…</DropdownMenu.Item>
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{/if}
				</li>
			{:else}
				<li class="muted">No working key. Rotate to create one.</li>
			{/each}
		</ul>
		{#if canManage}
			<Button size="sm" variant="outline" disabled={account.disabled} title={account.disabled ? 'Enable it first' : undefined} onclick={() => (rotating = true)} data-testid="rotate-key">Rotate key…</Button>
		{/if}
	</section>

	<section>
		<h3>Recent activity</h3>
		<ServiceActivity actor={account.name} />
	</section>
</aside>

<RotateKeyDialog {account} open={rotating} onrotated={() => sa.list.refresh()} onclose={() => (rotating = false)} />
{#if confirming}
	<ConfirmDialog open title={confirming.title} body={confirming.body} confirm={confirming.confirm} destructive={confirming.destructive} onconfirm={confirming.run} onclose={() => (confirming = null)} />
{/if}

<style>
	.panel {
		display: flex;
		width: 440px;
		min-height: 0;
		flex-shrink: 0;
		flex-direction: column;
		gap: 22px;
		overflow-y: auto;
		padding: 14px 18px 24px;
		border-left: 1px solid var(--border);
		background: var(--card);
	}
	header {
		display: flex;
		align-items: flex-start;
		gap: 4px;
	}
	.title {
		font-family: var(--font-display);
		font-size: 16px;
		font-weight: 600;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 7px;
		margin-top: 6px;
		font-size: 12.5px;
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--sb-text-faint);
	}
	.dot.active {
		background: var(--sb-success);
	}
	.dot.rotating {
		background: var(--sb-warning);
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
	.line {
		margin: 0;
		font-size: 13px;
	}
	.keys {
		display: flex;
		flex-direction: column;
		margin: 0 0 12px;
		padding: 0;
		list-style: none;
	}
	.keys li {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: 32px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.keys li:first-child {
		border-top: 1px solid var(--border);
	}
	.facts {
		overflow: hidden;
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.tag {
		flex-shrink: 0;
		padding: 0 5px;
		border: 1px solid color-mix(in srgb, var(--sb-warning) 45%, var(--border));
		border-radius: var(--radius);
		color: var(--sb-warning);
		font-size: 11px;
	}
	code {
		flex-shrink: 0;
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.err {
		margin: 6px 0 0;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
