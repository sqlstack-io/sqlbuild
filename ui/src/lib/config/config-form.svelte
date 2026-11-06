<script lang="ts">
	// Schema-driven typed config editor. Renders real controls (text / int / bool /
	// enum / secret) from a provider's field schema (the Pydantic -> JSON-schema
	// truth), with inline defaults, descriptions, required markers and live
	// validation. Opt-in raw YAML escape hatch, two-way bound. Reused by the
	// Providers page (durable config) and the Run panel (per-run overrides).
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import LockIcon from '@lucide/svelte/icons/lock';

	export type Field = {
		key: string;
		label?: string;
		type: 'string' | 'int' | 'bool' | 'enum' | 'secret';
		required?: boolean;
		default?: string | number | boolean;
		enum?: string[];
		min?: number;
		max?: number;
		description?: string;
		secretSource?: string; // e.g. 'env: SNOWFLAKE_PASSWORD'
	};

	let {
		schema,
		values = $bindable({}),
		/** baseline to diff against → per-run override highlighting (Run panel) */
		baseline = undefined,
		readOnly = false,
		/** bindable: how many fields are invalid (parent gates Save/Run on this) */
		errorCount = $bindable(0)
	}: {
		schema: Field[];
		values?: Record<string, unknown>;
		baseline?: Record<string, unknown>;
		readOnly?: boolean;
		errorCount?: number;
	} = $props();

	let rawOpen = $state(false);

	// effective value (typed value or its default)
	function eff(f: Field) {
		return values[f.key] ?? f.default;
	}
	function isOverride(f: Field) {
		return baseline !== undefined && values[f.key] !== undefined && values[f.key] !== baseline[f.key];
	}
	function errorFor(f: Field): string | null {
		const v = values[f.key];
		const present = v !== undefined && v !== '' && v !== null;
		// a secret sourced from env / a secret store is satisfied even without an inline value
		if (f.type === 'secret' && f.secretSource) return null;
		if (f.required && !present && f.default === undefined) return 'required';
		if (f.type === 'int' && present) {
			const n = Number(v);
			if (Number.isNaN(n)) return 'must be a number';
			if (f.min !== undefined && n < f.min) return `min ${f.min}`;
			if (f.max !== undefined && n > f.max) return `max ${f.max}`;
		}
		if (f.type === 'enum' && present && f.enum && !f.enum.includes(String(v))) return 'invalid option';
		return null;
	}

	const errors = $derived(schema.map((f) => ({ key: f.key, err: errorFor(f) })).filter((e) => e.err));
	const overrideCount = $derived(baseline ? schema.filter((f) => isOverride(f)).length : 0);
	$effect(() => {
		errorCount = errors.length;
	});

	function set(f: Field, v: unknown) {
		if (readOnly) return;
		values = { ...values, [f.key]: v };
	}

	// generated YAML (source-of-truth string; secrets masked)
	const yaml = $derived(
		schema
			.map((f) => {
				const v = eff(f);
				if (v === undefined) return null;
				const shown = f.type === 'secret' ? `<${f.secretSource ?? 'secret'}>` : String(v);
				return `${f.key}: ${shown}`;
			})
			.filter(Boolean)
			.join('\n')
	);
</script>

<div class="flex flex-col">
	{#if baseline && overrideCount > 0}
		<div class="text-primary mb-2 text-[11px]">▲ {overrideCount} per-run override{overrideCount > 1 ? 's' : ''}</div>
	{/if}

	<div class="flex flex-col">
		{#each schema as f (f.key)}
			{@const err = errorFor(f)}
			{@const ov = isOverride(f)}
			<div class="flex items-start gap-4 border-b border-[var(--border-subtle)] py-2.5 last:border-b-0">
				<!-- label -->
				<div class="w-[160px] shrink-0 pt-1">
					<div class="flex items-center gap-1 text-[12px]">
						{f.label ?? f.key}
						{#if f.required}<span class="text-[var(--sb-error)]">*</span>{/if}
						{#if ov}<span class="bg-primary ml-1 h-1.5 w-1.5 rounded-full" title="overridden for this run"></span>{/if}
					</div>
					{#if f.description}<div class="text-[var(--sb-text-faint)] mt-0.5 text-[10.5px] leading-snug">{f.description}</div>{/if}
				</div>

				<!-- control -->
				<div class="min-w-0 flex-1">
					{#if f.type === 'bool'}
						<button
							class="relative h-[18px] w-[32px] rounded-full transition-colors disabled:opacity-50"
							style:background={eff(f) ? 'var(--sb-success)' : 'var(--border-strong)'}
							onclick={() => set(f, !eff(f))}
							disabled={readOnly}
							aria-label="Toggle {f.key}"
						>
							<span class="absolute top-[2px] h-[14px] w-[14px] rounded-full bg-white transition-all" style:left={eff(f) ? '16px' : '2px'}></span>
						</button>
					{:else if f.type === 'enum'}
						<select
							class="bg-secondary w-full max-w-[280px] rounded-[2px] border px-2 py-1.5 text-[12px] outline-none disabled:opacity-60"
							style:border-color={err ? 'var(--sb-error)' : 'var(--border)'}
							value={eff(f)}
							disabled={readOnly}
							onchange={(e) => set(f, (e.currentTarget as HTMLSelectElement).value)}
						>
							{#each f.enum ?? [] as opt (opt)}<option value={opt}>{opt}</option>{/each}
						</select>
					{:else if f.type === 'secret'}
						<div class="flex items-center gap-2">
							<div class="bg-secondary flex max-w-[280px] flex-1 items-center gap-2 rounded-[2px] border border-border px-2 py-1.5">
								<LockIcon size={12} class="text-[var(--sb-stale)]" />
								<input type="password" value="••••••••••••" disabled class="text-muted-foreground w-full bg-transparent text-[12px] outline-none" />
							</div>
							{#if f.secretSource}<span class="text-[var(--sb-text-faint)] text-[12px]">{f.secretSource}</span>{/if}
						</div>
					{:else}
						<input
							class="bg-secondary w-full max-w-[280px] rounded-[2px] border px-2 py-1.5 text-[12px] outline-none disabled:opacity-60"
							style:border-color={err ? 'var(--sb-error)' : 'var(--border)'}
							type={f.type === 'int' ? 'number' : 'text'}
							min={f.min}
							max={f.max}
							placeholder={f.default !== undefined ? String(f.default) : ''}
							value={values[f.key] ?? ''}
							disabled={readOnly}
							oninput={(e) => set(f, f.type === 'int' ? Number((e.currentTarget as HTMLInputElement).value) : (e.currentTarget as HTMLInputElement).value)}
						/>
					{/if}

					<div class="mt-1 flex items-center gap-2 text-[10.5px]">
						{#if err}
							<span class="text-[var(--sb-error)]">⚠ {err}</span>
						{:else if values[f.key] === undefined && f.default !== undefined}
							<span class="text-[var(--sb-text-faint)]">default · {String(f.default)}</span>
						{/if}
						{#if f.type === 'int' && (f.min !== undefined || f.max !== undefined)}
							<span class="text-[var(--sb-text-faint)]">int · {f.min ?? '—'}–{f.max ?? '—'}</span>
						{/if}
					</div>
				</div>
			</div>
		{/each}
	</div>

	<!-- raw escape hatch -->
	<div class="mt-3 flex items-center">
		<button class="text-muted-foreground hover:text-foreground ml-auto flex items-center gap-1 text-[11px]" onclick={() => (rawOpen = !rawOpen)}>
			<ChevronRight size={12} style="transform: rotate({rawOpen ? 90 : 0}deg); transition: transform 0.12s;" />
			raw YAML
		</button>
	</div>
	{#if rawOpen}
		<pre class="bg-[var(--sb-inset)] mt-2 overflow-auto rounded-[2px] border border-[var(--border-subtle)] px-3 py-2.5 font-mono text-[11.5px] leading-[1.7]" style="color:var(--sb-mono)">{yaml}</pre>
	{/if}
</div>
