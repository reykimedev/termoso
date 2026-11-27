// UI strings. The app ships in English only; `tr()` and friends stay as the
// one place that fills `{name}` placeholders and picks plural forms.

import { createElement, Fragment, type ReactNode } from "react";

export type Vars = Record<string, string | number>;

function interpolate(s: string, vars: Vars | undefined): string {
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

/** Marks a module-level string that is shown later through `tr()`. */
export const msg = (text: string): string => text;

/** `text` with `{name}` placeholders filled from `vars`. */
export function tr(text: string, vars?: Vars): string {
  return interpolate(text, vars);
}

/** Like `tr`, but placeholders may be React elements (e.g. `<b>{name}</b>`);
 *  returns the pieces to render as children. */
export function trx(text: string, vars: Record<string, ReactNode>): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /\{(\w+)\}/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const k = m[1] ?? "";
    out.push(k in vars ? createElement(Fragment, { key: m.index }, vars[k]) : m[0]);
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** Pluralised message: `one` for a count of 1, `other` otherwise; `{count}` is filled in. */
export function trn(count: number, one: string, other: string, vars?: Vars): string {
  return interpolate(count === 1 ? one : other, { count, ...vars });
}
