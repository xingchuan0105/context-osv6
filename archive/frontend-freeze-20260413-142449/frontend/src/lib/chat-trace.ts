export interface TraceEvent {
  trace_id: string;
  turn_id: string;
  seq: number;
  stage: string;
  status: 'start' | 'progress' | 'done' | 'error';
  message: string;
  agent_id?: string;
  agent_name?: string;
  data?: Record<string, unknown>;
  timestamp: number;
}

export interface InlineStatusLine {
  text: string;
  tone: 'progress' | 'done' | 'error';
  live: boolean;
  stage: string;
  timestamp: number;
}

export type TranslateFn = (key: string, options?: Record<string, unknown>) => string;

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function normalizeTraceEvent(raw: unknown): TraceEvent | null {
  if (!isObject(raw)) return null;

  const seq = Number(raw.seq);
  const stage = typeof raw.stage === 'string' ? raw.stage : '';
  if (!Number.isFinite(seq) || seq <= 0 || !stage) return null;

  const statusRaw = typeof raw.status === 'string' ? raw.status : 'progress';
  const status: TraceEvent['status'] =
    statusRaw === 'start' || statusRaw === 'done' || statusRaw === 'error'
      ? statusRaw
      : 'progress';

  return {
    trace_id: typeof raw.trace_id === 'string' ? raw.trace_id : '',
    turn_id: typeof raw.turn_id === 'string' ? raw.turn_id : '',
    seq,
    stage,
    status,
    message: typeof raw.message === 'string' ? raw.message : '',
    agent_id: typeof raw.agent_id === 'string' ? raw.agent_id : undefined,
    agent_name: typeof raw.agent_name === 'string' ? raw.agent_name : undefined,
    data: isObject(raw.data) ? (raw.data as Record<string, unknown>) : undefined,
    timestamp: Number(raw.timestamp || Date.now()),
  };
}

function traceEventKey(event: TraceEvent): string {
  return `${event.trace_id}:${event.turn_id}:${event.seq}`;
}

function toNumber(value: unknown): number {
  const n = Number(value);
  return Number.isFinite(n) ? n : 0;
}

function formatLatency(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return '';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function stageDefaultKey(stage: string): string {
  const map: Record<string, string> = {
    'turn.start': 'chat.statusTurnStart',
    'agent.select': 'chat.statusAgentSelect',
    'kb.pipeline.start': 'chat.statusKbStart',
    'router.start': 'chat.statusRouterStart',
    'router.done': 'chat.statusRouterDone',
    'agent.run.start': 'chat.statusAgentRunStart',
    'agent.run.done': 'chat.statusAgentRunDone',
    'external.start': 'chat.statusExternalStart',
    'llm.think_summary': 'chat.statusThinking',
    'llm.stream.start': 'chat.statusGenerating',
    'llm.stream.done': 'chat.statusGenerated',
    'citation.emit': 'chat.statusCitation',
  };
  return map[stage] || 'chat.statusInProgress';
}

export function toInlineStatusLine(event: TraceEvent, t: TranslateFn): InlineStatusLine | null {
  if (!event || !event.stage) return null;

  if (event.stage === 'turn.done') {
    const latency = formatLatency(toNumber(event.data?.latency_ms));
    const tokens = toNumber(event.data?.total_tokens);
    return {
      text: t('chat.statusDone', { latency, tokens }),
      tone: 'done',
      live: false,
      stage: event.stage,
      timestamp: event.timestamp,
    };
  }

  if (event.status === 'error' || event.stage.endsWith('.error')) {
    const error =
      typeof event.data?.error === 'string' && event.data.error.trim()
        ? event.data.error
        : event.message || '';
    return {
      text: t('chat.statusError', { error }),
      tone: 'error',
      live: false,
      stage: event.stage,
      timestamp: event.timestamp,
    };
  }

  if (event.stage === 'external.request') {
    return {
      text: t('chat.statusExternalRequest', { step: toNumber(event.data?.step) || 1 }),
      tone: 'progress',
      live: true,
      stage: event.stage,
      timestamp: event.timestamp,
    };
  }

  if (event.stage === 'tool.call') {
    return {
      text: t('chat.statusToolCall', { count: toNumber(event.data?.count) || 1 }),
      tone: 'progress',
      live: true,
      stage: event.stage,
      timestamp: event.timestamp,
    };
  }

  if (event.stage === 'tool.result') {
    return {
      text: t('chat.statusToolResult', { tool: String(event.data?.tool_name || '') }),
      tone: 'progress',
      live: true,
      stage: event.stage,
      timestamp: event.timestamp,
    };
  }

  return {
    text: t(stageDefaultKey(event.stage)),
    tone: event.status === 'done' ? 'done' : 'progress',
    live: event.status !== 'done',
    stage: event.stage,
    timestamp: event.timestamp,
  };
}

export function appendTraceEvent(
  prev: TraceEvent[],
  next: TraceEvent,
  maxSize = 500
): TraceEvent[] {
  const nextKey = traceEventKey(next);
  if (prev.some((item) => traceEventKey(item) === nextKey)) {
    return prev;
  }

  const merged = [...prev, next].sort((a, b) => {
    if (a.trace_id !== b.trace_id) return a.trace_id.localeCompare(b.trace_id);
    if (a.turn_id !== b.turn_id) return a.turn_id.localeCompare(b.turn_id);
    return a.seq - b.seq;
  });

  if (merged.length <= maxSize) return merged;
  return merged.slice(merged.length - maxSize);
}
