"use client";

import { useEffect, useMemo, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { lookupWorkspaceCitation } from "../../lib/workspace/client";
import type { WorkspaceCitationRequest } from "../../lib/workspace/model";
import type { Citation } from "../../lib/workspace/stream";
import { AppModal } from "../ui/app-modal";

type WorkspaceCitationModalProps = {
  citationRequest: WorkspaceCitationRequest | null;
  workspaceId: string;
  onClose: () => void;
};

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

/**
 * W5 #17: centered citation dialog (anchor-positioned popover retired).
 */
export function WorkspaceCitationModal({
  citationRequest,
  workspaceId: _workspaceId,
  onClose,
}: WorkspaceCitationModalProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
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
  const bodyText = loading
    ? formatUiMessage(locale, "workspaceCitation.loading")
    : error || chunkText || formatUiMessage(locale, "workspaceCitation.empty");

  if (!citationRequest || !citation) {
    return null;
  }

  const title =
    citation.doc_name?.trim() ||
    formatUiMessage(locale, "workspaceCitation.dialogLabel");
  const pageLabel =
    citation.page !== null && citation.page !== undefined
      ? formatUiMessage(locale, "workspaceRightRail.viewerPage", {
          page: String(citation.page),
        })
      : null;

  return (
    <AppModal
      open
      size="md"
      title={title}
      closeLabel={formatUiMessage(locale, "appModal.close")}
      testId="workspace-citation-modal"
      onClose={onClose}
    >
      {pageLabel ? (
        <p style={{ margin: "0 0 0.75rem", color: "hsl(var(--muted-foreground))", fontSize: "0.85rem" }}>
          {pageLabel}
        </p>
      ) : null}
      <div
        data-testid="workspace-citation-body"
        style={{
          whiteSpace: "pre-wrap",
          lineHeight: 1.55,
          fontSize: "0.95rem",
          color: "hsl(var(--foreground))",
        }}
      >
        {bodyText}
      </div>
    </AppModal>
  );
}
