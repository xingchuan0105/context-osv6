import { describe, expect, it } from 'vitest';
import { appendTraceEvent, normalizeTraceEvent } from './chat-trace';

describe('chat-trace', () => {
  it('normalizes valid trace payload', () => {
    const trace = normalizeTraceEvent({
      seq: 2,
      trace_id: 'trace-1',
      turn_id: 'turn-1',
      stage: 'router.intent',
      status: 'progress',
      message: 'intent selected',
      data: { intents: ['D1'] },
      timestamp: 123,
    });

    expect(trace).not.toBeNull();
    expect(trace?.seq).toBe(2);
    expect(trace?.stage).toBe('router.intent');
    expect(trace?.trace_id).toBe('trace-1');
  });

  it('returns null for invalid trace payload', () => {
    const trace = normalizeTraceEvent({ seq: 'x', stage: 123 });
    expect(trace).toBeNull();
  });

  it('appends and deduplicates by trace-turn-seq key', () => {
    const one = normalizeTraceEvent({
      seq: 1,
      trace_id: 'trace-1',
      turn_id: 'turn-1',
      stage: 'turn.start',
      status: 'start',
      message: 'start',
      timestamp: 1,
    });
    const duplicate = normalizeTraceEvent({
      seq: 1,
      trace_id: 'trace-1',
      turn_id: 'turn-1',
      stage: 'turn.start',
      status: 'start',
      message: 'start again',
      timestamp: 2,
    });

    let acc = [] as any[];
    acc = appendTraceEvent(acc, one!);
    acc = appendTraceEvent(acc, duplicate!);

    expect(acc).toHaveLength(1);
    expect(acc[0].message).toBe('start');
  });
});
