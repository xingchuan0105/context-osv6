'use client';

import { X } from 'lucide-react';
import type { Agent } from '@/types';

interface AgentChipProps {
  agent: Agent;
  onRemove?: () => void;
}

export function AgentChip({ agent, onRemove }: AgentChipProps) {
  return (
    <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-xl bg-primary/12 text-primary border border-primary/25 text-sm shadow-[var(--shadow-sm)] shrink-0">
      <span>{agent.icon || '🤖'}</span>
      <span className="font-medium">{agent.name}</span>
      {onRemove && (
        <button
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onRemove();
          }}
          className="ml-0.5 hover:bg-primary/18 rounded-full p-0.5 transition-colors"
        >
          <X className="w-3 h-3" />
        </button>
      )}
    </span>
  );
}
