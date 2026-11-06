import { browser } from '$app/environment';
import {
	channelMatches,
	createClient,
	hasCapability,
	type ApiClient,
	type Bootstrap,
	type Capability,
	type ConnectionState,
	type NavCounts,
	type PlatformEvent,
	type Scope,
	type WarehouseSync
} from '$lib/api';

const SCOPE_KEY = 'sb-scope';

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/**
 * App-wide state: the bootstrap, the selected location, and the
 * shell's live data (navigation counts, platform events, warehouse sync).
 */
class AppState {
	client: ApiClient;
	bootstrap = $state<Bootstrap | null>(null);
	bootError = $state<string | null>(null);
	locationId = $state('');
	/** Wall clock, ticking every second, for ages and elapsed times. */
	now = $state(Date.now());
	connection = $state<ConnectionState>('connecting');
	navCounts = $state<NavCounts | null>(null);
	platformEvents = $state<PlatformEvent[]>([]);
	warehouseSync = $state<WarehouseSync | null>(null);

	readonly scope = $derived<Scope | null>(this.locationId ? { locationId: this.locationId } : null);
	readonly location = $derived(this.bootstrap?.locations.find((l) => l.id === this.locationId) ?? null);
	/** The selected location's one target. */
	readonly target = $derived(this.location?.target ?? null);
	readonly user = $derived(this.bootstrap?.user ?? null);
	readonly capabilities = $derived(this.bootstrap?.capabilities ?? []);

	#started = false;

	constructor() {
		this.client = createClient(new URLSearchParams(browser ? window.location.search : ''));
	}

	can(capability: Capability): boolean {
		return hasCapability(this.capabilities, capability);
	}

	async start(): Promise<void> {
		if (this.#started) return;
		this.#started = true;
		setInterval(() => (this.now = Date.now()), 1000);
		this.client.changes.onConnectionState((state) => (this.connection = state));
		this.client.changes.subscribe((signal) => {
			if (channelMatches(signal.channel, ['platform_events'])) void this.#loadPlatformEvents();
			if (channelMatches(signal.channel, ['warehouse_sync'])) void this.#loadWarehouseSync();
			if (channelMatches(signal.channel, ['runs', 'triggers', 'checks'])) void this.#loadNavCounts();
		});
		try {
			this.bootstrap = await this.client.bootstrap();
		} catch (error) {
			this.bootError = message(error);
			return;
		}
		this.#restoreScope();
		void this.#loadPlatformEvents();
		this.#loadScoped();
	}

	selectLocation(locationId: string): void {
		const location = this.bootstrap?.locations.find((l) => l.id === locationId);
		if (!location) return;
		this.locationId = location.id;
		this.#scopeChanged();
	}

	refreshNavCounts(): void {
		void this.#loadNavCounts();
	}

	#scopeChanged() {
		if (browser) localStorage.setItem(SCOPE_KEY, JSON.stringify(this.scope));
		this.navCounts = null;
		this.warehouseSync = null;
		this.#loadScoped();
	}

	#restoreScope() {
		const locations = this.bootstrap?.locations ?? [];
		const search = browser ? new URLSearchParams(window.location.search) : new URLSearchParams();
		let saved: Partial<Scope> = {};
		try {
			saved = browser ? JSON.parse(localStorage.getItem(SCOPE_KEY) ?? '{}') ?? {} : {};
		} catch {
			saved = {};
		}
		const wantedLocation = search.get('location') ?? saved.locationId;
		const location =
			locations.find((l) => l.id === wantedLocation || l.name === wantedLocation) ?? locations[0];
		if (!location) return;
		this.locationId = location.id;
	}

	#loadScoped() {
		void this.#loadNavCounts();
		void this.#loadWarehouseSync();
	}

	async #loadNavCounts() {
		const scope = this.scope;
		if (!scope) return;
		try {
			const counts = await this.client.navCounts(scope);
			if (this.scope === scope) this.navCounts = counts;
		} catch {
			// Counts are decoration; the pages report their own errors.
		}
	}

	async #loadWarehouseSync() {
		const scope = this.scope;
		if (!scope) return;
		try {
			const sync = await this.client.getWarehouseSync(scope);
			if (this.scope === scope) this.warehouseSync = sync;
		} catch {
			this.warehouseSync = null;
		}
	}

	async #loadPlatformEvents() {
		try {
			this.platformEvents = await this.client.listPlatformEvents({ activeOnly: true });
		} catch {
			this.platformEvents = [];
		}
	}
}

export const app = new AppState();
