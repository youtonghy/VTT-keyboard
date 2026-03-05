import { X } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { TranscriptionHistoryItem } from "../types/history";

interface HistoryDetailDialogProps {
  item: TranscriptionHistoryItem | null;
  onClose: () => void;
}

export function HistoryDetailDialog({ item, onClose }: HistoryDetailDialogProps) {
  const { t } = useTranslation();

  useEffect(() => {
    if (!item) {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [item, onClose]);

  if (!item) {
    return null;
  }

  const hasTriggerDetails = item.triggered;
  const hasError = Boolean(item.errorMessage?.trim());
  const formatMillisecondsAsSeconds = (valueMs: number | undefined) => {
    if (!Number.isFinite(valueMs) || !valueMs || valueMs <= 0) {
      return "--";
    }
    const seconds = (valueMs / 1000).toFixed(1);
    return t("history.seconds", { seconds });
  };
  const modelGroup = item.modelGroup?.trim()
    ? item.modelGroup
    : t("history.unknownModelGroup");
  const transcriptionElapsed = formatMillisecondsAsSeconds(item.transcriptionElapsedMs);
  const recordingDuration = formatMillisecondsAsSeconds(item.recordingDurationMs);

  return (
    <div className="history-dialog-backdrop" role="presentation" onClick={onClose}>
      <section
        className="history-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("history.detailTitle")}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="history-dialog-header">
          <h4>{t("history.detailTitle")}</h4>
          <button type="button" className="history-dialog-close" onClick={onClose}>
            <X size={16} />
          </button>
        </header>

        <div className="history-dialog-body">
          <div className="history-detail-row">
            <span>{t("history.detailModelGroup")}</span>
            <strong>{modelGroup}</strong>
          </div>

          <div className="history-detail-row">
            <span>{t("history.detailTranscriptionElapsed")}</span>
            <strong>{transcriptionElapsed}</strong>
          </div>

          <div className="history-detail-row">
            <span>{t("history.detailRecordingDuration")}</span>
            <strong>{recordingDuration}</strong>
          </div>

          <div className="history-detail-row">
            <span>{t("history.detailTranscription")}</span>
            <strong>{item.transcriptionText || t("history.emptyText")}</strong>
          </div>

          {hasTriggerDetails ? (
            <>
              <div className="history-detail-row">
                <span>{t("history.detailTriggerEvent")}</span>
                {item.triggerMatches.length > 0 ? (
                  <ul className="history-trigger-list">
                    {item.triggerMatches.map((match) => (
                      <li key={`${item.id}-${match.triggerId}-${match.mode}-${match.matchedValue}`}>
                        {match.triggerTitle} / {match.keyword || t("history.noKeyword")} / {match.matchedValue} / {t(`history.triggerMode.${match.mode}`)}
                      </li>
                    ))}
                  </ul>
                ) : (
                  <strong>{t("history.none")}</strong>
                )}
              </div>

              <div className="history-detail-row">
                <span>{t("history.detailTriggered")}</span>
                <strong>{item.triggeredByKeyword ? t("history.yes") : t("history.no")}</strong>
              </div>

              <div className="history-detail-row">
                <span>{t("history.detailTriggeredWhich")}</span>
                <strong>
                  {item.triggerMatches.length > 0
                    ? item.triggerMatches.map((match) => match.triggerTitle).join(" / ")
                    : t("history.none")}
                </strong>
              </div>

              <div className="history-detail-row">
                <span>{t("history.detailOriginal")}</span>
                <strong>{item.transcriptionText || t("history.emptyText")}</strong>
              </div>

              <div className="history-detail-row">
                <span>{t("history.detailFinal")}</span>
                <strong>{item.finalText || t("history.emptyText")}</strong>
              </div>
            </>
          ) : null}

          {hasError ? (
            <div className="history-detail-row">
              <span>{t("history.detailError")}</span>
              <strong className="history-error-text">{item.errorMessage}</strong>
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}
