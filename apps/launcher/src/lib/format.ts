import { t } from "./i18n";

export function bytes(n: number | null | undefined): string {
  if (n == null) return "–";
  const units = [t("units.b"), t("units.kb"), t("units.mb"), t("units.gb")];
  let v = n;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function speed(bps: number): string {
  return bytes(bps) + t("units.perSec");
}

export function duration(sec: number | null | undefined): string {
  if (sec == null || !isFinite(sec)) return "–";
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  if (h > 0) return `${h} ${t("units.h")} ${m} ${t("units.m")}`;
  if (m > 0) return `${m} ${t("units.m")}${m < 10 && s ? ` ${s} ${t("units.s")}` : ""}`;
  return `${s} ${t("units.s")}`;
}

export function date(iso: string): string {
  const d = new Date(iso);
  return isNaN(+d) ? iso : d.toLocaleDateString(undefined, { month: "2-digit", day: "2-digit" });
}

export function percent(a: number, b: number): number {
  return b > 0 ? Math.min(100, (a / b) * 100) : 0;
}
