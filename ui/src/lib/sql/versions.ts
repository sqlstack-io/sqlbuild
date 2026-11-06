/** A stored version of a model's SQL to compare against, e.g. what a build ran. */
export type SqlVersion = { id: string; label: string; detail: string; raw: string; compiled?: string };
