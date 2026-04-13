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
  const isFailed = item.status === "failed";

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
          <div className="history-dialog-header-actions">
            {isFailed && (
              <span className="history-status-badge history-status-failed">
                {t("history.failed")}
              </span>
            )}
            <button type="button" className="history-dialog-close" onClick={onClose}>
              <X size={16} />
            </button>
          </div>
        </header>

        <div className="history-dialog-body">
          <div className="history-detail-row">
            <span>{t("history.detailTranscription")}</span>
            <strong>{item.transcriptionText || t("history.emptyText")}</strong>
          </div>

          {hasTriggerDetails ? (
            <>
              {item.triggerMatches.length > 0 && (
                <div className="history-detail-row">
                  <span>{t("history.detailTriggerMatch")}</span>
                  <div className="history-trigger-chips">
                    {item.triggerMatches.map((match) => (
                      <span
                        className="history-trigger-chip"
                        key={`${item.id}-${match.triggerId}-${match.mode}-${match.matchedValue}`}
                      >
                        <strong>{match.triggerTitle}</strong>
                        <span className="history-trigger-chip-sep">·</span>
                        {match.matchedValue}
                        <span className="history-trigger-chip-mode">
                          {t(`history.triggerMode.${match.mode}`)}
                        </span>
                      </span>
                    ))}
                  </div>
                </div>
              )}

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

          <div className="history-detail-meta">
            <span title={modelGroup}>{modelGroup}</span>
            <span className="history-detail-meta-sep">·</span>
            <span>{t("history.detailTranscriptionElapsed")} {transcriptionElapsed}</span>
            <span className="history-detail-meta-sep">·</span>
            <span>{t("history.detailRecordingDuration")} {recordingDuration}</span>
          </div>
        </div>
      </section>
    </div>
  );
}
