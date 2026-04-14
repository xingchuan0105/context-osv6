/**
 * SearchDialog - 全局搜索命令面板
 * 
 * 功能：
 * - 全局搜索（Cmd/Ctrl + K 打开，Esc 关闭）
 * - 搜索结果按实体类型分组（工作区/文档/笔记/会话）
 * - 键盘导航（上下键选择，回车跳转）
 * - 实时搜索（300ms 防抖）
 * 
 * 快捷键：
 * - Cmd/Ctrl + K: 打开搜索
 * - Esc: 关闭搜索
 * - 上下箭头: 导航结果
 * - 回车: 跳转到选中结果
 * 
 * 搜索范围：
 * - RAG 默认锁定当前工作区
 * - 支持跨工作区导航
 */

'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Search, FileText, MessageSquare, FolderOpen, X } from 'lucide-react';
import { useAppStore } from '@/stores/useAppStore';
import { searchApi } from '@/lib/api/client';
import type { SearchResult } from '@/types';

function resolveTargetWorkspaceId(result: SearchResult): string | null {
  if (result.source_type === 'workspace') {
    return result.id;
  }
  // Use workspace_id for correct navigation (falls back to parent_id for backward compatibility)
  return result.workspace_id || result.parent_id || null;
}

export function SearchDialog() {
  const router = useRouter();
  const { searchDialogOpen, setSearchDialogOpen } = useAppStore();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    const onGlobalKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setSearchDialogOpen((prev) => !prev);
      }
      if (event.key === 'Escape' && searchDialogOpen) {
        setSearchDialogOpen(false);
      }
    };

    document.addEventListener('keydown', onGlobalKeyDown);
    return () => document.removeEventListener('keydown', onGlobalKeyDown);
  }, [searchDialogOpen, setSearchDialogOpen]);

  const handleSearch = useCallback(async (searchQuery: string) => {
    if (!searchQuery.trim()) {
      setResults([]);
      setSelectedIndex(0);
      return;
    }

    setLoading(true);
    try {
      const response = await searchApi.search(searchQuery);
      if (response.success && response.data) {
        setResults(response.data.results || []);
        setSelectedIndex(0);
      }
    } catch {
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      void handleSearch(query);
    }, 300);

    return () => clearTimeout(timer);
  }, [handleSearch, query]);

  const handleSelect = (result: SearchResult) => {
    const workspaceId = resolveTargetWorkspaceId(result);
    if (!workspaceId) return;

    // Navigate based on result type
    switch (result.source_type) {
      case 'workspace':
        router.push(`/dashboard/${workspaceId}`);
        break;
      case 'document':
        router.push(`/dashboard/${workspaceId}?tab=docs`);
        break;
      case 'note':
        router.push(`/dashboard/${workspaceId}?tab=notes`);
        break;
      case 'session':
        router.push(`/dashboard/${workspaceId}?tab=chat`);
        break;
      default:
        router.push(`/dashboard/${workspaceId}`);
    }

    setSearchDialogOpen(false);
    setQuery('');
    setResults([]);
    setSelectedIndex(0);
  };

  const handleInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelectedIndex((prev) => Math.min(prev + 1, results.length - 1));
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelectedIndex((prev) => Math.max(prev - 1, 0));
    }

    if (event.key === 'Enter' && results[selectedIndex]) {
      event.preventDefault();
      handleSelect(results[selectedIndex]);
    }
  };

  const grouped = useMemo(
    () => ({
      workspaces: results.filter((item) => item.source_type === 'workspace'),
      documents: results.filter((item) => item.source_type === 'document'),
      notes: results.filter((item) => item.source_type === 'note'),
      sessions: results.filter((item) => item.source_type === 'session'),
    }),
    [results]
  );

  if (!searchDialogOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh]">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200" onClick={() => setSearchDialogOpen(false)} />

      <div className="relative w-full max-w-2xl mx-4 bg-card/94 border border-border rounded-3xl shadow-[var(--shadow-lg)] backdrop-blur-xl overflow-hidden animate-in zoom-in-95 slide-in-from-top-4 duration-300">
        {/* Search Input Area */}
        <div className="flex items-center px-5 py-4 border-b border-border bg-[linear-gradient(90deg,rgba(124,58,237,0.08),rgba(37,99,235,0.05),transparent)]">
          <Search className="w-5 h-5 text-muted-foreground shrink-0 mr-3" />
          <input
            type="text"
            placeholder="搜索工作区、文档、对话、笔记..."
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={handleInputKeyDown}
            autoFocus
            className="flex-1 h-12 px-4 bg-transparent border-none outline-none text-foreground placeholder:text-muted-foreground text-lg"
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              className="p-1.5 rounded-lg hover:bg-accent transition-colors"
            >
              <X className="w-4 h-4 text-muted-foreground hover:text-foreground" />
            </button>
          )}
          <kbd className="ml-2 px-2 py-1 text-xs text-muted-foreground bg-background border border-border rounded">esc</kbd>
        </div>

        {/* Results Area */}
        <div className="max-h-[50vh] overflow-auto">
          {loading && (
            <div className="flex items-center justify-center py-12">
              <div className="flex items-center gap-3 text-muted-foreground">
                <div className="w-5 h-5 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
                <span>搜索中...</span>
              </div>
            </div>
          )}

          {!loading && query && results.length === 0 && (
            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
              <Search className="w-10 h-10 mb-3 opacity-50" />
              <p>未找到相关结果</p>
              <p className="text-sm mt-1">试试其他关键词</p>
            </div>
          )}

          {!loading && !query && (
            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
              <div className="w-16 h-16 mb-4 rounded-3xl bg-[linear-gradient(135deg,rgba(124,58,237,0.16),rgba(37,99,235,0.12))] border border-white/6 flex items-center justify-center">
                <Search className="w-8 h-8 text-violet-300" />
              </div>
              <p className="text-lg font-medium mb-1">全局搜索</p>
              <p className="text-sm">输入关键词开始搜索工作区、文档、对话和笔记</p>
              <div className="flex items-center gap-3 mt-4 text-xs">
                <span className="flex items-center gap-1">
                  <kbd className="px-2 py-1 bg-accent rounded">↑↓</kbd> 导航
                </span>
                <span className="flex items-center gap-1">
                  <kbd className="px-2 py-1 bg-accent rounded">↵</kbd> 跳转
                </span>
              </div>
            </div>
          )}

          {results.length > 0 && (
            <div className="p-2 space-y-1">
              <GroupSection title="工作区" items={grouped.workspaces} selectedIndex={selectedIndex} allResults={results} onSelect={handleSelect} icon={<FolderOpen className="w-4 h-4 text-indigo-400" />} />
              <GroupSection title="文档" items={grouped.documents} selectedIndex={selectedIndex} allResults={results} onSelect={handleSelect} icon={<FileText className="w-4 h-4 text-blue-400" />} />
              <GroupSection title="笔记" items={grouped.notes} selectedIndex={selectedIndex} allResults={results} onSelect={handleSelect} icon={<FileText className="w-4 h-4 text-green-400" />} />
              <GroupSection title="对话" items={grouped.sessions} selectedIndex={selectedIndex} allResults={results} onSelect={handleSelect} icon={<MessageSquare className="w-4 h-4 text-purple-400" />} />
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-3 border-t border-border bg-background/45 flex items-center justify-between text-xs text-muted-foreground">
          <div className="flex items-center gap-4">
            <span className="flex items-center gap-1.5">
              <kbd className="px-1.5 py-0.5 bg-background border border-border rounded">↑↓</kbd>
              导航
            </span>
            <span className="flex items-center gap-1.5">
              <kbd className="px-1.5 py-0.5 bg-background border border-border rounded">↵</kbd>
              跳转
            </span>
          </div>
          <span className="text-muted-foreground/60">支持跨工作区搜索</span>
        </div>
      </div>
    </div>
  );
}

function GroupSection({
  title,
  items,
  selectedIndex,
  allResults,
  onSelect,
  icon,
}: {
  title: string;
  items: SearchResult[];
  selectedIndex: number;
  allResults: SearchResult[];
  onSelect: (result: SearchResult) => void;
  icon: React.ReactNode;
}) {
  if (!items.length) {
    return null;
  }

  return (
    <>
      <div className="px-3 py-2 text-xs font-semibold text-muted-foreground/70 uppercase tracking-wider">{title}</div>
      {items.map((result) => {
        const isSelected = selectedIndex === allResults.indexOf(result);
        return (
          <button
            key={result.id}
            onClick={() => onSelect(result)}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-left transition-all duration-150 ${
              isSelected
                ? 'bg-primary/10 border border-primary/20 shadow-[var(--shadow-sm)]'
                : 'hover:bg-accent/60 border border-transparent'
            }`}
          >
            <div className={`shrink-0 ${isSelected ? 'scale-110' : ''} transition-transform`}>
              {icon}
            </div>
            <div className="flex-1 min-w-0">
              <div className={`truncate font-medium ${isSelected ? 'text-primary' : ''}`}>{result.title}</div>
              {result.summary && <div className="text-xs text-muted-foreground truncate mt-0.5">{result.summary}</div>}
            </div>
          </button>
        );
      })}
    </>
  );
}
