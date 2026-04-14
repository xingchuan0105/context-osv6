'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Bot, Send, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { AgentChip } from './agent-chip';
import type { Agent } from '@/types';

interface ChatSendPayload {
  content: string;
  mentionedAgent?: Agent | null;
}

interface ChatInputProps {
  selectedAgent: Agent | null;
  agents: Agent[];
  onAgentSelect: (agent: Agent) => void;
  onSend: (payload: ChatSendPayload) => void;
  disabled?: boolean;
}

type PickerMode = 'default' | 'mention';

function normalizeAgentToken(agent: Agent) {
  return {
    id: agent.id.toLowerCase(),
    name: agent.name.replace(/\s+/g, '').toLowerCase(),
  };
}

function placeCaretAtEnd(node: HTMLElement | null) {
  if (!node) return;
  node.focus();
  const selection = window.getSelection();
  if (!selection) return;
  const range = document.createRange();
  range.selectNodeContents(node);
  range.collapse(false);
  selection.removeAllRanges();
  selection.addRange(range);
}

export function ChatInput({ selectedAgent, agents, onAgentSelect, onSend, disabled }: ChatInputProps) {
  const { t } = useTranslation();
  const [message, setMessage] = useState('');
  const [mentionedAgent, setMentionedAgent] = useState<Agent | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerMode, setPickerMode] = useState<PickerMode>('mention');
  const [mentionDraft, setMentionDraft] = useState('');
  const [inputHeight, setInputHeight] = useState(56);
  const [isResizing, setIsResizing] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);
  const defaultAgentButtonRef = useRef<HTMLButtonElement>(null);
  const editableRef = useRef<HTMLDivElement>(null);
  const resizeStartY = useRef(0);
  const resizeStartHeight = useRef(0);

  const availableAgents = useMemo(() => agents, [agents]);
  const filteredAgents = useMemo(() => {
    if (!pickerOpen || pickerMode !== 'mention') return availableAgents;
    const q = mentionDraft.trim().toLowerCase();
    if (!q) return availableAgents;
    return availableAgents.filter((agent) => {
      const token = normalizeAgentToken(agent);
      return token.id.startsWith(q) || token.name.startsWith(q);
    });
  }, [availableAgents, mentionDraft, pickerMode, pickerOpen]);

  const openPicker = useCallback((mode: PickerMode) => {
    setPickerMode(mode);
    setPickerOpen(true);
  }, []);

  const closePicker = useCallback(() => {
    setPickerOpen(false);
    setMentionDraft('');
  }, []);

  const stripLeadingMentionDraft = (input: string) => {
    const m = input.match(/^@([a-zA-Z0-9_-]+)\s*([\s\S]*)$/);
    if (!m) return input;
    return (m[2] || '').replace(/^\s+/, '');
  };

  const stripLeadingAtChar = (input: string) => input.replace(/^@\s*/, '');

  const maybeConvertLeadingMention = (next: string) => {
    if (mentionedAgent) return false;

    const draftOnly = next.match(/^@([a-zA-Z0-9_-]*)$/);
    if (draftOnly) {
      setMentionDraft(draftOnly[1] || '');
      openPicker('mention');
      return false;
    }

    const withSpace = next.match(/^@([a-zA-Z0-9_-]+)\s+([\s\S]*)$/);
    if (!withSpace) return false;

    const token = withSpace[1].toLowerCase();
    const rest = (withSpace[2] || '').replace(/^\s+/, '');
    const matched = availableAgents.find((agent) => {
      const norm = normalizeAgentToken(agent);
      return token === norm.id || token === norm.name;
    });

    if (!matched) {
      setMentionDraft(token);
      openPicker('mention');
      return false;
    }

    setMentionedAgent(matched);
    setMessage(rest);
    closePicker();
    requestAnimationFrame(() => {
      if (editableRef.current) {
        editableRef.current.innerText = rest;
      }
      placeCaretAtEnd(editableRef.current);
    });
    return true;
  };

  const handleMessageChange = (next: string) => {
    if (next.endsWith('@')) {
      setMentionDraft('');
      openPicker('mention');
    }

    if (maybeConvertLeadingMention(next)) {
      return;
    }

    setMessage(next);
  };

  const handlePickAgent = (agent: Agent) => {
    if (pickerMode === 'default') {
      onAgentSelect(agent);
      closePicker();
      placeCaretAtEnd(editableRef.current);
      return;
    }

    setMentionedAgent(agent);
    setMessage((prev) => {
      const noDraft = prev.startsWith('@') ? stripLeadingMentionDraft(prev) : prev;
      const noTailAt = noDraft.endsWith('@') ? noDraft.slice(0, -1) : noDraft;
      return stripLeadingAtChar(noTailAt);
    });
    closePicker();
    requestAnimationFrame(() => {
      const next = stripLeadingAtChar(editableRef.current?.innerText || message || '');
      if (editableRef.current) {
        editableRef.current.innerText = next;
      }
      placeCaretAtEnd(editableRef.current);
    });
  };

  const handleSubmit = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (disabled) return;

    const content = message.trim();
    if (!content) return;

    onSend({ content, mentionedAgent });
    setMessage('');
    setMentionedAgent(null);
    closePicker();
    requestAnimationFrame(() => {
      if (editableRef.current) {
        editableRef.current.innerText = '';
      }
      placeCaretAtEnd(editableRef.current);
    });
  };

  const insertLineBreakAtCaret = () => {
    const selection = window.getSelection();
    const editable = editableRef.current;
    if (!selection || !editable || selection.rangeCount === 0) return;
    const range = selection.getRangeAt(0);
    range.deleteContents();
    const br = document.createElement('br');
    range.insertNode(br);
    const placeholder = document.createTextNode('\u00A0');
    range.setStartAfter(br);
    range.insertNode(placeholder);
    range.setStartAfter(placeholder);
    range.collapse(true);
    selection.removeAllRanges();
    selection.addRange(range);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape' && pickerOpen) {
      e.preventDefault();
      closePicker();
      return;
    }

    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      insertLineBreakAtCaret();
      handleInput();
      return;
    }

    if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleInput = () => {
    if (!editableRef.current) return;
    let text = editableRef.current.innerText || '';
    if (mentionedAgent && text.startsWith('@')) {
      const cleaned = stripLeadingAtChar(text);
      if (cleaned !== text) {
        editableRef.current.innerText = cleaned;
        text = cleaned;
        placeCaretAtEnd(editableRef.current);
      }
    }
    handleMessageChange(text);
  };

  useEffect(() => {
    if (!pickerOpen) return;

    const onMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (pickerRef.current?.contains(target)) return;
      if (defaultAgentButtonRef.current?.contains(target)) return;
      closePicker();
    };

    document.addEventListener('mousedown', onMouseDown);
    return () => {
      document.removeEventListener('mousedown', onMouseDown);
    };
  }, [closePicker, pickerOpen]);

  const handleResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
    resizeStartY.current = e.clientY;
    resizeStartHeight.current = inputHeight;
  };

  useEffect(() => {
    if (!isResizing) return;

    const handleMouseMove = (e: MouseEvent) => {
      const delta = resizeStartY.current - e.clientY;
      const newHeight = Math.max(40, Math.min(300, resizeStartHeight.current + delta));
      setInputHeight(newHeight);
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isResizing]);

  return (
    <div className="border-t border-border p-3 md:p-4 bg-card/84 backdrop-blur-xl relative">
      <div className="flex items-center justify-between gap-2 mb-2">
        <Button
          ref={defaultAgentButtonRef}
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => openPicker('default')}
          className="h-9 md:h-9 px-2.5 text-foreground hover:text-foreground border border-border bg-background/55 shadow-[var(--shadow-sm)]"
          title={t('chat.selectAgent')}
          disabled={disabled || availableAgents.length === 0}
        >
          <Bot className="w-4 h-4" />
          <span className="text-sm ml-1 font-medium">
            {selectedAgent?.name || t('chat.selectAgent')}
          </span>
        </Button>

        <span className="text-sm md:text-base font-semibold text-primary hidden">
          {t('chat.selectAgentHintStrong')}
        </span>
      </div>

      {pickerOpen && (
        <div
          ref={pickerRef}
          className="absolute left-3 right-3 md:left-4 md:right-auto md:w-80 bottom-[calc(100%+10px)] rounded-2xl border border-border bg-popover/96 shadow-[var(--shadow-lg)] backdrop-blur-xl p-1.5 z-30 max-h-[50vh] overflow-auto"
        >
          {(pickerMode === 'default' ? availableAgents : filteredAgents).length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">
              {t('chat.noAgents')}
            </div>
          ) : (
            (pickerMode === 'default' ? availableAgents : filteredAgents).map((agent) => (
              <button
                key={`${pickerMode}-${agent.id}`}
                type="button"
                onClick={() => handlePickAgent(agent)}
                className="w-full text-left px-3 py-2.5 md:py-2.5 rounded-xl hover:bg-accent transition-colors min-h-[44px]"
              >
                <div className="flex items-center gap-2">
                  <span>{agent.icon || '🤖'}</span>
                  <span className="text-sm font-medium">{agent.name}</span>
                  <span className="ml-auto text-xs text-muted-foreground">@{agent.id}</span>
                </div>
                <div className="text-xs text-muted-foreground mt-0.5 line-clamp-1">{agent.description}</div>
              </button>
            ))
          )}
        </div>
      )}

      <div className="absolute -top-3 left-3 right-3 flex justify-center pointer-events-none">
        <div
          className="h-2 w-16 rounded-full bg-white/10 cursor-ns-resize shadow-[var(--shadow-sm)] pointer-events-auto"
          onMouseDown={handleResizeStart}
          style={{ opacity: isResizing ? 0.75 : 0.5 }}
        />
      </div>

      <form onSubmit={handleSubmit} className="flex items-end gap-2">
        <div className="flex-1 relative">
          <div
            className="flex flex-nowrap items-center gap-1.5 border border-border rounded-3xl bg-background/72 px-3 py-2.5 min-w-0 cursor-text focus-within:border-primary/45 focus-within:ring-2 focus-within:ring-primary/18 focus-within:bg-background/86 transition-colors overflow-auto group shadow-[var(--shadow-sm)] backdrop-blur-sm"
            onClick={() => placeCaretAtEnd(editableRef.current)}
            style={{ height: `${inputHeight}px`, maxHeight: '300px', overflowY: 'auto' }}
          >
            {mentionedAgent && (
              <AgentChip agent={mentionedAgent} onRemove={() => setMentionedAgent(null)} />
            )}
            <div
              ref={editableRef}
              contentEditable
              onInput={handleInput}
              onKeyDown={handleKeyDown}
              className="chat-input-editor flex-1 min-w-[120px] outline-none focus:outline-none focus-visible:outline-none text-[15px] leading-[1.75] text-[color:var(--text-primary)] empty:before:content-[attr(data-placeholder)] empty:before:text-muted-foreground"
              data-placeholder={disabled ? t('chat.sending') : t('chat.placeholder')}
              suppressContentEditableWarning
              style={{ outline: 'none', boxShadow: 'none' }}
            />

          </div>

          <div className="absolute bottom-2 right-4 text-[10px] text-muted-foreground hidden md:flex items-center gap-2 pointer-events-none">
            <span>{t('dashboard.enterToSend')}</span>
            <span>•</span>
            <span>{t('dashboard.ctrlEnterNewline')}</span>
          </div>
        </div>

        <Button
          type="submit"
          size="icon"
          disabled={!message.trim() || disabled}
          className="h-[56px] w-[56px] md:h-[54px] md:w-[54px] shrink-0 rounded-2xl shadow-[var(--shadow-md)]"
        >
          {disabled ? (
            <Loader2 className="w-5 h-5 animate-spin" />
          ) : (
            <Send className="w-5 h-5" />
          )}
        </Button>
      </form>
    </div>
  );
}
