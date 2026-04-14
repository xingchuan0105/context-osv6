import { useState, useEffect } from 'react';
import { Bot, ChevronDown } from 'lucide-react';
import { agentsApi } from '@/lib/api/client';
import type { Agent } from '@/types';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';

interface AgentSelectorProps {
  selectedAgent: Agent | null;
  onSelect: (agent: Agent) => void;
}

export function AgentSelector({ selectedAgent, onSelect }: AgentSelectorProps) {
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchAgents = async () => {
      try {
        const response = await agentsApi.list();
        if (response.success && response.data) {
          setAgents(response.data.agents || []);
        }
      } catch (error) {
        console.error('Failed to fetch agents:', error);
      } finally {
        setLoading(false);
      }
    };

    fetchAgents();
  }, []);

  const getAgentIcon = (icon: string) => {
    return icon || '🤖';
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="flex items-center gap-2 text-muted-foreground hover:text-foreground/90">
          {loading ? (
            <Bot className="w-4 h-4 animate-pulse" />
          ) : (
            <>
              <span className="text-base">{selectedAgent ? getAgentIcon(selectedAgent.icon) : '🤖'}</span>
              <span className="text-sm">{selectedAgent?.name || '选择助手'}</span>
              <ChevronDown className="w-3 h-3" />
            </>
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuLabel>选择助手</DropdownMenuLabel>
        <DropdownMenuSeparator />
        {agents.map((agent) => (
          <DropdownMenuItem 
            key={agent.id}
            onClick={() => onSelect(agent)}
            className="flex items-center gap-3 py-3"
          >
            <span className="text-xl">{getAgentIcon(agent.icon)}</span>
            <div className="flex-1">
              <div className="font-medium text-foreground/90">{agent.name}</div>
              <div className="text-xs text-muted-foreground/80">{agent.description}</div>
            </div>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
