import { useCallback, useEffect, useRef, useState } from "react";
import { statisticsCommands, events, type HistoryStatistics, type StatisticsPeriod } from "@/lib/tauri";
import { useEventListeners } from "./useEventListeners";

export function useUsageStatistics(period: StatisticsPeriod) {
  const [statistics, setStatistics] = useState<HistoryStatistics | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const requestId = useRef(0);
  const refresh = useCallback(async () => {
    const id = ++requestId.current;
    setLoading(true);
    setError(null);
    try {
      const result = await statisticsCommands.getStatistics(period);
      if (id === requestId.current) setStatistics(result);
    } catch (caught) {
      if (id === requestId.current) {
        setStatistics(null);
        setError(String(caught));
      }
    } finally {
      if (id === requestId.current) setLoading(false);
    }
  }, [period]);
  useEffect(() => {
    setStatistics(null);
    void refresh();
    window.addEventListener("focus", refresh);
    return () => { ++requestId.current; window.removeEventListener("focus", refresh); };
  }, [refresh]);
  useEventListeners(async () => [await events.onTranscriptionComplete(refresh)], [refresh]);
  return { statistics, error, loading, refresh };
}
