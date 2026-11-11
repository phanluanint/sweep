// Formatting helpers. Sizes use base 10, like Finder.

export function bytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 KB";
  const units = ["bytes", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  if (i === 0) return `${Math.round(v)} bytes`;
  const digits = v >= 100 || i === 1 ? 0 : 1;
  return `${v.toFixed(digits)} ${units[i]}`;
}

export function count(n: number): string {
  return n.toLocaleString("en-US");
}

export function pct(part: number, whole: number): number {
  return whole > 0 ? Math.min(100, Math.max(0, (part / whole) * 100)) : 0;
}

export function ago(unixSecs: number, now = Date.now() / 1000): string {
  const days = Math.floor((now - unixSecs) / 86400);
  if (days < 1) return "Today";
  if (days < 2) return "Yesterday";
  if (days < 30) return `${days} days ago`;
  const months = Math.floor(days / 30.4);
  if (months < 12) return `${months} month${months === 1 ? "" : "s"} ago`;
  const years = Math.floor(days / 365);
  return `${years} year${years === 1 ? "" : "s"} ago`;
}

export function uptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d} day${d === 1 ? "" : "s"} ${h} h`;
  if (h > 0) return `${h} h ${m} min`;
  return `${m} min`;
}

export function rpm(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}
