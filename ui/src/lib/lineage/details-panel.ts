// Whether a graph page's Details sidebar is open. Pure; no side effects.

/** What the person last chose with the sidebar's toggle; null until they use it. */
export type DetailsChoice = 'open' | 'closed' | null;

export const DETAILS_KEY_PREFIX = 'sb-details:';

/**
 * Closed until something is selected, and open while it is, until the person opens or closes the
 * sidebar themselves; from then on their choice holds, selection or not.
 */
export function detailsOpen(choice: DetailsChoice, hasSelection: boolean): boolean {
	return choice === null ? hasSelection : choice === 'open';
}

/** The choice a click on the toggle records: the opposite of what is shown now. */
export function toggledChoice(choice: DetailsChoice, hasSelection: boolean): Exclude<DetailsChoice, null> {
	return detailsOpen(choice, hasSelection) ? 'closed' : 'open';
}

/** A stored choice, ignoring anything this version didn't write. */
export function parseChoice(raw: string | null): DetailsChoice {
	return raw === 'open' || raw === 'closed' ? raw : null;
}
