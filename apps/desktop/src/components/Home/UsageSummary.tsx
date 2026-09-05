import { useTranslation } from "react-i18next";
import { Card } from "@/components/ui/card";
import type { HistoryStatistics } from "@/lib/tauri";

export function UsageSummary({ statistics }: { statistics: HistoryStatistics }) {
  const { t, i18n } = useTranslation();
  const number = new Intl.NumberFormat(i18n?.language);
  const metrics = [
    { label: t("usage.words"), value: number.format(statistics.wordCount) },
    { label: t("usage.dictations"), value: number.format(statistics.dictationCount) },
    { label: t("usage.audioMinutes"), value: number.format(Math.round(statistics.audioDurationMs / 6000) / 10) },
    { label: t("usage.activeDays"), value: number.format(statistics.activeDays) },
  ];
  return <dl className="grid grid-cols-2 gap-3 md:grid-cols-4">
    {metrics.map(({ label, value }) => <Card key={label} className="p-5">
      <dt className="text-xs font-medium text-muted-foreground">{label}</dt>
      <dd className="mt-3 text-3xl font-semibold tracking-tight tabular-nums">{value}</dd>
    </Card>)}
  </dl>;
}
