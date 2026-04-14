// @vitest-environment jsdom

import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ChatTracePanel } from './chat-trace-panel';
import type { TraceEvent } from '@/lib/chat-trace';
import type { RAGTraceSummary } from '@/types';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

afterEach(() => {
  cleanup();
});

function sampleEvent(overrides?: Partial<TraceEvent>): TraceEvent {
  return {
    trace_id: 'trace-1',
    turn_id: 'turn-1',
    seq: 1,
    stage: 'turn.start',
    status: 'start',
    message: 'start',
    timestamp: 1,
    ...overrides,
  };
}

describe('ChatTracePanel', () => {
  it('renders trace events', () => {
    render(<ChatTracePanel events={[sampleEvent()]} loading={false} />);

    expect(screen.getByText('chat.traceTitle')).toBeInTheDocument();
    expect(screen.getByText('turn › start')).toBeInTheDocument();
    expect(screen.getAllByText('start').length).toBeGreaterThan(0);
  });

  it('collapses and expands panel', async () => {
    const user = userEvent.setup();
    render(<ChatTracePanel events={[sampleEvent()]} loading={false} />);

    await user.click(screen.getByRole('button', { name: 'chat.traceCollapse' }));
    expect(screen.queryByText('turn › start')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'chat.traceExpand' }));
    expect(screen.getByText('turn › start')).toBeInTheDocument();
  });

  it('renders rag trace summary', () => {
    const ragTrace: RAGTraceSummary = {
      item_count: 1,
      total_candidate_budget: 100,
      max_rerank_docs: 32,
      max_final_chunks: 15,
      top_k_returned: 3,
      summary_mode: 'fallback_all',
      items: [{
        priority: 1,
        item_type: 'primary',
        retrieval_mode: 'hybrid',
        purpose: 'answer the core question',
        query: 'Go scheduler GMP model',
        recall_budget: 40,
        bm25_k: 20,
        dense_k: 20,
        rerank_budget: 10,
        source_count: 2,
      }],
    };

    render(<ChatTracePanel events={[]} ragTrace={ragTrace} loading={false} />);

    expect(screen.getByText('chat.traceRagSummary')).toBeInTheDocument();
    expect(screen.getByText('chat.traceBudget')).toBeInTheDocument();
    expect(screen.getByText('answer the core question')).toBeInTheDocument();
  });
});
