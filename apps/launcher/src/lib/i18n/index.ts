// Minimal i18n: dotted keys + {param} interpolation, English fallback.
// Adding a language = add a dictionary with the same shape and register it in `dictionaries`.
import { en, type Dict } from "./en";

type Leaves<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? `${P}${K}` : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];

export type Key = Leaves<Dict>;

const dictionaries: Record<string, unknown> = { en };
let current: unknown = en;

export function setLanguage(lang: string) {
  current = dictionaries[lang] ?? en;
  document.documentElement.lang = dictionaries[lang] ? lang : "en";
}

function lookup(dict: unknown, key: string): string | undefined {
  let node = dict;
  for (const part of key.split(".")) {
    if (node && typeof node === "object" && part in node) node = (node as Record<string, unknown>)[part];
    else return undefined;
  }
  return typeof node === "string" ? node : undefined;
}

export function t(key: Key, params?: Record<string, string | number>): string {
  const s = lookup(current, key) ?? lookup(en, key) ?? key;
  return params ? s.replace(/\{(\w+)\}/g, (_, p) => String(params[p] ?? `{${p}}`)) : s;
}

/** For keys built at runtime (e.g. enum values). Falls back to the raw value. */
export function tDyn(key: string, fallback?: string): string {
  return lookup(current, key) ?? lookup(en, key) ?? fallback ?? key;
}
