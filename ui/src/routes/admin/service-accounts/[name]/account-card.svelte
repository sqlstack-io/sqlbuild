<script lang="ts">
	import BotIcon from '@lucide/svelte/icons/bot';
	import type { Account, ApiToken } from '$lib/api';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import IssuedSecret from '$lib/components/platform/issued-secret.svelte';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { tokenLabel, tokenTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import type { ServiceAccountsState } from './state.svelte';

	let { account, sa }: { account: Account; sa: ServiceAccountsState } = $props();

	const tokens = $derived(sa.tokensOf(account.name));
	const active = $derived(tokens.filter((t) => t.state === 'active').length);
	let creating = $state(false);
	let label = $state('');
	let expires = $state('90');
	let showOld = $state(false);
	const shown = $derived(showOld ? tokens : tokens.filter((t) => t.state === 'active'));

	function create() {
		void sa.createToken(account.name, label.trim(), expires === 'never' ? null : Number(expires)).then(() => {
			if (!sa.error) {
				label = '';
				creating = false;
			}
		});
	}

	function revoke(t: ApiToken) {
		if (!confirm(`Revoke "${t.label}" (${t.prefix}…)? Anything using it stops working straight away.`)) return;
		void sa.revoke(t);
	}

	function disable() {
		if (!confirm(`Disable ${account.name}? Its ${active} active token${active === 1 ? '' : 's'} stop working until you enable it again.`)) return;
		void sa.disable(account);
	}

	function enable() {
		if (!confirm(`Enable ${account.name}? Its tokens that aren't revoked or expired work again.`)) return;
		void sa.enable(account);
	}

	const expiry = (t: ApiToken) =>
		t.revokedAt ? `revoked ${formatAgo(t.revokedAt, app.now)}` : t.expiresAt ? (Date.parse(t.expiresAt) > app.now ? `expires ${formatIn(t.expiresAt, app.now)}` : `expired ${formatAgo(t.expiresAt, app.now)}`) : 'never expires';
</script>

<section class="card" class:off={account.disabled}>
	<header>
		<BotIcon size={16} strokeWidth={1.75} />
		<div class="min-w-0 flex-1">
			<div class="name"><code>{account.name}</code>{#if account.description}<span class="desc">{account.description}</span>{/if}</div>
			<div class="meta">
				{[account.owner ? `Looked after by ${account.owner}` : null, `created ${formatAgo(account.createdAt, app.now)}`, `last used ${account.lastSeenAt ? formatAgo(account.lastSeenAt, app.now) : 'never'}`].filter(Boolean).join(' · ')}
			</div>
		</div>
		<StatusLabel tone={account.disabled ? 'var(--sb-text-faint)' : 'var(--sb-success)'} label={account.disabled ? 'Disabled' : `${active} active token${active === 1 ? '' : 's'}`} muted={account.disabled} />
		<div class="actions">
			{#if !account.disabled}
				<GatedButton requires="token.manage" size="xs" onclick={() => (creating = !creating)}>New token</GatedButton>
				<GatedButton requires="user.manage" size="xs" variant="destructive" busy={sa.busy === account.name} onclick={disable}>Disable</GatedButton>
			{:else}
				<GatedButton requires="user.manage" size="xs" busy={sa.busy === account.name} onclick={enable}>Enable</GatedButton>
			{/if}
		</div>
	</header>

	{#if creating}
		<div class="create">
			<input class="field" placeholder="What it's for, e.g. nightly deploy" bind:value={label} aria-label="Token label" />
			<select class="field" bind:value={expires} aria-label="Expiry">
				<option value="30">Expires in 30 days</option>
				<option value="90">Expires in 90 days</option>
				<option value="365">Expires in 1 year</option>
				<option value="never">Never expires</option>
			</select>
			<GatedButton requires="token.manage" variant="default" size="xs" busy={sa.busy === `create-token:${account.name}`} disabled={!label.trim()} title="Give the token a label" onclick={create}>Create token</GatedButton>
			<button class="link" onclick={() => (creating = false)}>Cancel</button>
		</div>
	{/if}

	{#if sa.issued?.account === account.name}
		<div class="issued"><IssuedSecret label={`Token "${sa.issued.token.token.label}" for ${account.name}`} secret={sa.issued.token.secret} ondismiss={() => sa.dismissIssued()} /></div>
	{/if}
	{#if sa.error?.account === account.name}<p class="err">{sa.error.message}</p>{/if}

	{#if !app.can('token.manage')}
		<p class="muted pad">You need permission to see tokens (token.manage).</p>
	{:else if shown.length}
		<table>
			<tbody>
				{#each shown as t (t.id)}
					<tr class:old={t.state !== 'active'}>
						<td><b>{t.label}</b> <code class="faint" title="The start of the token, for recognising it; the rest is never shown again">{t.prefix}…</code></td>
						<td><RelativeTime value="created {formatAgo(t.createdAt, app.now)} by {t.createdBy.name}" exact={formatStamp(t.createdAt)} /></td>
						<td><RelativeTime value={t.lastUsedAt ? `used ${formatAgo(t.lastUsedAt, app.now)}` : 'never used'} exact={formatStamp(t.lastUsedAt)} /></td>
						<td><RelativeTime value={expiry(t)} exact={formatStamp(t.revokedAt ?? t.expiresAt)} /></td>
						<td><StatusLabel tone={tokenTone[t.state]} label={tokenLabel[t.state]} muted={t.state !== 'active'} /></td>
						<td class="num">
							{#if t.state === 'active'}
								<span class="row-actions">
									<span title="Issues a new secret with the same label and revokes this one"><GatedButton requires="token.manage" size="xs" busy={sa.busy === t.id} disabled={account.disabled} title="Enable the account first" onclick={() => sa.rotate(t)}>Rotate</GatedButton></span>
									<GatedButton requires="token.manage" size="xs" variant="destructive" busy={sa.busy === t.id} onclick={() => revoke(t)}>Revoke</GatedButton>
								</span>
							{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{:else if !creating}
		<p class="muted pad">No active tokens. Create one to let a script or CI job sign in as {account.name}.</p>
	{/if}
	{#if app.can('token.manage') && tokens.length > active}
		<button class="link pad-x" onclick={() => (showOld = !showOld)}>{showOld ? 'Hide' : 'Show'} {tokens.length - active} revoked or expired</button>
	{/if}
</section>

<style>
	.card {
		margin-bottom: 12px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--card);
		font-size: 12.5px;
	}
	.card.off {
		opacity: 0.75;
	}
	header {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 14px;
	}
	header > :global(svg) {
		color: var(--muted-foreground);
	}
	.name {
		display: flex;
		align-items: baseline;
		gap: 10px;
	}
	.name code {
		font-size: 13px;
		font-weight: 600;
	}
	.desc {
		color: var(--muted-foreground);
	}
	.meta {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.actions,
	.row-actions {
		display: inline-flex;
		gap: 6px;
	}
	.create {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 14px;
		border-top: 1px solid var(--border-subtle);
	}
	.field {
		height: 28px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	input.field {
		width: 280px;
	}
	.issued :global(.issued) {
		margin: 4px 14px 10px;
	}
	table {
		width: 100%;
		border-top: 1px solid var(--border-subtle);
		border-collapse: collapse;
	}
	td {
		padding: 6px 14px;
		border-bottom: 1px solid var(--border-subtle);
		white-space: nowrap;
	}
	tr:last-child td {
		border-bottom: 0;
	}
	tr.old {
		opacity: 0.65;
	}
	b {
		font-weight: 500;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.num {
		text-align: right;
	}
	.muted {
		color: var(--muted-foreground);
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.pad {
		padding: 8px 14px 10px;
		border-top: 1px solid var(--border-subtle);
	}
	.link {
		color: var(--primary);
		font-size: 12px;
	}
	.pad-x {
		padding: 0 14px 8px;
	}
	.err {
		padding: 0 14px 8px;
		color: var(--sb-error);
	}
</style>
