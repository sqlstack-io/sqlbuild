import type { Account } from "$lib/api";

/** What the Disable or Enable confirmation says for `a`; disabling is the destructive one. */
export function accountToggle(a: Account): {
  title: string;
  body: string;
  confirm: string;
  destructive: boolean;
} {
  if (a.disabled)
    return {
      title: `Enable ${a.displayName}?`,
      body: `${a.displayName} (${a.name}) can sign in again.`,
      confirm: "Enable",
      destructive: false,
    };
  const sessions =
    a.activeSessions === 1
      ? "their 1 signed-in session ends"
      : `their ${a.activeSessions} signed-in sessions end`;
  return {
    title: `Disable ${a.displayName}?`,
    body: `${a.displayName} (${a.name}) can't sign in any more, in the browser or with sqb login, and ${sessions} now.`,
    confirm: "Disable",
    destructive: true,
  };
}
