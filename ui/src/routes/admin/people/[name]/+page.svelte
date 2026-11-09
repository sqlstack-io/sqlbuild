<script lang="ts">
	import { page } from '$app/state';
	import { tick } from 'svelte';
	import UserIcon from '@lucide/svelte/icons/user';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import AccessChecker from '$lib/components/admin/access-checker.svelte';
	import AssignmentList from '$lib/components/admin/assignment-list.svelte';
	import AuditMini from '$lib/components/admin/audit-mini.svelte';
	import EffectiveAccessGrid from '$lib/components/admin/effective-access-grid.svelte';
	import type { Account } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import type { CheckInput } from '$lib/platform/access';
	import { accountSourceLabel, errorMessage } from '$lib/platform/labels';
	import { createAccessDirectory } from '$lib/state/access-directory.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const name = $derived(page.params.name ?? '');
	const d = createAccessDirectory();
	const account = $derived(d.accounts.data?.find((a) => a.name === name && a.kind === 'user') ?? null);
	const canSee = $derived(app.can('user.manage') || app.can('role.assign'));
	const canExplain = $derived(app.can('role.assign') || app.can('audit.view') || name === app.user?.name);

	const access = createResource((client) => (canExplain && name ? client.effectiveAccess(name) : Promise.resolve(null)));

	let why = $state<CheckInput | null>(null);
	let checkerEl = $state<HTMLElement | null>(null);
	async function showWhy(input: CheckInput) {
		why = { ...input };
		await tick();
		checkerEl?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	function changed() {
		d.refresh();
		access.refresh();
	}

	let busy = $state(false);
	let error = $state<string | null>(null);
	async function toggle(a: Account) {
		const sessions = a.activeSessions === 1 ? 'their 1 signed-in session ends' : `their ${a.activeSessions} signed-in sessions end`;
		const ask = a.disabled ? `Enable ${a.displayName} (${a.name})? They can sign in again.` : `Disable ${a.displayName} (${a.name})? They can't sign in any more, in the browser or with sqb login, and ${sessions} now.`;
		if (!confirm(ask)) return;
		busy = true;
		error = null;
		try {
			await (a.disabled ? app.client.enableAccount(a.name) : app.client.disableAccount(a.name));
			changed();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<AppTopbar title="People">
	<Button variant="outline" size="sm" onclick={changed}>↻ Refresh</Button>
</AppTopbar>

{#if !canSee}
	<NoAccess what="people" requires={['user.manage', 'role.assign']} />
{:else}
	<div class="flex min-h-0 flex-1 flex-col">
		<div class="head">
			<a href="/admin/people" class="back">← People</a>
			<UserIcon size={16} strokeWidth={1.75} />
			<span class="title">{account?.displayName ?? name}</span>
			<code class="muted">{name}</code>
			{#if account}
				<StatusLabel tone={account.disabled ? 'var(--sb-text-faint)' : 'var(--sb-success)'} label={account.disabled ? 'Disabled' : 'Active'} muted={account.disabled} />
				<div class="ml-auto flex items-center gap-2">
					{#if account.name === app.user?.name}
						<span class="muted">This is you</span>
					{:else}
						<GatedButton requires="user.manage" size="sm" variant={account.disabled ? 'outline' : 'destructive'} {busy} onclick={() => toggle(account)}>{account.disabled ? 'Enable' : 'Disable'}</GatedButton>
					{/if}
				</div>
			{/if}
		</div>
		{#if error}<p class="err px-6 pt-3">{error}</p>{/if}

		{#if d.accounts.data && !account}
			<p class="muted px-6 py-4">No person named <code>{name}</code>. They appear here after they first sign in.</p>
		{:else if account}
			<div class="min-h-0 flex-1 overflow-auto px-6 pt-5 pb-10">
				<div class="columns">
					<div class="min-w-0">
						<Panel title="Roles" note="What they've been given; SSO, policy-file and built-in roles are locked here">
							{#if app.can('role.assign')}
								<AssignmentList principal={account} assignments={d.assignments.data ?? []} rolesByLocation={d.rolesByLocation} onchange={changed} />
							{:else}
								<p class="muted">Seeing roles requires “Give roles and approve role changes” (role.assign).</p>
							{/if}
						</Panel>

						<Panel title="Effective access" note="Every action in every location, after adding up all their roles">
							{#if !canExplain}
								<p class="muted">Requires “Give roles” (role.assign) or “See the audit log” (audit.view).</p>
							{:else if access.data}
								<EffectiveAccessGrid access={access.data} onwhy={showWhy} />
							{:else if access.error}
								<p class="err">Couldn't load: {access.error}</p>
							{:else}
								<p class="muted">Loading…</p>
							{/if}
						</Panel>

						{#if canExplain}
							<div bind:this={checkerEl}>
								<Panel title="Why?" note="Pick a mark above, or check any action">
									<AccessChecker principals={[account]} initial={why ?? { principal: account.name }} lockPrincipal />
								</Panel>
							</div>
						{/if}
					</div>

					<div class="min-w-0">
						<Panel title="Details">
							<KeyValues
								rows={[
									{ label: 'Signs in with', value: account.source ? accountSourceLabel[account.source] : '—' },
									{ label: 'Email', value: account.email ?? '—' },
									{ label: 'SSO groups', value: account.ssoGroups.join(', ') || '—' },
									{ label: 'Last active', value: account.lastSeenAt ? formatAgo(account.lastSeenAt, app.now) : 'Never', title: formatStamp(account.lastSeenAt) },
									{ label: 'Signed in on', value: account.activeSessions === 0 ? 'No devices' : account.activeSessions === 1 ? '1 device' : `${account.activeSessions} devices` },
									{ label: 'First seen', value: formatAgo(account.createdAt, app.now), title: formatStamp(account.createdAt) }
								]}
							/>
						</Panel>
						<Panel title="Recent activity">
							<AuditMini actor={account.name} />
						</Panel>
					</div>
				</div>
			</div>
		{:else}
			<p class="muted px-6 py-4">Loading…</p>
		{/if}
	</div>
{/if}

<style>
	.head {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 14px 24px;
		border-bottom: 1px solid var(--border);
	}
	.head :global(svg) {
		color: var(--muted-foreground);
	}
	.back {
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.back:hover {
		color: var(--foreground);
	}
	.title {
		font-family: var(--font-display);
		font-size: 17px;
		font-weight: 600;
	}
	.columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 320px;
		gap: 0 36px;
		max-width: 1400px;
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
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
