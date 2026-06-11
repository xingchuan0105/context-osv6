"use client";

import { useEffect, useRef, useState, type ChangeEvent, type DragEvent } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import type { WorkspaceSource } from "../../lib/workspace/model";
import styles from "./workspace-right-rail.module.css";

export type WorkspaceSourcesPaneProps = {
  activeViewerSourceId?: string | null;
  focusedSourceId?: string | null;
  loading: boolean;
  polling: boolean;
  uploading?: boolean;
  sources: WorkspaceSource[];
  selectedSourceIds: string[];
  transientProcessingSourceIds?: string[];
  urlSource: string;
  onAddUrlSource: () => Promise<boolean> | boolean;
  onDeleteSource: (sourceId: string) => void;
  onOpenSource: (sourceId: string) => void;
  onReindexSource: (sourceId: string) => void;
  onSelectAll: () => void;
  onSelectedSourceToggle: (sourceId: string) => void;
  onUploadFiles: (files: File[]) => Promise<void> | void;
  onUrlSourceChange: (value: string) => void;
  error: string;
};

const SUPPORTED_UPLOAD_ACCEPT = ".pdf,.doc,.docx,.ppt,.pptx,.xls,.xlsx,.txt,.md,.csv,.json,.toml,.yaml,.yml,.rst";
const SUPPORTED_UPLOAD_FORMATS = ["PDF", "DOC", "DOCX", "PPT", "PPTX", "XLS", "XLSX", "TXT", "MD", "CSV", "JSON", "TOML", "YAML", "YML", "RST"];
const PASTED_SOURCE_FILENAME = "pasted-source.txt";

function getStatusLabel(locale: "zh-CN" | "en", status: string) {
  switch (status) {
    case "pending":
    case "enqueueing":
    case "queued":
    case "processing":
    case "indexing":
    case "completed":
    case "ready":
    case "failed":
    case "error":
      return formatUiMessage(locale, `workspaceRightRail.sourceStatus.${status}`);
    default:
      return status;
  }
}

function isProcessingSourceStatus(status: string) {
  return (
    status === "pending" ||
    status === "enqueueing" ||
    status === "queued" ||
    status === "processing" ||
    status === "indexing"
  );
}

function isFailureSourceStatus(status: string) {
  return status === "failed" || status === "error";
}

export function WorkspaceSourcesPane({
  activeViewerSourceId = null,
  focusedSourceId,
  loading: _loading,
  polling: _polling,
  uploading = false,
  sources,
  selectedSourceIds,
  transientProcessingSourceIds = [],
  urlSource,
  onAddUrlSource,
  onDeleteSource,
  onOpenSource,
  onReindexSource,
  onSelectAll,
  onSelectedSourceToggle,
  onUploadFiles,
  onUrlSourceChange,
  error,
}: WorkspaceSourcesPaneProps) {
  const { locale } = useUiPreferences();
  const [showNewSourceDialog, setShowNewSourceDialog] = useState(false);
  const [activeDialogTab, setActiveDialogTab] = useState<"upload" | "link" | "paste">("upload");
  const [openMenuSourceId, setOpenMenuSourceId] = useState<string | null>(null);
  const [pasteContent, setPasteContent] = useState("");
  const openMenuRef = useRef<HTMLDivElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const allSelected = sources.length > 0 && selectedSourceIds.length === sources.length;
  const supportedUploadFormatsLabel =
    locale === "zh-CN" ? SUPPORTED_UPLOAD_FORMATS.join("、") : SUPPORTED_UPLOAD_FORMATS.join(" / ");
  const hasUrlSourceInput = urlSource
    .split(/\r?\n/)
    .some((value) => value.trim().length > 0);
  const hasPasteContent = pasteContent.trim().length > 0;

  async function handleFileSelection(files: File[]) {
    if (files.length === 0) {
      return;
    }

    await onUploadFiles(files);
    setShowNewSourceDialog(false);
  }

  async function handleFileInputChange(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);

    try {
      await handleFileSelection(files);
    } finally {
      event.target.value = "";
    }
  }

  async function handleFileDrop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();

    const files = Array.from(event.dataTransfer.files ?? []);

    if (uploading) {
      return;
    }

    await handleFileSelection(files);
  }

  useEffect(() => {
    if (!openMenuSourceId) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;

      if (openMenuRef.current?.contains(target)) {
        return;
      }

      setOpenMenuSourceId(null);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpenMenuSourceId(null);
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [openMenuSourceId]);

  return (
    <section className={`${styles.pane} ${styles.railPane}`} aria-label={formatUiMessage(locale, "workspaceRightRail.sourcesSectionTitle")}>
      <div className={styles.paneHeader}>
        <div className={styles.paneHeading}>
          <div className={styles.paneTitleRow}>
            <h2 className={styles.paneTitle}>{formatUiMessage(locale, "workspaceRightRail.sourcesSectionTitle")}</h2>
            <span className={styles.paneCount}>{sources.length}</span>
          </div>
        </div>
      </div>

      <div className={styles.sectionControls}>
        <button
          className={`${styles.sectionActionButton} ${styles.sectionActionButtonLight}`}
          type="button"
          onClick={() => {
            setActiveDialogTab("upload");
            setShowNewSourceDialog(true);
          }}
        >
          <svg aria-hidden="true" className={styles.sectionActionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="M12 5v14M5 12h14" strokeLinecap="round" strokeWidth="1.8" />
          </svg>
          <span>{formatUiMessage(locale, "workspaceRightRail.newSourceAction")}</span>
        </button>

        <div className={styles.selectionRow}>
          <span className={styles.selectionLabel}>{formatUiMessage(locale, "workspaceRightRail.selectAllAction")}</span>
          <button
            aria-label={
              allSelected
                ? formatUiMessage(locale, "workspaceRightRail.clearSelectionAction")
                : formatUiMessage(locale, "workspaceRightRail.selectAllAction")
            }
            aria-pressed={allSelected}
            className={`${styles.selectionCheckbox}${allSelected ? ` ${styles.selectionCheckboxChecked}` : ""}`}
            onClick={onSelectAll}
            type="button"
          >
            {allSelected ? (
              <svg aria-hidden="true" className={styles.selectionCheckboxIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path d="m6 12.75 4 4 8-9" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" />
              </svg>
            ) : null}
          </button>
        </div>

        {error ? <div className={styles.error}>{error}</div> : null}
      </div>

      <div className={styles.sectionScroller}>
        {sources.length === 0 ? (
          <div className={styles.emptyStateBlock}>
            <div className={styles.emptyStateTitle}>{formatUiMessage(locale, "workspaceRightRail.emptySourcesTitle")}</div>
            <div className={styles.emptyState}>{formatUiMessage(locale, "workspaceRightRail.emptySourcesBody")}</div>
          </div>
        ) : (
          <ul aria-label={formatUiMessage(locale, "workspaceRightRail.sourcesListLabel")} className={styles.list}>
            {sources.map((source) => {
              const selected = selectedSourceIds.includes(source.id);
              const focused = focusedSourceId === source.id;
              const viewerOpen = activeViewerSourceId === source.id;
              const menuOpen = openMenuSourceId === source.id;
              const visualStatus = transientProcessingSourceIds.includes(source.id) ? "processing" : source.status;
              const processing = isProcessingSourceStatus(visualStatus);
              const failed = isFailureSourceStatus(visualStatus);
              const selectable = !processing && !failed;
              const showStatus = visualStatus !== "ready" && visualStatus !== "completed";
              const statusLabel = showStatus ? getStatusLabel(locale, visualStatus) : "";
              const showStatusText =
                showStatus &&
                !processing &&
                !failed;

              return (
                <li
                  className={[
                    styles.listItem,
                    styles.sourceListItem,
                    selected ? styles.listItemSelected : "",
                    focused || viewerOpen ? styles.listItemFocused : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  data-testid="ingestion-status"
                  data-document-id={source.id}
                  data-status={visualStatus}
                  key={source.id}
                  title={showStatus ? statusLabel : undefined}
                >
                  <div className={styles.listItemTopRow}>
                    <button
                      aria-pressed={selected}
                      className={styles.sourceToggleButton}
                      disabled={!selectable}
                      onClick={() => {
                        if (selectable) {
                          onSelectedSourceToggle(source.id);
                        }
                      }}
                      type="button"
                    >
                      <span
                        className={`${styles.selectionMark}${selected ? ` ${styles.selectionMarkChecked}` : ""}`}
                        aria-hidden="true"
                      >
                        {processing ? (
                          <span className={styles.sourceStatusSpinner} />
                        ) : failed ? (
                          <svg className={styles.sourceStatusErrorIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path d="M7 7l10 10M17 7 7 17" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.4" />
                          </svg>
                        ) : selected ? (
                          <svg className={styles.selectionMarkIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path d="m6 12.75 4 4 8-9" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" />
                          </svg>
                        ) : null}
                      </span>
                    </button>

                    <button
                      className={styles.sourceOpenButton}
                      type="button"
                      onClick={() => onOpenSource(source.id)}
                    >
                      <span className={styles.listItemTitleText}>{source.file_name}</span>
                    </button>

                    <div className={styles.itemMenuAnchor} ref={menuOpen ? openMenuRef : null}>
                      <button
                        aria-expanded={menuOpen}
                        aria-haspopup="menu"
                        aria-label={formatUiMessage(locale, "workspaceRightRail.sourceActionsLabel")}
                        className={styles.itemMenuTrigger}
                        type="button"
                        onClick={() => setOpenMenuSourceId((current) => (current === source.id ? null : source.id))}
                      >
                        <svg aria-hidden="true" className={styles.itemMenuIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path d="M12 6.75a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5ZM12 13.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5ZM12 19.75a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5Z" fill="currentColor" stroke="none" />
                        </svg>
                      </button>

                      {menuOpen ? (
                        <div className={styles.itemMenu} role="menu">
                          <button
                            className={styles.itemMenuButton}
                            role="menuitem"
                            type="button"
                            onClick={() => {
                              setOpenMenuSourceId(null);
                              onOpenSource(source.id);
                            }}
                          >
                            {viewerOpen
                              ? formatUiMessage(locale, "workspaceRightRail.hidePreviewAction")
                              : formatUiMessage(locale, "workspaceRightRail.openSourceAction")}
                          </button>
                          <button
                            className={styles.itemMenuButton}
                            role="menuitem"
                            type="button"
                            onClick={() => {
                              setOpenMenuSourceId(null);
                              onReindexSource(source.id);
                            }}
                          >
                            {formatUiMessage(locale, "workspaceRightRail.reindexAction")}
                          </button>
                          <button
                            className={`${styles.itemMenuButton} ${styles.itemMenuButtonDanger}`}
                            role="menuitem"
                            type="button"
                            onClick={() => {
                              setOpenMenuSourceId(null);
                              onDeleteSource(source.id);
                            }}
                          >
                            {formatUiMessage(locale, "workspaceRightRail.deleteSourceAction")}
                          </button>
                        </div>
                      ) : null}
                    </div>
                  </div>

                  {showStatusText || viewerOpen ? (
                    <div className={styles.listItemMetaRow}>
                      {showStatusText ? <span className={styles.statusBadge}>{statusLabel}</span> : null}
                      {viewerOpen ? (
                        <span className={styles.inlineBadge}>
                          {formatUiMessage(locale, "workspaceRightRail.viewerSectionTitle")}
                        </span>
                      ) : null}
                    </div>
                  ) : null}
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {showNewSourceDialog ? (
        <div className={styles.sourceDialogBackdrop} role="presentation">
          <div
            aria-label={formatUiMessage(locale, "workspaceRightRail.addSourceTitle")}
            aria-modal="true"
            className={styles.sourceDialog}
            role="dialog"
          >
            <div className={styles.dialogHeader}>
              <div className={styles.paneHeading}>
                <h3 className={styles.paneTitle}>{formatUiMessage(locale, "workspaceRightRail.addSourceTitle")}</h3>
                <p className={styles.paneSubtitle}>{formatUiMessage(locale, "workspaceRightRail.addSourceSubtitle")}</p>
              </div>
              <button className={styles.closeButton} type="button" onClick={() => setShowNewSourceDialog(false)}>
                {formatUiMessage(locale, "workspaceRightRail.closeViewerAction")}
              </button>
            </div>

            <div className={styles.sourceTabs} role="tablist" aria-label={formatUiMessage(locale, "workspaceRightRail.addSourceTitle")}>
              {(
                [
                  ["upload", formatUiMessage(locale, "workspaceRightRail.uploadFileTab")],
                  ["link", formatUiMessage(locale, "workspaceRightRail.webLinkTab")],
                  ["paste", formatUiMessage(locale, "workspaceRightRail.pasteTextTab")],
                ] as const
              ).map(([tab, label]) => (
                <button
                  key={tab}
                  aria-selected={activeDialogTab === tab}
                  className={`${styles.sourceTab}${activeDialogTab === tab ? ` ${styles.sourceTabActive}` : ""}`}
                  role="tab"
                  type="button"
                  onClick={() => setActiveDialogTab(tab)}
                >
                  {label}
                </button>
              ))}
            </div>

            {activeDialogTab === "upload" ? (
              <div
                className={styles.uploadWell}
                onDragOver={(event) => event.preventDefault()}
                onDrop={(event) => {
                  void handleFileDrop(event);
                }}
              >
                <input
                  accept={SUPPORTED_UPLOAD_ACCEPT}
                  multiple
                  onChange={(event) => {
                    void handleFileInputChange(event);
                  }}
                  ref={fileInputRef}
                  style={{ display: "none" }}
                  type="file"
                />
                <div className={styles.uploadIcon} aria-hidden="true">
                  <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      d="M12 16V6m0 0 4 4m-4-4-4 4M5.5 17.5v.5A1.5 1.5 0 0 0 7 19.5h10a1.5 1.5 0 0 0 1.5-1.5v-.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth="1.8"
                    />
                  </svg>
                </div>
                <div className={styles.uploadTitle}>{formatUiMessage(locale, "workspaceRightRail.uploadDropTitle")}</div>
                <div className={styles.emptyState}>
                  {formatUiMessage(locale, "workspaceRightRail.uploadDropBody", {
                    formats: supportedUploadFormatsLabel,
                  })}
                </div>
                <button
                  className={styles.button}
                  disabled={uploading}
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                >
                  {formatUiMessage(locale, "workspaceRightRail.browseFilesAction")}
                </button>
              </div>
            ) : null}

            {activeDialogTab === "link" ? (
              <div className={styles.dialogFields}>
                <label className={styles.fieldLabel} htmlFor="workspace-source-url">
                  {formatUiMessage(locale, "workspaceRightRail.sourceUrlLabel")}
                </label>
                <textarea
                  className={styles.textarea}
                  id="workspace-source-url"
                  onChange={(event) => onUrlSourceChange(event.target.value)}
                  placeholder={formatUiMessage(locale, "workspaceRightRail.sourceUrlPlaceholder")}
                  value={urlSource}
                />
                <button
                  className={`${styles.button} ${styles.buttonPrimary}`}
                  disabled={!hasUrlSourceInput}
                  type="button"
                  onClick={async () => {
                    const added = await onAddUrlSource();

                    if (added) {
                      setShowNewSourceDialog(false);
                    }
                  }}
                >
                  {formatUiMessage(locale, "workspaceRightRail.addLinkAction")}
                </button>
              </div>
            ) : null}

            {activeDialogTab === "paste" ? (
              <div className={styles.dialogFields}>
                <div className={styles.fieldRow}>
                  <label className={styles.fieldLabel} htmlFor="workspace-source-paste-content">
                    {formatUiMessage(locale, "workspaceRightRail.pasteContentLabel")}
                  </label>
                  <textarea
                    className={styles.textarea}
                    id="workspace-source-paste-content"
                    value={pasteContent}
                    onChange={(event) => setPasteContent(event.target.value)}
                  />
                </div>
                <button
                  className={`${styles.button} ${styles.buttonPrimary}`}
                  disabled={!hasPasteContent || uploading}
                  type="button"
                  onClick={async () => {
                    const pastedSourceFile = new File([pasteContent], PASTED_SOURCE_FILENAME, {
                      type: "text/plain",
                    });

                    await onUploadFiles([pastedSourceFile]);
                    setPasteContent("");
                    setShowNewSourceDialog(false);
                  }}
                >
                  {formatUiMessage(locale, "workspaceRightRail.saveAsSourceAction")}
                </button>
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </section>
  );
}
