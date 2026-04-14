import { describe, it, expect } from 'vitest';
import type { 
  User, 
  KnowledgeBase, 
  Note,
  ChatMessage,
  Agent,
  Document,
  AuthResponse 
} from '@/types';

describe('Types', () => {
  describe('User', () => {
    it('should have required fields', () => {
      const user: User = {
        id: '123',
        email: 'test@example.com',
      };
      expect(user.id).toBe('123');
      expect(user.email).toBe('test@example.com');
    });

    it('should have optional fields', () => {
      const user: User = {
        id: '123',
        email: 'test@example.com',
        full_name: 'Test User',
        avatar_url: 'https://example.com/avatar.png',
      };
      expect(user.full_name).toBe('Test User');
      expect(user.avatar_url).toBe('https://example.com/avatar.png');
    });
  });

  describe('KnowledgeBase', () => {
    it('should have required fields', () => {
      const kb: KnowledgeBase = {
        id: 'kb-123',
        user_id: 'user-123',
        title: 'My Workspace',
        created_at: '2024-01-01T00:00:00Z',
      };
      expect(kb.id).toBe('kb-123');
      expect(kb.title).toBe('My Workspace');
    });
  });

  describe('Note', () => {
    it('should have note_type field', () => {
      const note: Note = {
        id: 'note-123',
        kb_id: 'kb-123',
        user_id: 'user-123',
        content: 'Note content',
        note_type: 'draft',
        is_shared: false,
        created_at: '2024-01-01T00:00:00Z',
        updated_at: '2024-01-01T00:00:00Z',
      };
      expect(note.note_type).toBe('draft');
    });

    it('should support committed note_type', () => {
      const note: Note = {
        id: 'note-123',
        kb_id: 'kb-123',
        user_id: 'user-123',
        content: 'Note content',
        note_type: 'committed',
        is_shared: false,
        created_at: '2024-01-01T00:00:00Z',
        updated_at: '2024-01-01T00:00:00Z',
      };
      expect(note.note_type).toBe('committed');
    });
  });

  describe('ChatMessage', () => {
    it('should support user role', () => {
      const msg: ChatMessage = {
        id: 1,
        session_id: 'session-123',
        role: 'user',
        content: 'Hello',
        created_at: '2024-01-01T00:00:00Z',
      };
      expect(msg.role).toBe('user');
    });

    it('should support assistant role with citations', () => {
      const msg: ChatMessage = {
        id: 2,
        session_id: 'session-123',
        role: 'assistant',
        content: 'Based on the document...',
        citations: [
          { citation_id: 1, doc_id: 'doc-1', doc_name: 'Test.pdf', content: 'Reference content', score: 0.95 }
        ],
        created_at: '2024-01-01T00:00:00Z',
      };
      expect(msg.role).toBe('assistant');
      expect(msg.citations).toHaveLength(1);
      expect(msg.citations?.[0].citation_id).toBe(1);
    });
  });

  describe('Agent', () => {
    it('should have agent types', () => {
      const agents: Agent[] = [
        { id: 'general', name: '通用聊天助手', description: '日常对话', icon: '💬' },
        { id: 'knowledge_base', name: '知识库助手', description: '知识查询', icon: '📚' },
        { id: 'search', name: '搜索助手', description: '外部搜索', icon: '🔍' },
      ];
      expect(agents).toHaveLength(3);
      expect(agents[0].id).toBe('general');
    });
  });

  describe('Document', () => {
    it('should have document status', () => {
      const doc: Document = {
        id: 'doc-123',
        kb_id: 'kb-123',
        user_id: 'user-123',
        file_name: 'test.pdf',
        status: 'pending',
        chunk_count: 0,
        created_at: '2024-01-01T00:00:00Z',
      };
      expect(['pending', 'enqueueing', 'queued', 'processing', 'completed', 'failed']).toContain(doc.status);
    });
  });

  describe('AuthResponse', () => {
    it('should have success response structure', () => {
      const response: AuthResponse = {
        success: true,
        data: {
          token: 'jwt-token',
          user: { id: '123', email: 'test@example.com' },
        },
      };
      expect(response.success).toBe(true);
      expect(response.data?.token).toBe('jwt-token');
    });

    it('should have error response structure', () => {
      const response: AuthResponse = {
        success: false,
        error: 'Invalid credentials',
      };
      expect(response.success).toBe(false);
      expect(response.error).toBe('Invalid credentials');
    });
  });
});
