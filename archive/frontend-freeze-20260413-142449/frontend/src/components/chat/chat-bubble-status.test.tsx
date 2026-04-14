import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ChatBubble } from './chat-bubble';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('ChatBubble inline status', () => {
  it('shows assistant status line when provided', () => {
    render(
      <ChatBubble
        message={{
          id: 1,
          session_id: 's1',
          role: 'assistant',
          content: 'hello',
          created_at: new Date().toISOString(),
        }}
        statusLine={{ text: '正在检索资料…', tone: 'progress', live: true, stage: 'router.start', timestamp: Date.now() }}
      />
    );

    expect(screen.getByText('正在检索资料…')).toBeInTheDocument();
  });

  it('does not show status line for user message', () => {
    render(
      <ChatBubble
        message={{
          id: 2,
          session_id: 's1',
          role: 'user',
          content: 'question',
          created_at: new Date().toISOString(),
        }}
        statusLine={{ text: 'should not appear', tone: 'progress', live: true, stage: 'router.start', timestamp: Date.now() }}
      />
    );

    expect(screen.queryByText('should not appear')).not.toBeInTheDocument();
  });
});
