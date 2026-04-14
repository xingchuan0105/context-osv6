'use client';

import { Fragment, useMemo, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, FileText, Loader2, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { chatApi } from '@/lib/api/client';
import type { InlineStatusLine } from '@/lib/chat-trace';
import type { ChatMessage, Citation } from '@/types';

interface ChatBubbleProps {
  message: ChatMessage;
  agentName?: string;
  agentIcon?: string;
  statusLine?: InlineStatusLine;
  onExtract?: (content: string) => void;
}

const bubbleColors = {
  user:
    'border border-primary/25 bg-[linear-gradient(180deg,rgba(124,58,237,0.18),rgba(124,58,237,0.08))] text-[color:var(--text-primary)] shadow-[var(--shadow-sm)] backdrop-blur-sm',
  general:
    'border border-border bg-card/92 text-[color:var(--text-primary)] shadow-[var(--shadow-sm)] backdrop-blur-sm',
  knowledge_base:
    'border border-border bg-card/92 text-[color:var(--text-primary)] shadow-[var(--shadow-sm)] backdrop-blur-sm',
  search:
    'border border-border bg-card/92 text-[color:var(--text-primary)] shadow-[var(--shadow-sm)] backdrop-blur-sm',
  fallback:
    'border border-border bg-card/92 text-[color:var(--text-primary)] shadow-[var(--shadow-sm)] backdrop-blur-sm',
};

function getAgentColorClass(agentId?: string, isUser?: boolean): string {
  if (isUser) return bubbleColors.user;

  switch (agentId) {
    case 'general':
      return bubbleColors.general;
    case 'knowledge_base':
      return bubbleColors.knowledge_base;
    case 'search':
      return bubbleColors.search;
    default:
      return bubbleColors.fallback;
  }
}

function getAgentInfo(agentId?: string, agentName?: string, agentIcon?: string) {
  const agents: Record<string, { name: string; icon: string; label: string }> = {
    general: { name: '通用助手', icon: '💬', label: 'General' },
    knowledge_base: { name: '知识库助手', icon: '📚', label: 'Knowledge' },
    search: { name: '搜索助手', icon: '🔍', label: 'Search' },
  };

  const info = agentId ? agents[agentId] : null;
  return {
    name: agentName || info?.name || '助手',
    icon: agentIcon || info?.icon || '🤖',
    label: info?.label || 'Assistant',
  };
}

function getAgentBadgeClass(agentId?: string) {
  switch (agentId) {
    case 'general':
      return 'badge-general';
    case 'knowledge_base':
      return 'badge-doc';
    case 'search':
      return 'badge-search';
    default:
      return 'badge-general';
  }
}

function statusToneClass(tone: InlineStatusLine['tone']): string {
  switch (tone) {
    case 'done':
      return 'text-emerald-700 dark:text-emerald-300';
    case 'error':
      return 'text-rose-700 dark:text-rose-300';
    default:
      return 'text-foreground/70';
  }
}

export function ChatBubble({ message, agentName, agentIcon, statusLine, onExtract }: ChatBubbleProps) {
  const { t } = useTranslation();
  const [lookupLoadingId, setLookupLoadingId] = useState<number | null>(null);
  const [lookupOpen, setLookupOpen] = useState(false);
  const [lookupTitle, setLookupTitle] = useState('');
  const [lookupText, setLookupText] = useState('');
  const [lookupImageUrl, setLookupImageUrl] = useState('');
  const [lookupCaption, setLookupCaption] = useState('');

  const isUser = message.role === 'user';
  const agentInfo = getAgentInfo(message.agent_id, agentName, agentIcon);
  const badgeClass = getAgentBadgeClass(message.agent_id);

  const citations: Citation[] = useMemo(() => {
    if (!message.citations) return [];
    if (Array.isArray(message.citations)) return message.citations;
    try {
      return JSON.parse(String(message.citations));
    } catch {
      return [];
    }
  }, [message.citations]);
  const citationByDisplayId = useMemo(() => {
    const next = new Map<number, Citation>();
    citations.forEach((citation, index) => {
      const citationId = Number(citation.citation_id || index + 1);
      next.set(citationId, citation);
    });
    return next;
  }, [citations]);

  const handleCopy = () => {
    navigator.clipboard.writeText(message.content);
  };

  const handleCitationClick = async (citation: Citation) => {
    // For search results with an explicit URL, jump directly to the webpage
    if (citation.layer === 'search' && (citation.doc_id?.startsWith('http') || citation.asset_id?.startsWith('http'))) {
      const url = citation.doc_id?.startsWith('http') ? citation.doc_id : citation.asset_id;
      if (url) {
        window.open(url, '_blank', 'noopener,noreferrer');
        return;
      }
    }

    const citationId = Number(citation.citation_id || 0);
    // If it's a simple search citation with a doc_name that looks like a URL, try opening it
    if (citation.doc_name?.startsWith('http')) {
      window.open(citation.doc_name, '_blank', 'noopener,noreferrer');
      return;
    }

    if (!citationId || !message.session_id || !message.id) {
      // Fallback for citations that might not have server-side lookup (e.g. search agents)
      setLookupTitle(citation.doc_name || t('chat.citationTitle', { id: '?' }));
      setLookupText(citation.preview || citation.content || t('chat.noCitationText'));
      setLookupImageUrl(citation.image_url || '');
      setLookupCaption(citation.caption || '');
      setLookupOpen(true);
      return;
    }

    setLookupLoadingId(citationId);
    try {
      const response = await chatApi.lookupCitation(message.session_id, Number(message.id), citationId);
      if (!response?.success || !response?.data) {
        throw new Error(response?.error || 'lookup failed');
      }

      // If lookup returns a URL for a search result, open it
      if (response.data.doc_id?.startsWith('http') || response.data.asset_id?.startsWith('http')) {
         const url = response.data.doc_id?.startsWith('http') ? response.data.doc_id : response.data.asset_id;
         if (url) {
           window.open(url, '_blank', 'noopener,noreferrer');
           return;
         }
      }

      setLookupTitle(response.data.doc_name || t('chat.citationTitle', { id: citationId }));
      setLookupText(response.data.content || t('chat.noCitationText'));
      setLookupImageUrl(response.data.image_url || citation.image_url || '');
      setLookupCaption(response.data.caption || citation.caption || '');
      setLookupOpen(true);
    } catch {
      setLookupTitle(citation.doc_name || t('chat.citationTitle', { id: citationId }));
      setLookupText(citation.preview || t('chat.citationLookupFailed'));
      setLookupImageUrl(citation.image_url || '');
      setLookupCaption(citation.caption || '');
      setLookupOpen(true);
    } finally {
      setLookupLoadingId(null);
    }
  };

  const bubbleColorClass = getAgentColorClass(message.agent_id, isUser);

  const renderInlineText = (text: string) => {
    const nodes: ReactNode[] = [];
    // Support both [[1]] (standard RAG) and [1] (often from Search/Perplexity style)
    const tokenPattern = /\[\[(\d+)\]\]|\[(\d+)\]/g;
    let lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = tokenPattern.exec(text)) !== null) {
      if (match.index > lastIndex) {
        nodes.push(
          <Fragment key={`text-${lastIndex}`}>{text.slice(lastIndex, match.index)}</Fragment>
        );
      }
      const displayId = Number(match[1] || match[2]);
      const citation = citationByDisplayId.get(displayId);
      nodes.push(
        <button
          key={`cite-${displayId}-${match.index}`}
          onClick={() => citation && void handleCitationClick(citation)}
          className="mx-0.5 rounded-md border border-border bg-background/55 px-1.5 py-0.5 text-[11px] text-muted-foreground hover:bg-accent hover:text-foreground transition-colors align-middle"
        >
          {`[${displayId}]`}
        </button>
      );
      lastIndex = match.index + match[0].length;
    }
    if (lastIndex < text.length) {
      nodes.push(<Fragment key={`text-tail-${lastIndex}`}>{text.slice(lastIndex)}</Fragment>);
    }
    return nodes;
  };

  const renderMessageContent = () => {
    const lines = String(message.content || '').split('\n');
    return (
      <div className="space-y-3 text-[15px] leading-[1.75] text-[color:var(--text-primary)]">
        {lines.map((line, index) => {
          const imageMatch = line.trim().match(/^\[\[image:(\d+)\]\]$/);
          if (imageMatch) {
            const displayId = Number(imageMatch[1]);
            const citation = citationByDisplayId.get(displayId);
            if (citation?.image_url) {
              return (
                <div key={`image-${displayId}-${index}`} className="rounded-2xl border border-border/70 bg-background/35 p-2">
                  <img
                    src={citation.image_url}
                    alt={citation.caption || citation.doc_name || `citation ${displayId}`}
                    className="max-h-96 w-auto max-w-full rounded-xl object-contain"
                  />
                  {(citation.caption || citation.doc_name) && (
                    <div className="mt-2 text-sm text-muted-foreground whitespace-pre-wrap">
                      {citation.caption || citation.doc_name}
                    </div>
                  )}
                </div>
              );
            }
            return (
              <div key={`image-fallback-${displayId}-${index}`} className="text-sm text-muted-foreground">
                {`[image ${displayId}]`}
              </div>
            );
          }

          if (line.trim() === '') {
            return <div key={`spacer-${index}`} className="h-2" />;
          }

          return (
            <p key={`line-${index}`} className="whitespace-pre-wrap break-words">
              {renderInlineText(line)}
            </p>
          );
        })}
      </div>
    );
  };

  return (
    <>
      <div className={`flex gap-3 mb-4 ${isUser ? 'flex-row-reverse' : ''}`}>
        <div className={`flex-1 max-w-[85%] ${isUser ? 'items-end' : 'items-start'} flex flex-col`}>
          {!isUser && (
            <div className="flex items-center gap-2 mb-1.5 px-1">
              <span className={`text-[11px] font-medium px-2.5 py-1 rounded-md ${badgeClass}`}>
                {agentInfo.icon} {agentInfo.label}
              </span>
              <span className="text-sm font-medium text-muted-foreground">{agentInfo.name}</span>
            </div>
          )}

          {!isUser && statusLine?.text && (
            <div
              data-testid="assistant-inline-status"
              className={`mb-1.5 px-1 text-sm flex items-center gap-1.5 ${statusToneClass(statusLine.tone)}`}
            >
              {statusLine.live && statusLine.tone === 'progress' ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <span className="w-1.5 h-1.5 rounded-full bg-current opacity-80" />
              )}
              <span className="truncate">{statusLine.text}</span>
            </div>
          )}

          <div
            className={`rounded-3xl px-4 py-3.5 animate-in fade-in slide-in-from-bottom-2 duration-300 ${bubbleColorClass}`}
          >
            {renderMessageContent()}

            {!isUser && citations.length > 0 && (
              <div className="mt-3 pt-3 border-t border-white/8">
                <div className="text-xs text-muted-foreground mb-2 uppercase tracking-[0.18em]">{t('chat.citationMarker')}</div>
                <div className="flex flex-wrap gap-2">
                  {citations.map((citation, index) => {
                    const citationId = Number(citation.citation_id || 0);
                    const displayId = citationId > 0 ? citationId : index + 1;
                    const hoverPreview = (citation.preview || citation.content || '').trim();
                    return (
                      <div key={`${message.id}-${displayId}-${index}`} className="group relative">
                        <button
                          onClick={() => void handleCitationClick(citation)}
                          className="px-2.5 py-1 rounded-lg border border-border bg-background/40 text-xs text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                          title={hoverPreview || citation.doc_name || t('chat.citationTitle', { id: displayId })}
                        >
                          {lookupLoadingId === citationId && citationId > 0 ? (
                            <Loader2 className="w-3 h-3 animate-spin" />
                          ) : (
                            `[[${displayId}]]`
                          )}
                        </button>
                        {hoverPreview && (
                          <div className="pointer-events-none absolute left-0 top-full z-20 mt-2 hidden w-80 rounded-xl border border-border bg-card/95 p-2.5 text-xs leading-5 text-foreground shadow-lg group-hover:block">
                            <div className="font-medium text-[11px] text-muted-foreground mb-1">
                              {citation.doc_name || t('chat.citationTitle', { id: displayId })}
                            </div>
                            <div className="max-h-40 overflow-auto whitespace-pre-wrap break-words">
                              {hoverPreview}
                            </div>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </div>

          {!isUser && (
            <div className="flex items-center gap-1 mt-1.5 px-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={handleCopy}
                className="h-7 px-2.5 text-xs text-muted-foreground hover:text-foreground"
              >
                <Copy className="w-3 h-3 mr-1" />
                {t('chat.copy')}
              </Button>
              {onExtract && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => onExtract(message.content)}
                  className="h-7 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                >
                  <FileText className="w-3 h-3 mr-1" />
                  {t('chat.extract')}
                </Button>
              )}
            </div>
          )}
        </div>
      </div>

      {lookupOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-200">
          <div className="w-full max-w-2xl surface-glass rounded-3xl animate-in zoom-in-95 slide-in-from-bottom-4 duration-300">
            <div className="p-4 border-b border-border flex items-center justify-between gap-2">
              <div>
                <h3 className="text-base font-semibold">{t('chat.citationDetail')}</h3>
                <p className="text-xs text-muted-foreground mt-0.5">{lookupTitle}</p>
              </div>
              <button
                onClick={() => setLookupOpen(false)}
                className="p-2 rounded-xl hover:bg-accent transition-colors"
                title={t('chat.close')}
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-4 max-h-[60vh] overflow-auto">
              {lookupImageUrl && (
                <div className="mb-4">
                  <img
                    src={lookupImageUrl}
                    alt={lookupCaption || lookupTitle}
                    className="max-h-80 w-auto max-w-full rounded-2xl border border-border bg-muted object-contain"
                  />
                  {lookupCaption && (
                    <div className="mt-2 text-sm text-muted-foreground whitespace-pre-wrap">{lookupCaption}</div>
                  )}
                </div>
              )}
              <pre className="text-base whitespace-pre-wrap break-words leading-7">{lookupText}</pre>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
