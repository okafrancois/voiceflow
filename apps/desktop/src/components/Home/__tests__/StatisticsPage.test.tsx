import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { StatisticsPage } from "../StatisticsPage";
import type { HistoryStatistics } from "@/lib/tauri";
const { getStatistics } = vi.hoisted(() => ({ getStatistics: vi.fn() }));
vi.mock("@/lib/tauri", () => ({ statisticsCommands: { getStatistics }, events: { onTranscriptionComplete: vi.fn(async () => () => undefined) } }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }) }));
const sample: HistoryStatistics = { period: "7d", rangeStartMs: 0, rangeEndMs: 1, wordCount: 210, dictationCount: 4, audioDurationMs: 120000, activeDays: 2, localDictationCount: 4, cloudDictationCount: 0, trend: [] };

beforeEach(() => getStatistics.mockReset());

it("keeps the selected period when an older request finishes last", async () => {
  let resolveOld: (value: HistoryStatistics) => void = () => undefined;
  getStatistics.mockImplementationOnce(() => new Promise<HistoryStatistics>(resolve => { resolveOld = resolve; })).mockResolvedValueOnce({ ...sample, period: "30d", wordCount: 987 });
  render(<StatisticsPage />);
  fireEvent.click(screen.getByRole("button", { name: "usage.period30" }));
  expect(await screen.findByText("987")).toBeInTheDocument();
  await act(async () => resolveOld(sample));
  expect(screen.queryByText("210")).not.toBeInTheDocument();
  expect(screen.getByText("usage.retainedOnly")).toBeInTheDocument();
});

it("ignores an older request error after the selected period succeeds", async () => {
  let rejectOld: (reason: Error) => void = () => undefined;
  getStatistics
    .mockImplementationOnce(() => new Promise<HistoryStatistics>((_resolve, reject) => { rejectOld = reject; }))
    .mockResolvedValueOnce({ ...sample, period: "30d", wordCount: 987 });

  render(<StatisticsPage />);
  fireEvent.click(screen.getByRole("button", { name: "usage.period30" }));
  expect(await screen.findByText("987")).toBeInTheDocument();
  await act(async () => rejectOld(new Error("stale failure")));

  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.getByText("987")).toBeInTheDocument();
});

it("labels the all-time chart as a sparse list of active local dates", async () => {
  getStatistics
    .mockResolvedValueOnce(sample)
    .mockResolvedValueOnce({
      ...sample,
      period: "all",
      trend: [
        { date: "2024-01-01", wordCount: 100, dictationCount: 1, audioDurationMs: 1_000, localDictationCount: 1, cloudDictationCount: 0 },
        { date: "2026-09-05", wordCount: 110, dictationCount: 3, audioDurationMs: 2_000, localDictationCount: 3, cloudDictationCount: 0 },
      ],
    });

  render(<StatisticsPage />);
  fireEvent.click(screen.getByRole("button", { name: "usage.periodAll" }));

  expect(await screen.findByText("usage.activeDatesOnly")).toBeInTheDocument();
  expect(screen.queryByText("usage.localDates")).not.toBeInTheDocument();
});

it("shows a selected-period failure without stale statistics", async () => {
  getStatistics
    .mockResolvedValueOnce(sample)
    .mockRejectedValueOnce(new Error("statistics unavailable"));

  render(<StatisticsPage />);
  expect(await screen.findByText("210")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "usage.periodAll" }));

  expect(await screen.findByRole("alert")).toHaveTextContent("statistics unavailable");
  expect(screen.queryByText("210")).not.toBeInTheDocument();
});
