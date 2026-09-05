import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useUsageStatistics } from "@/hooks/useUsageStatistics";
import type { StatisticsPeriod } from "@/lib/tauri";
import { SettingsPageLayout } from "./SettingsPageLayout";
import { ActivityCurve } from "./ActivityCurve";
import { UsageSummary } from "./UsageSummary";

export function StatisticsPage() {
  const { t, i18n } = useTranslation();
  const [period, setPeriod] = useState<StatisticsPeriod>("7d");
  const { statistics, error, loading, refresh } = useUsageStatistics(period);
  const periods: { value: StatisticsPeriod; label: string }[] = [
    { value: "7d", label: t("usage.period7") }, { value: "30d", label: t("usage.period30") }, { value: "all", label: t("usage.periodAll") },
  ];
  const maxWords = statistics?.trend.reduce((maximum, day) => Math.max(maximum, day.wordCount), 1) ?? 1;
  const number = new Intl.NumberFormat(i18n?.language);
  return <SettingsPageLayout title={t("usage.title")} description={t("usage.description")} testId="statistics-page">
    <div className="flex flex-wrap gap-2" role="group" aria-label={t("usage.period")}>
      {periods.map(option => <Button key={option.value} variant={period === option.value ? "default" : "outline"} aria-pressed={period === option.value} onClick={() => setPeriod(option.value)}>{option.label}</Button>)}
    </div>
    {error && <div role="alert" className="rounded-2xl border border-destructive/40 p-4 text-sm">{error}<Button variant="ghost" onClick={refresh}>{t("common.retry")}</Button></div>}
    {loading && <p role="status" className="text-sm text-muted-foreground">{t("common.loading")}</p>}
    {statistics && !loading && <>
      <UsageSummary statistics={statistics} />
      <Card><CardHeader><CardTitle>{t("usage.activity")}</CardTitle><p className="text-xs text-muted-foreground">{t(period === "all" ? "usage.activeDatesOnly" : "usage.localDates")}</p></CardHeader>
        <CardContent>
          {statistics.dictationCount === 0 ? <p className="py-8 text-center text-sm text-muted-foreground">{t("usage.empty")}</p> : <>
            <div className="mb-2 text-xs tabular-nums text-muted-foreground">{number.format(maxWords)} {t("usage.words")}</div>
            <ActivityCurve days={statistics.trend} title={t("usage.activity")} metric={t("usage.words")} locale={i18n?.language} />
            <div className="mt-2 flex justify-between gap-4 text-xs tabular-nums text-muted-foreground"><span>{statistics.trend[0]?.date}</span><span>{statistics.trend.at(-1)?.date}</span></div>
            <details className="mt-4 text-sm"><summary className="cursor-pointer text-muted-foreground">{t("usage.viewDaily")}</summary>
              <div className="mt-3 max-h-72 overflow-auto"><table className="w-full text-left text-xs"><thead><tr><th className="p-2">{t("usage.date")}</th><th>{t("usage.words")}</th><th>{t("usage.dictations")}</th></tr></thead><tbody>
                {statistics.trend.map(day => <tr className="border-t border-border/50" key={day.date}><td className="p-2">{day.date}</td><td>{number.format(day.wordCount)}</td><td>{number.format(day.dictationCount)}</td></tr>)}
              </tbody></table></div>
            </details>
          </>}
        </CardContent>
      </Card>
      <Card><CardHeader><CardTitle>{t("usage.processing")}</CardTitle></CardHeader><CardContent>
        <dl className="grid grid-cols-2 gap-4 text-sm"><div><dt className="text-muted-foreground">{t("history.filter.local")}</dt><dd className="mt-1 text-xl font-semibold">{number.format(statistics.localDictationCount)}</dd></div><div><dt className="text-muted-foreground">{t("history.filter.cloud")}</dt><dd className="mt-1 text-xl font-semibold">{number.format(statistics.cloudDictationCount)}</dd></div></dl>
      </CardContent></Card>
    </>}
    <p className="text-xs leading-6 text-muted-foreground">{t("usage.retainedOnly")}</p>
  </SettingsPageLayout>;
}
