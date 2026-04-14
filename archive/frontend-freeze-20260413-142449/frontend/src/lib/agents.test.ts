import { describe, expect, it } from 'vitest';
import { BUILTIN_CHAT_AGENTS, mergeAgentsWithBuiltins } from './agents';
import type { Agent } from '@/types';

describe('agents', () => {
  it('includes built-in assistants when remote list is empty', () => {
    const merged = mergeAgentsWithBuiltins([]);

    expect(merged.map((agent) => agent.id)).toEqual(['knowledge_base', 'general', 'search']);
    expect(merged).toHaveLength(BUILTIN_CHAT_AGENTS.length);
  });

  it('preserves remote agents and appends custom ones', () => {
    const remoteAgents: Agent[] = [
      {
        id: 'general',
        name: '通用聊天助手',
        description: '来自后端',
        icon: '💬',
      },
      {
        id: 'custom_tool',
        name: '自定义助手',
        description: '额外助手',
        icon: '🧰',
      },
    ];

    const merged = mergeAgentsWithBuiltins(remoteAgents);

    expect(merged.map((agent) => agent.id)).toEqual([
      'knowledge_base',
      'general',
      'search',
      'custom_tool',
    ]);
    expect(merged.find((agent) => agent.id === 'general')?.description).toBe('来自后端');
  });

  it('ignores remote agents without valid id', () => {
    const remoteAgents = [
      {
        id: '',
        name: '无效助手',
        description: 'no id',
        icon: '❌',
      },
    ] as Agent[];

    const merged = mergeAgentsWithBuiltins(remoteAgents);

    expect(merged.map((agent) => agent.id)).toEqual(['knowledge_base', 'general', 'search']);
  });
});
