import type { Agent } from '@/types';

const BUILTIN_AGENT_ORDER = ['knowledge_base', 'general', 'search'] as const;

export const BUILTIN_CHAT_AGENTS: Agent[] = [
  {
    id: 'knowledge_base',
    name: '知识库助手',
    description: '个人知识查询专家，可引用知识库内容',
    icon: '📚',
  },
  {
    id: 'general',
    name: '通用聊天助手',
    description: '日常对话、文本润色、头脑风暴和内容创作',
    icon: '💬',
  },
  {
    id: 'search',
    name: '网络搜索助手',
    description: '外部信息获取专家，搜索互联网获取最新信息',
    icon: '🔍',
  },
];

function normalizeAgent(raw: Agent, fallback?: Agent): Agent {
  const id = String(raw.id || fallback?.id || '').trim();
  const name = String(raw.name || fallback?.name || id).trim();
  const description = String(raw.description || fallback?.description || '').trim();
  const icon = String(raw.icon || fallback?.icon || '🤖').trim();

  return {
    id,
    name,
    description,
    icon,
  };
}

export function mergeAgentsWithBuiltins(remoteAgents?: Agent[] | null): Agent[] {
  const byId = new Map<string, Agent>();

  for (const builtin of BUILTIN_CHAT_AGENTS) {
    byId.set(builtin.id, normalizeAgent(builtin));
  }

  for (const item of remoteAgents || []) {
    const id = String(item?.id || '').trim();
    if (!id) {
      continue;
    }

    byId.set(id, normalizeAgent(item, byId.get(id)));
  }

  const ordered: Agent[] = [];
  for (const id of BUILTIN_AGENT_ORDER) {
    const matched = byId.get(id);
    if (!matched) {
      continue;
    }
    ordered.push(matched);
    byId.delete(id);
  }

  for (const agent of byId.values()) {
    ordered.push(agent);
  }

  return ordered;
}
