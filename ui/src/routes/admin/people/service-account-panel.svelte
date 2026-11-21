<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import type { ServiceAccount, ServiceAccountKey } from '$lib/api';
	import AuditMini from '$lib/components/admin/audit-mini.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import IssuedSecret from '$lib/components/platform/issued-secret.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import type { ServiceAccountsState } from './service-accounts.svelte';
	import { OVERLAP_CHOICES, SERVICE_STATE_LABEL, serviceState } from './utils';

	// One service account beside the list: the key just issued, its keys, rotate, disable, and what it did lately.
	let { account, sa }: { account: ServiceAccount; sa: ServiceAccountsState } = $props();

	const status = $derived(serviceState(account));
	const issued = $derived(sa.issued?.name === account.name ? sa.issued : null);
	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);
	let rotating = $state(false);
	let overlap = $state<number | null>(60);

	async function act(key: string, action: () => Promise<unknown>) {
		busy = key;
		error = null;
		try {
			await action();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = null;
		}
	}

	function rotate() {
		void act('rotate', async () => {
			const result = await app.client.rotateServiceAccountKey(account.name, { overlapMinutes: overlap });
			rotating = false;
			sa.showIssued(result, 'rotated');
		});
	}

	function stop(key: ServiceAccountKey) {
		if (!confirm(`Stop the old key ${key.prefix}… now? Anything still using it gets “401 Unauthorized” from now on.`)) return;
		void act(key.id, async () => {
			await app.client.revokeServiceAccountKey(account.name, key.id);
			sa.list.refresh();
		});
	}

	function toggle() {
		const keys = account.keys.length === 1 ? 'its key stops' : `its ${account.keys.length} keys stop`;
		const ask = account.disabled
			? `Enable ${account.name}? Its ${account.keys.length === 1 ? 'key works' : 'keys work'} again.`
			: `Disable ${account.name}? ${keys[0].toUpperCase()}${keys.slice(1)} working at once: anything using ${account.keys.length === 1 ? 'it' : 'them'} gets “401 Unauthorized” until it's enabled again.`;
		if (!confirm(ask)) return;
		void act('toggle', async () => {
			await app.client.setServiceAccountDisabled(account.name, !account.disabled);
			sa.list.refresh();
		});
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !issued && sa.close()} />

<aside class="panel sb-scroll" aria-label={account.name} data-testid="service-account-panel">
	<header>
		<div class="min-w-0">
			<div class="title">{account.name}</div>
			<div class="muted">{account.description ?? 'Service account'}</div>
		</div>
		<Button variant="ghost" size="icon-sm" aria-label="Close" onclick={() => sa.close()}><XIcon size={15} /></Button>
	</header>

	{#if issued}
		<IssuedSecret
			flush
			label={issued.reason === 'created' ? `${account.name} is ready: here is its API key` : `New key for ${account.name}`}
			secret={issued.secret}
			note={issued.reason === 'created'
				? "Store it now, for example in your deploy's secret store: it won't be shown again. SQLBuild keeps only a hash. If it's lost, rotate the key."
				: "Store it now: it won't be shown again. Update whatever uses the old key before its overlap ends."}
			ondismiss={() => sa.dismissIssued()}
		/>
	{/if}

	<div class="status">
		<StatusLabel tone={status === 'rotating' ? 'var(--sb-warning)' : status === 'active' ? 'var(--sb-success)' : 'var(--sb-text-faint)'} label={SERVICE_STATE_LABEL[status]} muted={account.disabled} />
		<span class="muted" title={formatStamp(account.lastUsedAt)}>Last used {account.lastUsedAt ? formatAgo(account.lastUsedAt, app.now) : 'never'}</span>
		<span class="ml-auto">
			<GatedButton requires="user.manage" size="sm" variant={account.disabled ? 'outline' : 'destructive'} busy={busy === 'toggle'} onclick={toggle}>{account.disabled ? 'Enable' : 'Disable'}</GatedButton>
		</span>
	</div>
	{#if account.disabled}<p class="note">Disabled: its keys are refused. Enabling lets the same keys work again; rotate as well if one may have leaked.</p>{/if}
	{#if error}<p class="err" role="alert">{error}</p>{/if}

	<section>
		<h3>Role</h3>
		<p class="line"><strong>{account.role}</strong> <span class="muted">everywhere · change it in the list</span></p>
	</section>

	<section>
		<h3>API keys</h3>
		<ul class="keys">
			{#each account.keys as k (k.id)}
				<li class:old={k.expiresAt}>
					<div class="key-head">
						<code>{k.prefix}…</code>
						{#if k.expiresAt}
							<span class="badge warn" title={formatStamp(k.expiresAt)}>Old · stops {formatIn(k.expiresAt, app.now)}</span>
						{:else}
							<span class="badge">Current</span>
						{/if}
						{#if k.expiresAt}
							<span class="ml-auto"><GatedButton requires="user.manage" size="sm" variant="ghost" busy={busy === k.id} onclick={() => stop(k)}>Stop now</GatedButton></span>
						{/if}
					</div>
					<div class="muted small">
						{k.origin === 'imported' ? 'Imported with the CLI' : 'Issued'}
						<span title={formatStamp(k.createdAt)}>{formatAgo(k.createdAt, app.now)}</span> by {k.createdBy.displayName}
						· last used <span title={formatStamp(k.lastUsedAt)}>{k.lastUsedAt ? formatAgo(k.lastUsedAt, app.now) : 'never'}</span>
						{#if k.expiresAt}· works until {formatStamp(k.expiresAt)}{/if}
					</div>
				</li>
			{:else}
				<li class="muted">No working key. Rotate to issue one.</li>
			{/each}
		</ul>
		{#if rotating}
			<div class="rotate" data-testid="rotate-form">
				<label>
					Keep the old key working for
					<select bind:value={overlap}>
						{#each OVERLAP_CHOICES as c (c.minutes)}<option value={c.minutes}>{c.label}</option>{/each}
					</select>
				</label>
				<p class="muted small">
					{#if overlap}
						Both keys work until {formatStamp(new Date(app.now + overlap * 60_000).toISOString())}, so you can update the script first.{#if account.keys.length > 1} The key already in its overlap stops now: only one old key is kept.{/if}
					{:else}
						Anything still using the current key gets “401 Unauthorized” at once. Choose this if the key may have leaked.
					{/if}
				</p>
				<div class="actions">
					<Button size="sm" disabled={busy === 'rotate'} onclick={rotate}>{busy === 'rotate' ? 'Rotating…' : 'Rotate and issue new key'}</Button>
					<Button size="sm" variant="ghost" onclick={() => (rotating = false)}>Cancel</Button>
				</div>
			</div>
		{:else}
			<GatedButton requires="user.manage" size="sm" disabled={account.disabled} title={account.disabled ? 'Enable it first' : undefined} onclick={() => (rotating = true)}><RotateCwIcon size={13} /> Rotate key</GatedButton>
		{/if}
	</section>

	<section>
		<h3>Recent activity</h3>
		<AuditMini actor={account.name} limit={6} />
	</section>
</aside>

<style>
	.panel {
		display: flex;
		width: 420px;
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
	.small {
		font-size: 12px;
	}
	.line {
		margin: 0;
		font-size: 13px;
	}
	.note {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.keys {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin: 0 0 10px;
		padding: 0;
		list-style: none;
	}
	.keys li {
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.keys li.old {
		border-color: color-mix(in srgb, var(--sb-warning) 45%, var(--border));
	}
	.key-head {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 3px;
	}
	.badge {
		padding: 0 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.badge.warn {
		border-color: color-mix(in srgb, var(--sb-warning) 45%, var(--border));
		color: var(--sb-warning);
	}
	.rotate {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.rotate label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 12.5px;
	}
	.rotate p {
		margin: 0;
	}
	select {
		padding: 6px 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md, 6px);
		background: transparent;
		font-size: 13px;
	}
	.actions {
		display: flex;
		gap: 8px;
	}
	.err {
		margin: 0;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
