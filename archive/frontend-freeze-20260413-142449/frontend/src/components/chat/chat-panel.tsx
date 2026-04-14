/**
 * ChatPanel - AI 对话面板组件
 */

'use client';

import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { ChatBubble } from './chat-bubble';
import { ChatInput } from './chat-input';
import { ChatTracePanel } from './chat-trace-panel';
import { useAppStore } from '@/stores/useAppStore';
import { agentsApi, chatApi, getAuthToken, notesApi } from '@/lib/api/client';
import { mergeAgentsWithBuiltins } from '@/lib/agents';
import { appendTraceEvent, normalizeTraceEvent, toInlineStatusLine, type InlineStatusLine, type TraceEvent } from '@/lib/chat-trace';
import { toast } from '@/components/ui/toaster';
import type { Agent, ChatMessage, Citation, RAGTraceSummary } from '@/types';

interface ParsedSSEEvent {
  event: string;
  data: any;
}

const STREAM_IDLE_TIMEOUT_MS = 195000;
const STREAM_MAX_DURATION_MS = 180000;
const STREAM_IDLE_TIMEOUT_ERROR = '__stream_idle_timeout__';

async function readStreamChunkWithTimeout(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  timeoutMs: number
) {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  try {
    return await Promise.race([
      reader.read(),
      new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          reject(new Error(STREAM_IDLE_TIMEOUT_ERROR));
        }, timeoutMs);
      }),
    ]);
  } finally {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
    }
  }
}

function formatAgentErrorMessage(rawError: string, t: (key: string) => string): string {
  const normalized = rawError.toLowerCase();
  if (
    normalized.includes('stream total timeout') ||
    normalized.includes('aborterror') ||
    normalized.includes('the operation was aborted') ||
    normalized.includes('the user aborted a request') ||
    normalized.includes(STREAM_IDLE_TIMEOUT_ERROR)
  ) {
    return t('chat.streamTimeout');
  }
  if (normalized.includes('external provider not configured')) {
    return t('chat.providerNotConfigured');
  }
  return rawError.trim() || t('chat.chatError');
}

function parseSSEEvent(rawEvent: string): ParsedSSEEvent | null {
  const lines = rawEvent.split('\n');
  let event = 'message';
  const dataLines: string[] = [];

  for (const line of lines) {
    if (line.startsWith('event:')) {
      event = line.slice(6).trim();
      continue;
    }
    if (line.startsWith('data:')) {
      dataLines.push(line.slice(5).trim());
    }
  }

  if (dataLines.length === 0) {
    return null;
  }

  const payload = dataLines.join('\n');
  try {
    const data = JSON.parse(payload);

    // Backward compatibility: some services may send { type: 'token' } as payload.
    if (event === 'message' && typeof data?.type === 'string') {
      event = data.type;
    }

    return { event, data };
  } catch {
    return null;
  }
}

function resolveMentionedAgent(rawInput: string, agents: Agent[], selectedAgent: Agent | null) {
  const input = rawInput.trim();
  const mentionMatch = input.match(/^@([a-zA-Z0-9_-]+)\s+([\s\S]+)$/);

  if (!mentionMatch) {
    return {
      agent: selectedAgent,
      content: input,
      usedMention: false,
    };
  }

  const token = mentionMatch[1].toLowerCase();
  const content = mentionMatch[2].trim();

  const matched = agents.find((agent) => {
    const id = agent.id.toLowerCase();
    const normalizedName = agent.name.replace(/\s+/g, '').toLowerCase();
    return token === id || token === normalizedName;
  });

  return {
    agent: matched || null,
    content,
    usedMention: true,
  };
}

function mapAgentIDToBackendType(agentID?: string): 'rag' | 'general' | 'search' {
  const normalized = String(agentID || '').toLowerCase();
  if (normalized === 'knowledge_base' || normalized === 'rag') {
    return 'rag';
  }
  if (normalized === 'general') {
    return 'general';
  }
  if (normalized === 'search') {
    return 'search';
  }
  return 'rag';
}

interface ChatPanelProps {
  workspaceId?: string;
  sessionSource?: {
    type: 'share' | 'favorite';
    token: string;
  };
  selectedSourceIds?: string[];
  onExtractToNote?: (content: string) => void;
}

interface ChatSendPayload {
  content: string;
  mentionedAgent?: Agent | null;
}

function parseCitations(raw: unknown): Citation[] {
  if (Array.isArray(raw)) return raw as Citation[];
  if (typeof raw !== 'string' || !raw.trim()) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as Citation[]) : [];
  } catch {
    return [];
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function toRecord(value: unknown): Record<string, unknown> | null {
  if (!isObject(value)) return null;
  return value as Record<string, unknown>;
}

function parseDegradeTrace(raw: unknown): Record<string, unknown>[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter((item): item is Record<string, unknown> => isObject(item));
}

function isChatDebugEnabled(): boolean {
  if (typeof window === 'undefined') return false;
  try {
    const params = new URLSearchParams(window.location.search);
    if (params.get('chat_debug') === '1') return true;
    return window.localStorage.getItem('chat:debug') === '1';
  } catch {
    return false;
  }
}

function normalizeRagTrace(raw: unknown): RAGTraceSummary | null {
  if (!isObject(raw)) return null;
  const rawItems = Array.isArray(raw.items) ? raw.items : [];
  const items = rawItems
    .filter((item): item is Record<string, unknown> => isObject(item))
    .map((item) => ({
      priority: Number(item.priority || 0),
      item_type: typeof item.item_type === 'string' ? item.item_type : '',
      retrieval_mode: typeof item.retrieval_mode === 'string' ? item.retrieval_mode : '',
      purpose: typeof item.purpose === 'string' ? item.purpose : '',
      query: typeof item.query === 'string' ? item.query : undefined,
      recall_budget: Number(item.recall_budget || 0),
      bm25_k: Number(item.bm25_k || 0),
      dense_k: Number(item.dense_k || 0),
      rerank_budget: Number(item.rerank_budget || 0),
      source_count: Number(item.source_count || 0),
      source_ids: Array.isArray(item.source_ids)
        ? item.source_ids.filter((id): id is string => typeof id === 'string')
        : undefined,
    }))
    .filter((item) => item.item_type || item.purpose || item.query);

  return {
    item_count: Number(raw.item_count || items.length),
    total_candidate_budget: Number(raw.total_candidate_budget || 0),
    max_rerank_docs: Number(raw.max_rerank_docs || 0),
    max_final_chunks: Number(raw.max_final_chunks || 0),
    top_k_returned: Number(raw.top_k_returned || 0),
    summary_mode: typeof raw.summary_mode === 'string' ? raw.summary_mode : undefined,
    items,
  };
}

function normalizeRagTraceFromModeDebug(raw: unknown): RAGTraceSummary | null {
  const modeDebug = toRecord(raw);
  if (!modeDebug) return null;
  const rag = toRecord(modeDebug.rag);
  if (!rag) return null;

  const itemTrace = Array.isArray(rag.item_trace) ? rag.item_trace : [];
  const retrievalTrace = toRecord(rag.retrieval_trace);
  const summaryTrace = toRecord(rag.summary_injection_trace);

  return normalizeRagTrace({
    item_count: itemTrace.length,
    total_candidate_budget: Number(retrievalTrace?.total_candidate_budget || 0),
    max_rerank_docs: Number(retrievalTrace?.max_rerank_docs || 0),
    max_final_chunks: Number(retrievalTrace?.max_final_chunks || 0),
    top_k_returned: Number(retrievalTrace?.top_k_returned || 0),
    summary_mode: typeof summaryTrace?.mode === 'string' ? summaryTrace.mode : undefined,
    items: itemTrace,
  });
}

function normalizeChatMessage(raw: any): ChatMessage {
  return {
    id: Number(raw?.id || 0),
    session_id: String(raw?.session_id || ''),
    role: raw?.role === 'assistant' ? 'assistant' : 'user',
    content: String(raw?.content || ''),
    agent_id: typeof raw?.agent_id === 'string' ? raw.agent_id : undefined,
    agent_name: typeof raw?.agent_name === 'string' ? raw.agent_name : undefined,
    agent_icon: typeof raw?.agent_icon === 'string' ? raw.agent_icon : undefined,
    citations: parseCitations(raw?.citations),
    created_at: raw?.created_at ? new Date(raw.created_at).toISOString() : new Date().toISOString(),
  };
}

export function ChatPanel({
  workspaceId,
  sessionSource,
  selectedSourceIds = [],
  onExtractToNote,
}: ChatPanelProps) {
  const { t } = useTranslation();
  const { currentWorkspace } = useAppStore();
  const [agents, setAgents] = useState<Agent[]>(() => mergeAgentsWithBuiltins());
  const [selectedAgent, setSelectedAgent] = useState<Agent | null>(() => {
    const seededAgents = mergeAgentsWithBuiltins();
    return seededAgents.find((agent) => agent.id === 'knowledge_base') || seededAgents[0] || null;
  });
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [sessionId, setSessionId] = useState<string>('');
  const [assistantStatusById, setAssistantStatusById] = useState<Record<number, InlineStatusLine>>({});
  const [traceEvents, setTraceEvents] = useState<TraceEvent[]>([]);
  const [ragTrace, setRagTrace] = useState<RAGTraceSummary | null>(null);
  const [degradeTrace, setDegradeTrace] = useState<Record<string, unknown>[]>([]);
  const [plannerOutput, setPlannerOutput] = useState<Record<string, unknown> | null>(null);
  const [modeDebug, setModeDebug] = useState<Record<string, unknown> | null>(null);
  const [debugEnabled, setDebugEnabled] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const turnToAssistantRef = useRef<Record<string, number>>({});
  const activeAssistantRef = useRef<number | null>(null);
  const activeWorkspaceId = workspaceId || currentWorkspace?.id || '';
  const activeSessionSourceType = sessionSource?.type;
  const activeSessionSourceToken = sessionSource?.token || '';
  const sessionScopeKey = activeSessionSourceType && activeSessionSourceToken
    ? `${activeSessionSourceType}:${activeSessionSourceToken}`
    : 'owner';
  const lastSessionStorageKey = activeWorkspaceId
    ? `chat:last-session:${activeWorkspaceId}:${sessionScopeKey}`
    : '';

  const resetDebugArtifacts = useCallback(() => {
    setTraceEvents([]);
    setRagTrace(null);
    setDegradeTrace([]);
    setPlannerOutput(null);
    setModeDebug(null);
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    setDebugEnabled(isChatDebugEnabled());
  }, []);

  useEffect(() => {
    if (!sessionId) return;
    chatApi.cacheMessages(sessionId, messages);
  }, [messages, sessionId]);

  useEffect(() => {
    const fetchAgents = async () => {
      try {
        const response = await agentsApi.list();
        if (response.success && response.data?.agents) {
          const nextAgents = mergeAgentsWithBuiltins(response.data.agents as Agent[]);
          setAgents(nextAgents);
          setSelectedAgent((prev) => {
            if (prev) {
              const matched = nextAgents.find((agent) => agent.id === prev.id);
              if (matched) {
                return matched;
              }
            }
            const kbAgent = nextAgents.find((agent) => agent.id === 'knowledge_base');
            return kbAgent || nextAgents[0] || null;
          });
        }
      } catch (error) {
        console.error('Failed to fetch agents:', error);
      }
    };
    void fetchAgents();
  }, []);

  const persistSessionId = useCallback(
    (id: string) => {
      if (typeof window === 'undefined' || !lastSessionStorageKey) return;
      if (id) {
        localStorage.setItem(lastSessionStorageKey, id);
      } else {
        localStorage.removeItem(lastSessionStorageKey);
      }
    },
    [lastSessionStorageKey]
  );

  const loadSessionMessages = useCallback(
    async (nextSessionId: string) => {
      if (!nextSessionId) {
        setMessages([]);
        setAssistantStatusById({});
        resetDebugArtifacts();
        turnToAssistantRef.current = {};
        activeAssistantRef.current = null;
        return;
      }
      try {
        setLoadingHistory(true);
        const response = await chatApi.getMessages(nextSessionId);
        if (!response?.success || !Array.isArray(response?.data)) {
          toast.error(response?.error || t('chat.historyLoadFailed'));
          setMessages([]);
          setAssistantStatusById({});
          resetDebugArtifacts();
          turnToAssistantRef.current = {};
          activeAssistantRef.current = null;
          return;
        }
        setMessages(response.data.map(normalizeChatMessage));
      } catch (error) {
        console.error('Failed to load chat messages:', error);
        toast.error(t('chat.historyLoadFailed'));
        setMessages([]);
        setAssistantStatusById({});
        resetDebugArtifacts();
        turnToAssistantRef.current = {};
        activeAssistantRef.current = null;
      } finally {
        setLoadingHistory(false);
      }
    },
    [resetDebugArtifacts, t]
  );

  const loadPreferredSession = useCallback(
    async (preferredId: string) => {
      if (!preferredId) {
        return '';
      }

      const response = await chatApi.getSession(preferredId);
      if (!response?.success || !response.data?.id) {
        return '';
      }

      const nextSessionId = String(response.data.id || '');
      if (!nextSessionId) {
        return '';
      }

      setSessionId(nextSessionId);
      persistSessionId(nextSessionId);
      await loadSessionMessages(nextSessionId);
      return nextSessionId;
    },
    [loadSessionMessages, persistSessionId]
  );

  useEffect(() => {
    let cancelled = false;
    const restoreSession = async () => {
      if (!activeWorkspaceId) {
        setSessionId('');
        setMessages([]);
        setAssistantStatusById({});
        resetDebugArtifacts();
        turnToAssistantRef.current = {};
        activeAssistantRef.current = null;
        return;
      }

      try {
        setLoadingHistory(true);
        const response = await chatApi.listSessions(
          activeWorkspaceId,
          activeSessionSourceType,
          activeSessionSourceToken || undefined
        );
        if (!response?.success) {
          if (!cancelled) {
            toast.error(response.error || t('chat.sessionLoadFailed'));
            setSessionId('');
            setMessages([]);
            setAssistantStatusById({});
            resetDebugArtifacts();
            turnToAssistantRef.current = {};
            activeAssistantRef.current = null;
            persistSessionId('');
          }
          return;
        }
        let preferredId = '';
        if (typeof window !== 'undefined' && lastSessionStorageKey) {
          preferredId = localStorage.getItem(lastSessionStorageKey) || '';
        }

        const sessions = Array.isArray(response?.data) ? response.data : [];
        if (sessions.length === 0) {
          if (preferredId && !cancelled) {
            const restoredSessionId = await loadPreferredSession(preferredId);
            if (restoredSessionId) {
              return;
            }
          }
          if (!cancelled) {
            setSessionId('');
            setMessages([]);
            setAssistantStatusById({});
            resetDebugArtifacts();
            turnToAssistantRef.current = {};
            activeAssistantRef.current = null;
            persistSessionId('');
          }
          return;
        }

        let target = sessions.find((s: any) => s?.id === preferredId) || null;
        if (!target && preferredId) {
          const preferredResponse = await chatApi.getSession(preferredId);
          if (preferredResponse?.success && preferredResponse.data?.id) {
            target = preferredResponse.data as any;
          }
        }
        if (!target) {
          target = [...sessions].sort((a: any, b: any) => {
            const ta = Date.parse(String(a?.updated_at || a?.created_at || 0));
            const tb = Date.parse(String(b?.updated_at || b?.created_at || 0));
            return tb - ta;
          })[0];
        }

        const nextSessionId = String(target?.id || '');
        if (!nextSessionId) {
          if (!cancelled) {
            setSessionId('');
            setMessages([]);
            setAssistantStatusById({});
            resetDebugArtifacts();
            turnToAssistantRef.current = {};
            activeAssistantRef.current = null;
            persistSessionId('');
          }
          return;
        }

        if (!cancelled) {
          setSessionId(nextSessionId);
          persistSessionId(nextSessionId);
        }
        await loadSessionMessages(nextSessionId);
      } catch (error) {
        console.error('Failed to restore chat session:', error);
        if (!cancelled) {
          toast.error(t('chat.sessionLoadFailed'));
          setSessionId('');
          setMessages([]);
          setAssistantStatusById({});
          resetDebugArtifacts();
          turnToAssistantRef.current = {};
          activeAssistantRef.current = null;
        }
      } finally {
        if (!cancelled) {
          setLoadingHistory(false);
        }
      }
    };

    void restoreSession();
    return () => {
      cancelled = true;
    };
  }, [
    activeWorkspaceId,
    activeSessionSourceToken,
    activeSessionSourceType,
    lastSessionStorageKey,
    loadPreferredSession,
    loadSessionMessages,
    persistSessionId,
    resetDebugArtifacts,
    t,
  ]);

  const handleSend = async ({ content: rawContent, mentionedAgent }: ChatSendPayload) => {
    if (!activeWorkspaceId) {
      toast.error(t('chat.selectWorkspaceFirst'));
      return;
    }

    const input = rawContent.trim();
    if (!input) {
      toast.error(t('chat.emptyMessage'));
      return;
    }

    // Prefer explicit chip selection; fall back to legacy "@agent content" parsing.
    const resolved = mentionedAgent
      ? { agent: mentionedAgent, content: input, usedMention: false }
      : resolveMentionedAgent(input, agents, selectedAgent);
    if (resolved.usedMention && !resolved.agent) {
      toast.error(t('chat.unrecognizedAgent'));
      return;
    }
    if (!resolved.agent) {
      toast.error(t('chat.noAgentSelected'));
      return;
    }

    const agentToUse = resolved.agent;
    const contentToSend = resolved.content.trim();
    if (!contentToSend) {
      toast.error(t('chat.emptyMessage'));
      return;
    }

    const now = Date.now();
    const userTempId = now;
    const assistantTempId = now + 1;

    const userMessage: ChatMessage = {
      id: userTempId,
      session_id: sessionId,
      role: 'user',
      content: contentToSend,
      agent_id: agentToUse.id,
      agent_name: agentToUse.name,
      agent_icon: agentToUse.icon,
      created_at: new Date().toISOString(),
    };

    const assistantMessage: ChatMessage = {
      id: assistantTempId,
      session_id: sessionId,
      role: 'assistant',
      content: '',
      agent_id: agentToUse.id,
      agent_name: agentToUse.name,
      agent_icon: agentToUse.icon,
      citations: [],
      created_at: new Date().toISOString(),
    };

    const historyForRequest = [...messages, userMessage].map((m) => ({
      role: m.role,
      content: m.content,
    }));

    setMessages((prev) => [...prev, userMessage, assistantMessage]);
    activeAssistantRef.current = assistantTempId;
    setAssistantStatusById((prev) => ({
      ...prev,
      [assistantTempId]: {
        text: t('chat.statusTurnStart'),
        tone: 'progress',
        live: true,
        stage: 'turn.start',
        timestamp: Date.now(),
      },
    }));
    setLoading(true);
    resetDebugArtifacts();

    try {
      const controller = new AbortController();
      const hardTimeoutId = setTimeout(() => {
        controller.abort(new Error('stream total timeout'));
      }, STREAM_MAX_DURATION_MS);

      try {
        const token = getAuthToken();
        const response = await fetch('/api/v1/chat?stream=true', {
          method: 'POST',
          headers: {
            Accept: 'text/event-stream',
            'Content-Type': 'application/json',
            ...(token ? { Authorization: `Bearer ${token}` } : {}),
          },
          signal: controller.signal,
          body: JSON.stringify({
            query: contentToSend,
            notebook_id: activeWorkspaceId,
            session_id: sessionId || undefined,
            agent_type: mapAgentIDToBackendType(agentToUse.id),
            source_type: activeSessionSourceType || undefined,
            source_token: activeSessionSourceToken || undefined,
            messages: historyForRequest,
            doc_scope: selectedSourceIds,
            stream: true,
          }),
        });

        if (!response.ok) {
          let errorText = '';
          try {
            const payload = await response.json();
            if (typeof payload?.error === 'string') {
              errorText = payload.error;
            }
          } catch {
            // Ignore JSON parse failures and fall back to status text.
          }
          throw new Error(errorText || `HTTP ${response.status}`);
        }

        const reader = response.body?.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        let streamError = '';
        let shouldStop = false;
        let ragTraceReceived = false;

        while (reader) {
          let readResult: ReadableStreamReadResult<Uint8Array>;
          try {
            readResult = await readStreamChunkWithTimeout(reader, STREAM_IDLE_TIMEOUT_MS);
          } catch (err) {
            if (err instanceof Error && err.message === STREAM_IDLE_TIMEOUT_ERROR) {
              streamError = t('chat.streamTimeout');
              const targetId = activeAssistantRef.current || assistantTempId;
              setAssistantStatusById((prev) => ({
                ...prev,
                [targetId]: {
                  text: t('chat.statusError', { error: streamError }),
                  tone: 'error',
                  live: false,
                  stage: 'turn.error',
                  timestamp: Date.now(),
                },
              }));
              shouldStop = true;
              break;
            }
            throw err;
          }

          const { done, value } = readResult;
          if (done) {
            buffer += decoder.decode();
          } else if (value) {
            buffer += decoder.decode(value, { stream: true });
          }

          let boundaryIndex = buffer.indexOf('\n\n');
          while (boundaryIndex >= 0) {
            const rawEvent = buffer.slice(0, boundaryIndex).trim();
            buffer = buffer.slice(boundaryIndex + 2);

            if (rawEvent) {
              const parsed = parseSSEEvent(rawEvent);
              if (parsed) {
                const { event, data } = parsed;

                if (event === 'trace') {
                  const normalized = normalizeTraceEvent(data);
                  if (normalized) {
                    setTraceEvents((prev) => appendTraceEvent(prev, normalized));
                    const statusLine = toInlineStatusLine(normalized, (key, options) =>
                      String(t(key as any, options as any))
                    );
                    if (statusLine) {
                      const mappedId = normalized.turn_id ? turnToAssistantRef.current[normalized.turn_id] : undefined;
                      const targetId = mappedId || activeAssistantRef.current || assistantTempId;
                      setAssistantStatusById((prev) => ({
                        ...prev,
                        [targetId]: statusLine,
                      }));
                    }
                  }
                  boundaryIndex = buffer.indexOf('\n\n');
                  continue;
                }

                if (event === 'rag_trace') {
                  const normalized = normalizeRagTrace(data?.rag ?? data);
                  if (normalized) {
                    setRagTrace(normalized);
                    ragTraceReceived = true;
                  }
                }

                if (event === 'start') {
                  const nextSessionId = typeof data?.session_id === 'string' ? data.session_id : '';
                  const turnId = typeof data?.turn_id === 'string' ? data.turn_id : '';
                  if (turnId) {
                    turnToAssistantRef.current[turnId] = activeAssistantRef.current || assistantTempId;
                  }

                  if (nextSessionId) {
                    setSessionId(nextSessionId);
                    persistSessionId(nextSessionId);
                    setMessages((prev) =>
                      prev.map((m) =>
                        m.id === userTempId || m.id === assistantTempId
                          ? { ...m, session_id: nextSessionId }
                          : m
                      )
                    );
                  }
                }

                if (event === 'token' && typeof data?.content === 'string') {
                  setMessages((prev) =>
                    prev.map((m) =>
                      m.id === assistantTempId
                        ? { ...m, content: m.content + data.content }
                        : m
                    )
                  );
                }

                if (event === 'search_start') {
                  const targetId = activeAssistantRef.current || assistantTempId;
                  const message = typeof data?.message === 'string' ? data.message : t('chat.searchStarted') || 'Searching...';
                  setAssistantStatusById((prev) => ({
                    ...prev,
                    [targetId]: {
                      text: message,
                      tone: 'progress',
                      live: true,
                      stage: 'search.start',
                      timestamp: Date.now(),
                    },
                  }));
                }

                if (event === 'search_results') {
                  const targetId = activeAssistantRef.current || assistantTempId;
                  const resultCount = Number(data?.data?.result_count || 0);
                  const message = t('chat.searchResults', { count: resultCount }) || `${resultCount} search results found`;
                  setAssistantStatusById((prev) => ({
                    ...prev,
                    [targetId]: {
                      text: message,
                      tone: 'progress',
                      live: true,
                      stage: 'search.done',
                      timestamp: Date.now(),
                    },
                  }));
                }

                if (event === 'citation') {
                  setMessages((prev) =>
                    prev.map((m) => {
                      if (m.id !== assistantTempId) return m;
                      const nextCitation = data as Citation;
                      const prevCitations = Array.isArray(m.citations) ? m.citations : [];
                      return {
                        ...m,
                        citations: [...prevCitations, nextCitation],
                      };
                    })
                  );
                }

                if (event === 'citations' && Array.isArray(data?.citations)) {
                  setMessages((prev) =>
                    prev.map((m) => {
                      if (m.id !== assistantTempId) return m;
                      const prevCitations = Array.isArray(m.citations) ? m.citations : [];
                      return {
                        ...m,
                        citations: [...prevCitations, ...(data.citations as Citation[])],
                      };
                    })
                  );
                }

                if (event === 'done') {
                  const nextSessionId = typeof data?.session_id === 'string' ? data.session_id : '';
                  const persistedMessageId = Number(data?.message_id || 0);
                  const doneAnswer = typeof data?.answer === 'string' ? data.answer : '';
                  const doneCitations = Array.isArray(data?.citations) ? (data.citations as Citation[]) : null;
                  const nextDegradeTrace = parseDegradeTrace(data?.degrade_trace);
                  const nextPlannerOutput = toRecord(data?.planner_output);
                  const nextModeDebug = toRecord(data?.mode_debug);

                  setDegradeTrace(nextDegradeTrace);
                  setPlannerOutput(nextPlannerOutput);
                  setModeDebug(nextModeDebug);
                  if (!ragTraceReceived) {
                    const fallbackRagTrace = normalizeRagTraceFromModeDebug(nextModeDebug);
                    if (fallbackRagTrace) {
                      setRagTrace(fallbackRagTrace);
                    }
                  }

                  if (nextSessionId) {
                    setSessionId(nextSessionId);
                    persistSessionId(nextSessionId);
                  }

                  if (Number.isFinite(persistedMessageId) && persistedMessageId > 0) {
                    setMessages((prev) =>
                      prev.map((m) => {
                        if (m.id !== assistantTempId) return m;
                        return {
                          ...m,
                          id: persistedMessageId,
                          session_id: nextSessionId || m.session_id,
                          ...(doneAnswer ? { content: doneAnswer } : {}),
                          ...(doneCitations ? { citations: doneCitations } : {}),
                        };
                      })
                    );

                    setAssistantStatusById((prev) => {
                      if (!prev[assistantTempId]) {
                        return prev;
                      }
                      const next = {
                        ...prev,
                        [persistedMessageId]: { ...prev[assistantTempId], live: false },
                      };
                      delete next[assistantTempId];
                      return next;
                    });

                    Object.keys(turnToAssistantRef.current).forEach((turnId) => {
                      if (turnToAssistantRef.current[turnId] === assistantTempId) {
                        turnToAssistantRef.current[turnId] = persistedMessageId;
                      }
                    });
                    activeAssistantRef.current = persistedMessageId;
                  } else if (activeAssistantRef.current) {
                    const messageId = activeAssistantRef.current;
                    if (doneAnswer || doneCitations) {
                      setMessages((prev) =>
                        prev.map((m) =>
                          m.id === messageId
                            ? {
                                ...m,
                                ...(doneAnswer ? { content: doneAnswer } : {}),
                                ...(doneCitations ? { citations: doneCitations } : {}),
                              }
                            : m
                        )
                      );
                    }
                    setAssistantStatusById((prev) => {
                      const current = prev[messageId];
                      if (!current) return prev;
                      return {
                        ...prev,
                        [messageId]: { ...current, live: false },
                      };
                    });
                  }
                }

                if (event === 'error') {
                  const traceId = typeof data?.trace_id === 'string' ? data.trace_id.trim() : '';
                  if (typeof data?.error === 'string') {
                    streamError = data.error;
                  } else if (typeof data?.content === 'string') {
                    streamError = data.content;
                  } else {
                    streamError = 'stream error';
                  }
                  if (traceId) {
                    streamError = `${streamError} (trace_id=${traceId})`;
                  }
                  const targetId = activeAssistantRef.current || assistantTempId;
                  setAssistantStatusById((prev) => ({
                    ...prev,
                    [targetId]: {
                      text: t('chat.statusError', { error: streamError }),
                      tone: 'error',
                      live: false,
                      stage: 'turn.error',
                      timestamp: Date.now(),
                    },
                  }));
                  shouldStop = true;
                  break;
                }
              }
            }

            boundaryIndex = buffer.indexOf('\n\n');
          }

          if (shouldStop || done) break;
        }

        if (streamError) {
          const friendlyError = formatAgentErrorMessage(streamError, t);
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantTempId
                ? { ...m, content: friendlyError }
                : m
            )
          );
          const targetId = activeAssistantRef.current || assistantTempId;
          setAssistantStatusById((prev) => ({
            ...prev,
            [targetId]: {
              text: t('chat.statusError', { error: friendlyError }),
              tone: 'error',
              live: false,
              stage: 'turn.error',
              timestamp: Date.now(),
            },
          }));
          toast.error(friendlyError);
        }
      } finally {
        clearTimeout(hardTimeoutId);
      }
    } catch (error) {
      const message = error instanceof Error ? `${error.name}: ${error.message}` : '';
      const friendlyError = formatAgentErrorMessage(message, t);
      console.error('Chat error:', error);
      setMessages((prev) =>
        prev.map((m) =>
          m.id === assistantTempId
            ? { ...m, content: friendlyError || t('chat.chatError') }
            : m
        )
      );
      const targetId = activeAssistantRef.current || assistantTempId;
      setAssistantStatusById((prev) => ({
        ...prev,
        [targetId]: {
          text: t('chat.statusError', { error: friendlyError || t('chat.chatError') }),
          tone: 'error',
          live: false,
          stage: 'turn.error',
          timestamp: Date.now(),
        },
      }));
      toast.error(friendlyError || t('chat.chatError'));
    } finally {
      setLoading(false);
      activeAssistantRef.current = null;
    }
  };

  const handleExtract = useCallback(
    async (content: string) => {
      if (!activeWorkspaceId) {
        toast.error(t('chat.selectWorkspaceFirst'));
        return;
      }

      if (onExtractToNote) {
        onExtractToNote(content);
        return;
      }

      try {
        const response = await notesApi.create(
          activeWorkspaceId,
          content,
          `${t('chat.extractTitle')} ${new Date().toLocaleString()}`
        );

        if (response.success && response.data) {
          toast.success(t('chat.extractedAsDraft'));
        } else {
          throw new Error(response.error || 'Failed to create note');
        }
      } catch (error) {
        console.error('Failed to extract to note:', error);
        toast.error(t('chat.extractFailed'));
      }
    },
    [activeWorkspaceId, onExtractToNote, t]
  );

  const handleClearChat = useCallback(() => {
    if (messages.length === 0) return;
    if (window.confirm(t('chat.confirmClear'))) {
      const deletingSessionId = sessionId;
      setMessages([]);
      setAssistantStatusById({});
      resetDebugArtifacts();
      turnToAssistantRef.current = {};
      activeAssistantRef.current = null;
      setSessionId('');
      persistSessionId('');
      if (deletingSessionId) {
        chatApi.clearCachedMessages(deletingSessionId);
        void (async () => {
          const response = await chatApi.deleteSession(deletingSessionId);
          if (!response.success) {
            console.error('Failed to delete chat session:', response.error);
            toast.error(response.error || t('chat.chatError'));
          }
        })();
      }
    }
  }, [messages.length, persistSessionId, resetDebugArtifacts, sessionId, t]);

  useEffect(() => {
    const onClearChat = () => {
      handleClearChat();
    };
    window.addEventListener('dashboard:clear-chat', onClearChat);
    return () => {
      window.removeEventListener('dashboard:clear-chat', onClearChat);
    };
  }, [handleClearChat]);

  return (
    <div className="flex flex-col h-full">
      <div className="shrink-0 px-3 md:px-4 py-2 border-b border-border flex items-center justify-start">
        <h2 className="text-lg font-semibold">{t('dashboard.aiChat')}</h2>
      </div>
      
      <div className="flex-1 overflow-auto p-3 md:p-4">
        {messages.length === 0 ? (
          <div className="h-full" />
        ) : (
          <div>
            {messages.map((msg) => (
              <ChatBubble
                key={`${msg.id}-${msg.created_at}`}
                message={msg}
                agentName={msg.role === 'assistant' ? (msg.agent_name || selectedAgent?.name) : undefined}
                agentIcon={msg.role === 'assistant' ? (msg.agent_icon || selectedAgent?.icon) : undefined}
                statusLine={msg.role === 'assistant' ? assistantStatusById[msg.id] : undefined}
                onExtract={msg.role === 'assistant' ? handleExtract : undefined}
              />
            ))}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {degradeTrace.length > 0 && (
        <div className="mx-3 md:mx-4 mb-2 rounded-md border border-amber-300/70 bg-amber-50/60 px-3 py-2 text-xs text-amber-900">
          <div className="font-medium">{t('chat.degradeTitle')}</div>
          <div className="mt-1">{t('chat.degradeHint', { count: degradeTrace.length })}</div>
          {debugEnabled && (
            <pre className="mt-2 whitespace-pre-wrap break-words rounded bg-amber-100/70 px-2 py-1 text-[11px] text-amber-950">
              {JSON.stringify(degradeTrace, null, 2)}
            </pre>
          )}
        </div>
      )}

      {debugEnabled && (plannerOutput || modeDebug) && (
        <div className="mx-3 md:mx-4 mb-2 rounded-md border border-border bg-card/70 px-3 py-2 text-xs">
          <div className="font-medium">{t('chat.debugPayloadTitle')}</div>
          {plannerOutput && (
            <pre className="mt-2 whitespace-pre-wrap break-words rounded bg-background/80 px-2 py-1 text-[11px] text-muted-foreground">
              {JSON.stringify({ planner_output: plannerOutput }, null, 2)}
            </pre>
          )}
          {modeDebug && (
            <pre className="mt-2 whitespace-pre-wrap break-words rounded bg-background/80 px-2 py-1 text-[11px] text-muted-foreground">
              {JSON.stringify({ mode_debug: modeDebug }, null, 2)}
            </pre>
          )}
        </div>
      )}

      {debugEnabled && (traceEvents.length > 0 || ragTrace) && (
        <ChatTracePanel events={traceEvents} ragTrace={ragTrace} loading={loading} />
      )}

      <ChatInput
        selectedAgent={selectedAgent}
        agents={agents}
        onAgentSelect={(agent) => setSelectedAgent(agent)}
        onSend={handleSend}
        disabled={loading || loadingHistory || !activeWorkspaceId}
      />
    </div>
  );
}
