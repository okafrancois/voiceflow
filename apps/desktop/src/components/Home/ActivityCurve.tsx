import { useId } from "react";
import type { UsageDay } from "@/lib/tauri";

interface ActivityCurveProps {
  days: UsageDay[];
  title: string;
  metric: string;
  locale?: string;
}

export function ActivityCurve({ days, title, metric, locale }: ActivityCurveProps) {
  const gradientId = useId();
  const width = 720;
  const baseline = 184;
  const maximum = days.reduce((value, day) => Math.max(value, day.wordCount), 1);
  const number = new Intl.NumberFormat(locale);
  const points = days.map((day, index) => ({
    x: days.length === 1 ? width / 2 : 12 + index / (days.length - 1) * (width - 24),
    y: baseline - day.wordCount / maximum * 160,
    label: `${day.date}: ${number.format(day.wordCount)} ${metric}`,
    date: day.date,
  }));
  const first = points[0];
  const last = points.at(-1);
  const curve = points.reduce((path, point, index) => {
    if (index === 0) return `M ${point.x} ${point.y}`;
    const previous = points[index - 1];
    const middle = (previous.x + point.x) / 2;
    // Horizontal endpoint tangents keep each segment within its measured range.
    return `${path} C ${middle} ${previous.y}, ${middle} ${point.y}, ${point.x} ${point.y}`;
  }, "");

  return <svg viewBox={`0 0 ${width} 200`} className="h-52 w-full overflow-visible text-emerald-800 dark:text-emerald-300" preserveAspectRatio="none" role="img" aria-label={title}>
    <defs><linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stopColor="currentColor" stopOpacity="0.18" />
      <stop offset="100%" stopColor="currentColor" stopOpacity="0.01" />
    </linearGradient></defs>
    {[24, 104, baseline].map(y => <line key={y} x1="12" x2={width - 12} y1={y} y2={y} className="stroke-border/60" strokeDasharray="3 6" vectorEffect="non-scaling-stroke" />)}
    {first && last && points.length > 1 && <>
      <path d={`${curve} L ${last.x} ${baseline} L ${first.x} ${baseline} Z`} fill={`url(#${gradientId})`} />
      <path d={curve} fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
    </>}
    {points.map(point => <circle key={point.date} cx={point.x} cy={point.y} r="4" fill="currentColor" stroke="currentColor" strokeWidth="2" tabIndex={0} aria-label={point.label} className={points.length === 1 ? "outline-none focus:stroke-foreground" : "opacity-0 outline-none hover:opacity-100 focus:opacity-100 focus:stroke-foreground"}>
      <title>{point.label}</title>
    </circle>)}
  </svg>;
}
