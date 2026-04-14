import { describe, expect, it } from 'vitest';
import type { TraceEvent } from './chat-trace';
import { toInlineStatusLine } from './chat-trace';

function t(key: string, params?: Record<string, unknown>) {
  switch (key) {
    case 'chat.statusRouterStart':
      return 'routing';
    case 'chat.statusExternalRequest':
      return `calling model #${String(params?.step ?? '')}`;
    case 'chat.statusDone':
      return `done ${String(params?.latency ?? '')} ${String(params?.tokens ?? '')}`.trim();
    case 'chat.statusError':
      return `error ${String(params?.error ?? '')}`.trim();
    default:
      return key;
  }
}

function event(overrides?: Partial<TraceEvent>): TraceEvent {
  return {
    trace_id: 'trace-1',
    turn_id: 'turn-1',
    seq: 1,
    stage: 'router.start',
    status: 'progress',
    message: 'router starts',
    timestamp: 1,
    ...overrides,
  };
}

describe('toInlineStatusLine', () => {
  it('maps router.start to a progress line', () => {
    const line = toInlineStatusLine(event({ stage: 'router.start' }), t);
    expect(line).not.toBeNull();
    expect(line?.tone).toBe('progress');
    expect(line?.text).toBe('routing');
    expect(line?.live).toBe(true);
  });

  it('maps external.request with step', () => {
    const line = toInlineStatusLine(
      event({ stage: 'external.request', data: { step: 2 } as Record<string, unknown> }),
      t
    );
    expect(line?.text).toContain('#2');
  });

  it('maps turn.done with latency and tokens', () => {
    const line = toInlineStatusLine(
      event({
        stage: 'turn.done',
        status: 'done',
        data: { latency_ms: 2100, total_tokens: 123 } as Record<string, unknown>,
      }),
      t
    );
    expect(line?.tone).toBe('done');
    expect(line?.live).toBe(false);
    expect(line?.text).toContain('2.1s');
    expect(line?.text).toContain('123');
  });

  it('maps errors to terminal error line', () => {
    const line = toInlineStatusLine(
      event({
        stage: 'external.error',
        status: 'error',
        data: { error: 'timeout' } as Record<string, unknown>,
      }),
      t
    );
    expect(line?.tone).toBe('error');
    expect(line?.live).toBe(false);
    expect(line?.text).toContain('timeout');
  });
});
