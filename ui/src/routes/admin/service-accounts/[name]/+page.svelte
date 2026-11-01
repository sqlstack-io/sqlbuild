<script lang="ts">
	import { page } from '$app/state';
	import { tick } from 'svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import AccessChecker from '$lib/components/admin/access-checker.svelte';
	import AssignmentList from '$lib/components/admin/assignment-list.svelte';
	import AuditMini from '$lib/components/admin/audit-mini.svelte';
	import EffectiveAccessGrid from '$lib/components/admin/effective-access-grid.svelte';
	import type { CheckInput } from '$lib/platform/access';
	import { createAccessDirectory } from '$lib/state/access-directory.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import AccountCard from './account-card.svelte';
	import { createServiceAccountsState } from './state.svelte';

	const name = $derived(page.params.name ?? '');
	const sa = createServiceAccountsState();
	const d = createAccessDirectory();
	const account = $derived(sa.services.find((a) => a.name === name) ?? null);
	const allowed = $derived(app.can('user.manage') || app.can('token.manage'));
	const canExplain = $derived(app.can('role.assign') || app.can('audit.view'));
	const access = createResource((client) => (canExplain && name ? client.effectiveAccess(name) : Promise.resolve(null)));
	const isNew = $derived(!!account && !d.assignmentsOf(name).length && !sa.tokensOf(name).length && !!d.assignments.data);

	let why = $state<CheckInput | null>(null);
	let checkerEl = $state<HTMLElement | null>(null);
	async function showWhy(input: CheckInput) {
		why = { ...input };
		await tick();
		checkerEl?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	function changed() {
		d.refresh();
		sa.refresh();
		access.refresh();
	}
</script>

<AppTopbar title="Service accounts">
	<Button variant="outline" size="sm" onclick={changed}>↻ Refresh</Button>
</AppTopbar>

{#if !allowed}
	<NoAccess what="service accounts" requires={['user.manage', 'token.manage']} />
{:else}
	<div class="flex min-h-0 flex-1 flex-col">
		<div class="head">
			<a href="/admin/service-accounts" class="back">← Service accounts</a>
			<code class="title">{name}</code>
		</div>
		{#if sa.accounts.data && !account}
			<p class="muted px-6 py-4">No service account named <code>{name}</code>.</p>
		{:else if account}
			<div class="min-h-0 flex-1 overflow-auto px-6 pt-5 pb-10">
				<div class="max-w-[1300px]">
					{#if isNew}
						<p class="created">It can't do anything yet: create a token below, then give it a role.</p>
					{/if}
					<Panel title="Tokens" note="Secrets are shown once, when created or rotated">
						<AccountCard {account} {sa} />
					</Panel>
					<Panel title="Roles">
						{#if app.can('role.assign')}
							<AssignmentList principal={account} assignments={d.assignments.data ?? []} rolesByLocation={d.rolesByLocation} onchange={changed} />
						{:else}
							<p class="muted">Seeing roles requires “Give roles and approve role changes” (role.assign).</p>
						{/if}
					</Panel>
					{#if canExplain}
						<Panel title="Effective access" note="Every action in every location, after adding up all its roles">
							{#if access.data}
								<EffectiveAccessGrid access={access.data} onwhy={showWhy} />
							{:else if access.error}
								<p class="err">Couldn't load: {access.error}</p>
							{:else}
								<p class="muted">Loading…</p>
							{/if}
						</Panel>
						<div bind:this={checkerEl}>
							<Panel title="Why?" note="Pick a mark above, or check any action">
								<AccessChecker principals={[account]} initial={why ?? { principal: account.name }} lockPrincipal />
							</Panel>
						</div>
					{/if}
					<Panel title="Recent activity">
						<AuditMini actor={account.name} />
					</Panel>
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
	.back {
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.back:hover {
		color: var(--foreground);
	}
	.title {
		font-family: var(--font-mono);
		font-size: 15px;
		font-weight: 600;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.err {
		color: var(--sb-error);
		font-size: 12.5px;
	}
	code {
		font-family: var(--font-mono);
	}
	.created {
		margin-bottom: 16px;
		padding: 8px 12px;
		border: 1px solid color-mix(in srgb, var(--sb-success) 40%, var(--border));
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-success) 7%, var(--background));
		font-size: 12.5px;
	}
</style>
