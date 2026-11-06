import type { Capability } from './types';

/**
 * Whether the current principal holds `capability`.
 *
 * Only used to hide or disable UI; the server remains the real check.
 */
export function hasCapability(capabilities: readonly Capability[] | undefined, capability: Capability): boolean {
	return capabilities?.includes(capability) ?? false;
}

/** Whether the current principal holds any of `required`. */
export function hasAnyCapability(
	capabilities: readonly Capability[] | undefined,
	required: readonly Capability[]
): boolean {
	return required.some((capability) => hasCapability(capabilities, capability));
}

export const ADMIN_CAPABILITIES: readonly Capability[] = [
	'location.manage',
	'user.manage',
	'role.assign',
	'audit.view'
];
