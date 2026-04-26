"use client";

import { type CSSProperties, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { lookupWorkspaceCitation } from "../../lib/workspace/client";
import type { WorkspaceCitationRequest } from "../../lib/workspace/model";
import type { Citation } from "../../lib/workspace/stream";
import styles from "./workspace-shell.module.css";

type WorkspaceCitationModalProps = {
  citationRequest: WorkspaceCitationRequest | null;
  workspaceId: string;
  onClose: () => void;
};

const CITATION_POPOVER_OFFSET = 10;
const CITATION_POPOVER_PADDING = 12;
const CITATION_POPOVER_MAX_WIDTH = 520;
const CITATION_POPOVER_MAX_HEIGHT = 420;

function mergeCitationDetail(
  citation: Citation,
  detail: Partial<{
    asset_id: string | null;
    caption: string | null;
    chunk_id: string | null;
    chunk_type: string | null;
    content: string | null;
    doc_id: string | null;
    doc_name: string | null;
    image_url: string | null;
    page: number | null;
  }>,
): Citation {
  return {
    ...citation,
    doc_name: detail.doc_name ?? citation.doc_name,
    content: detail.content ?? citation.content,
    doc_id: detail.doc_id ?? citation.doc_id,
    chunk_id: detail.chunk_id ?? citation.chunk_id,
    page: detail.page ?? citation.page,
    chunk_type: detail.chunk_type ?? citation.chunk_type,
    asset_id: detail.asset_id ?? citation.asset_id,
    caption: detail.caption ?? citation.caption,
    image_url: detail.image_url ?? citation.image_url,
  };
}

export function WorkspaceCitationModal({
  citationRequest,
  workspaceId: _workspaceId,
  onClose,
}: WorkspaceCitationModalProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const [detail, setDetail] = useState<Partial<{
    asset_id: string | null;
    caption: string | null;
    chunk_id: string | null;
    chunk_type: string | null;
    content: string | null;
    doc_id: string | null;
    doc_name: string | null;
    image_url: string | null;
    page: number | null;
  }> | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [popoverStyle, setPopoverStyle] = useState<CSSProperties | null>(null);

  useEffect(() => {
    if (!citationRequest) {
      return;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [citationRequest, onClose]);

  useEffect(() => {
    if (!citationRequest || !auth.token) {
      setDetail(null);
      setLoading(false);
      setError("");
      return;
    }

    let cancelled = false;
    setDetail(null);
    setLoading(true);
    setError("");

    void lookupWorkspaceCitation(auth.token, {
      session_id: citationRequest.session_id,
      message_id: citationRequest.message_id,
      citation_id: citationRequest.citation.citation_id,
    })
      .then((response) => {
        if (cancelled) {
          return;
        }

        setDetail(response);
      })
      .catch(() => {
        if (!cancelled) {
          setError(formatUiMessage(locale, "workspaceCitation.error"));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [auth.token, citationRequest, locale]);

  const citation = useMemo(() => {
    if (!citationRequest) {
      return null;
    }

    return mergeCitationDetail(citationRequest.citation, detail ?? {});
  }, [citationRequest, detail]);
  const chunkText = citation?.content?.trim() || citation?.preview?.trim() || "";
  const popoverText =
    loading
      ? formatUiMessage(locale, "workspaceCitation.loading")
      : error || chunkText || formatUiMessage(locale, "workspaceCitation.empty");

  useLayoutEffect(() => {
    if (!citationRequest || typeof window === "undefined") {
      setPopoverStyle(null);
      return;
    }

    function updatePosition() {
      const popover = popoverRef.current;
      const width = Math.min(CITATION_POPOVER_MAX_WIDTH, window.innerWidth - CITATION_POPOVER_PADDING * 2);
      const maxHeight = Math.min(CITATION_POPOVER_MAX_HEIGHT, window.innerHeight - CITATION_POPOVER_PADDING * 2);
      const measuredHeight = popover?.offsetHeight ?? Math.min(280, maxHeight);
      const height = Math.min(measuredHeight, maxHeight);
      const anchorRect = citationRequest?.anchorRect ?? null;

      let left = (window.innerWidth - width) / 2;
      let top = (window.innerHeight - height) / 2;

      if (anchorRect) {
        left = anchorRect.left + anchorRect.width / 2 - width / 2;
        left = Math.min(
          window.innerWidth - width - CITATION_POPOVER_PADDING,
          Math.max(CITATION_POPOVER_PADDING, left),
        );

        const belowTop = anchorRect.bottom + CITATION_POPOVER_OFFSET;
        const aboveTop = anchorRect.top - height - CITATION_POPOVER_OFFSET;

        if (belowTop + height + CITATION_POPOVER_PADDING <= window.innerHeight || aboveTop < CITATION_POPOVER_PADDING) {
          top = Math.min(
            window.innerHeight - height - CITATION_POPOVER_PADDING,
            Math.max(CITATION_POPOVER_PADDING, belowTop),
          );
        } else {
          top = Math.max(CITATION_POPOVER_PADDING, aboveTop);
        }
      }

      setPopoverStyle({
        top: `${Math.round(top)}px`,
        left: `${Math.round(left)}px`,
        width: `${Math.round(width)}px`,
        maxHeight: `${Math.round(maxHeight)}px`,
      });
    }

    updatePosition();
    window.addEventListener("resize", updatePosition);

    return () => {
      window.removeEventListener("resize", updatePosition);
    };
  }, [citationRequest, popoverText]);

  if (!citationRequest || !citation) {
    return null;
  }

  return (
    <div className={`${styles.modalBackdrop} ${styles.citationPopoverBackdrop}`} onClick={onClose}>
      <div
        aria-label={formatUiMessage(locale, "workspaceCitation.dialogLabel")}
        className={`${styles.modalCard} ${styles.citationPopoverCard}`}
        onClick={(event) => event.stopPropagation()}
        ref={popoverRef}
        role="dialog"
        style={popoverStyle ?? undefined}
      >
        <div className={styles.citationPopoverContent}>{popoverText}</div>
      </div>
    </div>
  );
}
