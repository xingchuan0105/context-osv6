'use client';

import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TraceEvent } from '@/lib/chat-trace';
import type { RAGTraceSummary } from '@/types';

interface ChatTracePanelProps {
  events: TraceEvent[];
  ragTrace?: RAGTraceSummary | null;
  loading?: boolean;
}

function formatTime(ms: number): string {
  const date = new Date(ms);
  return date.toLocaleTimeString();
}

function statusClass(status: TraceEvent['status']) {
  switch (status) {
    case 'start':
      return 'bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-200';
    case 'done':
      return 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-200';
    case 'error':
      return 'bg-rose-100 text-rose-700 dark:bg-rose-900/40 dark:text-rose-200';
    default:
      return 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-200';
  }
}

function stageLabel(stage: string): string {
  return stage.replace(/\./g, ' › ');
}

export function ChatTracePanel({ events, ragTrace, loading }: ChatTracePanelProps) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);

  const grouped = useMemo(() => {
    return [...events].sort((a, b) => {
      if (a.trace_id !== b.trace_id) return a.trace_id.localeCompare(b.trace_id);
      if (a.turn_id !== b.turn_id) return a.turn_id.localeCompare(b.turn_id);
      return a.seq - b.seq;
    });
  }, [events]);

  return (
    <div className="border-t border-border bg-card/60">
      <div className="px-3 md:px-4 py-2 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-sm md:text-base font-medium">
          <span>{t('chat.traceTitle')}</span>
          <span className="text-xs text-muted-foreground">{grouped.length}</span>
          {loading && <span className="text-xs text-muted-foreground">{t('chat.traceStreaming')}</span>}
        </div>
        <button
          type="button"
          onClick={() => setCollapsed((prev) => !prev)}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          {collapsed ? t('chat.traceExpand') : t('chat.traceCollapse')}
        </button>
      </div>

      {!collapsed && (
        <div className="max-h-52 overflow-auto px-3 md:px-4 pb-3 space-y-2">
          {ragTrace && (
            <div className="rounded-md border border-border/70 bg-background/80 px-3 py-2.5 space-y-2">
              <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <span className="font-medium text-foreground">{t('chat.traceRagSummary')}</span>
                <span>{t('chat.traceItems', { count: ragTrace.item_count || 0 })}</span>
                <span>{t('chat.traceBudget', { count: ragTrace.total_candidate_budget || 0 })}</span>
                <span>{t('chat.traceTopKReturned', { count: ragTrace.top_k_returned || 0 })}</span>
                {ragTrace.summary_mode && <span>{t('chat.traceSummaryMode', { mode: ragTrace.summary_mode })}</span>}
              </div>

              {Array.isArray(ragTrace.items) && ragTrace.items.length > 0 && (
                <div className="space-y-1.5">
                  {ragTrace.items.map((item, index) => (
                    <div
                      key={`${item.item_type}-${item.priority}-${index}`}
                      className="rounded-md border border-border/60 bg-card/70 px-2 py-1.5"
                    >
                      <div className="flex flex-wrap items-center gap-2 text-xs">
                        <span className="font-medium text-foreground">
                          {t('chat.traceItemLabel', { index: index + 1 })}
                        </span>
                        <span className="text-muted-foreground">{item.item_type}</span>
                        <span className="text-muted-foreground">{item.retrieval_mode}</span>
                        <span className="text-muted-foreground">{t('chat.traceHits', { count: item.source_count || 0 })}</span>
                      </div>
                      <div className="mt-1 text-xs text-muted-foreground whitespace-pre-wrap break-words">
                        {item.purpose}
                      </div>
                      <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                        <span>{t('chat.traceRecallBudget', { count: item.recall_budget || 0 })}</span>
                        <span>{t('chat.traceBm25K', { count: item.bm25_k || 0 })}</span>
                        <span>{t('chat.traceDenseK', { count: item.dense_k || 0 })}</span>
                        <span>{t('chat.traceRerankBudget', { count: item.rerank_budget || 0 })}</span>
                      </div>
                      {item.query && (
                        <div className="mt-1 text-[11px] text-muted-foreground whitespace-pre-wrap break-words">
                          {item.query}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
          {grouped.length === 0 ? (
            <div className="text-xs text-muted-foreground">{t('chat.traceEmpty')}</div>
          ) : (
            grouped.map((event) => (
              <div
                key={`${event.trace_id}:${event.turn_id}:${event.seq}`}
                className="rounded-md border border-border/70 bg-background/80 px-2 py-1.5"
              >
                <div className="flex items-center gap-2 text-xs">
                  <span className="text-muted-foreground">#{event.seq}</span>
                  <span className="text-muted-foreground">{formatTime(event.timestamp)}</span>
                  <span className={`px-1.5 py-0.5 rounded ${statusClass(event.status)}`}>{event.status}</span>
                  {event.agent_name && <span className="text-muted-foreground">{event.agent_name}</span>}
                </div>
                <div className="text-sm font-medium mt-1">{stageLabel(event.stage)}</div>
                {event.message && <div className="text-xs text-muted-foreground mt-0.5">{event.message}</div>}
                {event.data && (
                  <pre className="mt-1 text-xs text-muted-foreground whitespace-pre-wrap break-words leading-relaxed">
                    {JSON.stringify(event.data, null, 2)}
                  </pre>
                )}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
