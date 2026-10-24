import { useId, useState, type ReactNode } from "react";
import { ArrowDown, Check, Trash } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import "@/components/Home/model/model-catalog.css";

export interface CatalogMetric {
  value: number | null;
  maximum: number;
  label?: string;
}

export interface CatalogModel {
  id: string;
  name: string;
  description: string;
  sizeMb: number | null;
  sizeLabel: string;
  available: boolean;
  builtIn: boolean;
  selected: boolean;
  speed: CatalogMetric;
  accuracy?: CatalogMetric;
  usage?: string;
  languageCodes?: string[];
  templates?: string[];
  warning?: string | null;
  downloading: boolean;
  progress?: number | null;
  downloadDisabled?: boolean;
}

interface ModelCatalogProps {
  models: CatalogModel[];
  kind: "transcription" | "polish";
  language?: string;
  onSelect: (id: string) => void;
  onDownload: (id: string) => void;
  onCancel: (id: string) => void;
  onDelete: (id: string) => void;
}

export const TEMPLATE_LABEL_KEYS: Record<string, string> = {
  filler: "model.catalog.useClean",
  chat: "model.catalog.useChat",
  formal: "model.catalog.useFormal",
  concise: "model.catalog.useConcise",
  document: "model.catalog.useDocument",
  agent: "model.catalog.useAgent",
};

export function displayModelName(name: string): string {
  return name.replace(/\s+\([\d.]+[MG]\)$/, "");
}

export function displaySize(sizeMb: number, locale?: string): string {
  const formatter = new Intl.NumberFormat(locale, { style: "unit", unit: sizeMb >= 1000 ? "gigabyte" : "megabyte", unitDisplay: "short", maximumFractionDigits: sizeMb >= 1000 ? 2 : 0 });
  const value = sizeMb >= 1000 ? sizeMb / 1000 : sizeMb;
  return formatter.format(value);
}

/** Parse catalogue display units only; an absent size remains unknown. */
export function parseDisplaySize(size: string): number | null {
  const match = size.trim().match(/^~?\s*([\d.]+)\s*(MB|GB)$/i);
  return match ? Number(match[1]) * (match[2].toUpperCase() === "GB" ? 1000 : 1) : null;
}

export function ActiveModelSummary({ name, detail, ready = true, children }: {
  name: string;
  detail?: string;
  ready?: boolean;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <div className="model-active-summary">
      <span className={ready ? "model-active-dot" : "model-active-dot model-active-dot-pending"} aria-hidden="true" />
      <div className="model-active-copy">
        <p className="model-overline">{t("model.catalog.active")}</p>
        <p className="model-active-name">{name}</p>
        {children}
      </div>
      {detail && <p className="model-active-detail">{detail}</p>}
    </div>
  );
}

function ModelGauge({ metric, label, secondary = false }: { metric: CatalogMetric; label: string; secondary?: boolean }) {
  const { t } = useTranslation();
  const value = metric.value === null ? null : Math.max(0, Math.min(metric.maximum, metric.value));
  const text = metric.label ?? (value === null ? t("model.catalog.unmeasured") : `${value}/${metric.maximum}`);
  return (
    <div className={secondary ? "model-gauge model-gauge-secondary" : "model-gauge"}>
      <div className="model-gauge-caption"><span className="model-mobile-label">{label}</span><span>{text}</span></div>
      <div className="model-gauge-track" role="img" aria-label={`${label}: ${text}`}>
        {Array.from({ length: metric.maximum }, (_, index) => <span key={index} className={value !== null && index < value ? "model-gauge-segment model-gauge-filled" : "model-gauge-segment"} />)}
      </div>
    </div>
  );
}

export function ModelCatalog({ models, kind, language, onSelect, onDownload, onCancel, onDelete }: ModelCatalogProps) {
  const { t } = useTranslation();
  const headingId = useId();
  const [size, setSize] = useState("all");
  const [scope, setScope] = useState("all");
  const [availableOnly, setAvailableOnly] = useState(false);
  const [sort, setSort] = useState("catalogue");
  const [expanded, setExpanded] = useState(false);
  const isPolish = kind === "polish";
  const baseLanguage = language?.toLowerCase().split(/[-_]/)[0];
  const languageKnown = Boolean(baseLanguage && baseLanguage !== "auto");
  const rows = models.filter(model => {
    if (availableOnly && !model.available) return false;
    if (size !== "all" && model.sizeMb === null) return false;
    if (size === "500" && model.sizeMb !== null && model.sizeMb >= 500) return false;
    if (size === "1000" && model.sizeMb !== null && model.sizeMb >= 1000) return false;
    if (size === "large" && model.sizeMb !== null && model.sizeMb < 1000) return false;
    if (isPolish && scope !== "all" && !model.templates?.includes(scope)) return false;
    if (!isPolish && scope === "current" && languageKnown && model.languageCodes?.length && !model.languageCodes.includes(baseLanguage ?? "")) return false;
    return true;
  }).sort((a, b) => {
    if (sort === "size") return (a.sizeMb ?? Infinity) - (b.sizeMb ?? Infinity);
    if (sort === "speed") return (b.speed.value ?? -1) - (a.speed.value ?? -1);
    if (sort === "accuracy") return (b.accuracy?.value ?? -1) - (a.accuracy?.value ?? -1);
    return Number(b.selected) - Number(a.selected);
  });
  const visibleRows = expanded ? rows : rows.slice(0, 6);
  const speedLabel = isPolish ? t("model.catalog.responsiveness") : t("model.available.speed");

  return (
    <section className="model-catalog" aria-labelledby={headingId}>
      <div className="model-catalog-heading">
        <div><h3 id={headingId}>{t("model.catalog.title")}</h3><span className="model-count" aria-live="polite">{t("model.catalog.count", { count: rows.length })}</span></div>
        <label className="model-sort">{t("model.catalog.sort")}<select aria-label={t("model.catalog.sort")} value={sort} onChange={event => setSort(event.target.value)}>
          <option value="catalogue">{t("model.catalog.catalogueOrder")}</option>
          <option value="speed">{t("model.available.speed")}</option>
          {!isPolish && <option value="accuracy">{t("model.available.accuracy")}</option>}
          <option value="size">{t("model.catalog.smallest")}</option>
        </select></label>
      </div>
      <div className="model-filters">
        <label className="model-filter-select"><span>{t("model.catalog.size")}</span><select aria-label={t("model.catalog.size")} value={size} onChange={event => { setSize(event.target.value); setExpanded(false); }}>
          <option value="all">{t("model.catalog.allSizes")}</option><option value="500">{t("model.catalog.under500")}</option><option value="1000">{t("model.catalog.under1gb")}</option><option value="large">{t("model.catalog.over1gb")}</option>
        </select></label>
        <label className="model-filter-select"><span>{isPolish ? t("model.catalog.usage") : t("model.catalog.language")}</span><select aria-label={isPolish ? t("model.catalog.usage") : t("model.catalog.language")} value={scope} onChange={event => { setScope(event.target.value); setExpanded(false); }}>
          <option value="all">{t("model.catalog.all")}</option>
          {isPolish ? Object.entries(TEMPLATE_LABEL_KEYS).map(([id, key]) => <option key={id} value={id}>{t(key)}</option>) : <option value="current" disabled={!languageKnown}>{t("model.catalog.currentLanguage", { language: language ?? "auto" })}</option>}
        </select></label>
        <label className="model-filter-ready"><input type="checkbox" checked={availableOnly} onChange={event => { setAvailableOnly(event.target.checked); setExpanded(false); }} />{t("model.catalog.readyOnly")}</label>
      </div>
      <div className="model-catalog-columns" aria-hidden="true"><span>{t("model.info.model")}</span><span>{speedLabel} ↑</span><span>{isPolish ? t("model.catalog.bestFor") : t("model.available.accuracy")} {isPolish ? "" : "↑"}</span><span className="model-disk-label">{t("model.catalog.disk")}</span><span /></div>
      <div className="model-catalog-list">
        {visibleRows.map(model => (
          <article className="model-catalog-row" key={model.id} aria-label={model.name}>
            <div className="model-catalog-identity"><p className="model-catalog-name">{displayModelName(model.name)}</p><p className="model-catalog-description">{model.description}</p></div>
            <div className="model-speed-cell"><ModelGauge metric={model.speed} label={speedLabel} /></div>
            <div className="model-second-cell">{model.accuracy ? <ModelGauge metric={model.accuracy} label={t("model.available.accuracy")} secondary /> : <p className="model-usage">{model.usage}</p>}</div>
            <p className="model-catalog-size">{model.sizeLabel}</p>
            <div className="model-catalog-actions">
              {model.downloading ? <Button size="sm" variant="outline" onClick={() => onCancel(model.id)}>{t("model.available.cancel")}</Button>
                : model.selected && model.available ? <Button size="sm" className="model-selected-button" disabled><Check aria-hidden="true" />{t("model.catalog.selected")}</Button>
                : model.available ? <Button size="sm" onClick={() => onSelect(model.id)}>{t("model.catalog.use")}</Button>
                : !model.builtIn ? <Button size="sm" variant="outline" disabled={model.downloadDisabled} onClick={() => onDownload(model.id)}><ArrowDown aria-hidden="true" />{t("model.available.download")}</Button>
                : <span className="model-unavailable">{t("model.catalog.unavailable")}</span>}
              {model.available && !model.builtIn && !model.downloading && <Button size="icon" variant="ghost" className="model-delete-button" aria-label={t("model.available.delete")} title={t("model.available.delete")} onClick={() => onDelete(model.id)}><Trash aria-hidden="true" /></Button>}
            </div>
            {model.warning && <p className="model-row-warning">{model.warning}</p>}
            {model.downloading && <div className="model-download"><div role="progressbar" aria-label={t("model.available.download")} aria-valuenow={model.progress ?? undefined} aria-valuemin={0} aria-valuemax={100} className="model-download-track"><div className="model-download-fill" style={{ width: `${Math.max(0, Math.min(100, model.progress ?? 0))}%` }} /></div><span>{model.progress === null || model.progress === undefined ? t("model.catalog.starting") : `${model.progress}%`}</span></div>}
          </article>
        ))}
        {rows.length === 0 && <p className="model-catalog-empty" role="status">{models.length === 0 ? t("model.active.noModels") : t("model.catalog.empty")}</p>}
      </div>
      {rows.length > 6 && <button type="button" className="model-show-more" aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>{expanded ? t("model.catalog.showLess") : t("model.catalog.showMore", { count: rows.length - 6 })}</button>}
      <p className="model-catalog-note">{isPolish ? t("model.catalog.polishNote") : t("model.catalog.sttNote")}</p>
    </section>
  );
}
