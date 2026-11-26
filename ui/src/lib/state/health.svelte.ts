import { isUnsupported, type ApiClient, type HealthComponentDetails, type ServerHealth } from '$lib/api';
import { degradedParts, outageOf, SERVER_UNREACHABLE, shellHealth, type Outage } from '$lib/platform/health';

/** How often health is read while everything works; while something doesn't, the report's own retry interval. */
const HEALTHY_POLL_MS = 30_000;
/** The retry interval before a report has said one. */
const DEFAULT_RETRY_SECONDS = 10;

/**
 * The server's health, read from `/api/health` without a session, so the shell can say why pages don't load
 * while the control database is unreachable, and load them again by itself when it is back.
 */
class HealthState {
	report = $state<ServerHealth | null>(null);
	/** The server itself didn't answer: why, and since when. */
	unreachable = $state<Outage | null>(null);
	/** Administrators' details; null when the person may not read them or the server can't check. */
	details = $state<HealthComponentDetails[] | null>(null);

	readonly status = $derived(shellHealth(this.report, this.unreachable));
	readonly outage = $derived(outageOf(this.report, this.unreachable));
	readonly degraded = $derived(degradedParts(this.report));
	readonly retrySeconds = $derived(this.report?.retrySeconds ?? DEFAULT_RETRY_SECONDS);

	#client: ApiClient | null = null;
	#timer: ReturnType<typeof setTimeout> | null = null;
	#recovered: (() => void)[] = [];
	#admin = false;

	/** Read health now and keep reading it. */
	start(client: ApiClient): void {
		if (this.#client || !client.supports('readHealth')) return;
		this.#client = client;
		void this.refresh();
	}

	/** Also read the details, for a person who may manage people. */
	readDetails(admin: boolean): void {
		this.#admin = admin;
		if (admin) void this.#loadDetails();
	}

	/** Run `listener` each time the server can serve pages again after it couldn't. */
	onRecovered(listener: () => void): void {
		this.#recovered.push(listener);
	}

	async refresh(): Promise<void> {
		const client = this.#client;
		if (!client) return;
		if (this.#timer) clearTimeout(this.#timer);
		const blocked = this.outage !== null;
		try {
			this.report = await client.readHealth();
			this.unreachable = null;
		} catch (error) {
			if (isUnsupported(error)) return;
			this.unreachable ??= { reason: SERVER_UNREACHABLE, since: new Date().toISOString() };
		}
		if (blocked && this.outage === null) for (const listener of this.#recovered) listener();
		if (this.#admin && this.status !== 'live') void this.#loadDetails();
		const wait = this.status === 'live' ? HEALTHY_POLL_MS : this.retrySeconds * 1000;
		this.#timer = setTimeout(() => void this.refresh(), wait);
	}

	async #loadDetails(): Promise<void> {
		if (!this.#client?.supports('readHealthDetails')) return;
		try {
			this.details = await this.#client.readHealthDetails();
		} catch {
			// Details need a session the server can check, which it can't while its control database is down.
			this.details = null;
		}
	}
}

export const health = new HealthState();
