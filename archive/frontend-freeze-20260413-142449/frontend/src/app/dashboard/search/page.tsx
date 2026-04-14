'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Search, FileText, MessageSquare, Loader2, ArrowRight, FolderOpen } from 'lucide-react';
import { searchApi } from '@/lib/api/client';
import type { SearchResult } from '@/types';
import { Button } from '@/components/ui/button';

function resolveWorkspaceId(result: SearchResult): string | null {
  if (result.source_type === 'workspace') {
    return result.id;
  }
  // Use workspace_id for correct navigation (falls back to parent_id for backward compatibility)
  return result.workspace_id || result.parent_id || null;
}

export default function SearchPage() {
  const router = useRouter();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [hasSearched, setHasSearched] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const grouped = useMemo(
    () => ({
      workspace: results.filter((item) => item.source_type === 'workspace'),
      document: results.filter((item) => item.source_type === 'document'),
      note: results.filter((item) => item.source_type === 'note'),
      session: results.filter((item) => item.source_type === 'session'),
    }),
    [results]
  );

  const handleSearch = async () => {
    if (!query.trim()) return;

    setLoading(true);
    setHasSearched(true);

    try {
      const response = await searchApi.search(query);
      if (response.success && response.data) {
        setResults(response.data.results || []);
      }
    } catch {
      setResults([]);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="max-w-4xl mx-auto p-8 text-foreground">
      <div className="text-center mb-8">
        <h1 className="text-2xl font-semibold mb-2">搜索</h1>
        <p className="text-muted-foreground">在工作区中搜索文档、笔记和对话</p>
      </div>

      <div className="relative mb-8">
        <div className="flex items-center gap-2 px-4 py-3 rounded-xl bg-card border border-border focus-within:ring-2 focus-within:ring-primary">
          <Search className="w-5 h-5 text-muted-foreground shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                void handleSearch();
              }
            }}
            placeholder="输入搜索关键词..."
            className="flex-1 bg-transparent border-none outline-none text-foreground placeholder:text-muted-foreground text-lg"
          />
          <Button onClick={() => void handleSearch()} disabled={loading || !query.trim()} size="sm">
            {loading ? <Loader2 className="w-4 h-4 animate-spin" /> : '搜索'}
          </Button>
        </div>
      </div>

      {hasSearched && (
        <div className="space-y-4">
          {loading ? (
            <div className="text-center py-12">
              <Loader2 className="w-8 h-8 mx-auto text-primary animate-spin mb-4" />
              <p className="text-muted-foreground">搜索中...</p>
            </div>
          ) : results.length === 0 ? (
            <div className="text-center py-12 text-muted-foreground">
              <Search className="w-12 h-12 mx-auto mb-4 opacity-50" />
              <p className="text-lg mb-2">未找到相关结果</p>
              <p className="text-sm">试试其他关键词</p>
            </div>
          ) : (
            <>
              <div className="text-sm text-muted-foreground">找到 {results.length} 个结果</div>
              <ResultGroup title="工作区" items={grouped.workspace} onSelect={(item) => {
                const workspaceId = resolveWorkspaceId(item);
                if (workspaceId) router.push(`/dashboard/${workspaceId}`);
              }} />
              <ResultGroup title="文档" items={grouped.document} onSelect={(item) => {
                const workspaceId = resolveWorkspaceId(item);
                if (workspaceId) router.push(`/dashboard/${workspaceId}?tab=docs`);
              }} />
              <ResultGroup title="笔记" items={grouped.note} onSelect={(item) => {
                const workspaceId = resolveWorkspaceId(item);
                if (workspaceId) router.push(`/dashboard/${workspaceId}?tab=notes`);
              }} />
              <ResultGroup title="对话" items={grouped.session} onSelect={(item) => {
                const workspaceId = resolveWorkspaceId(item);
                if (workspaceId) router.push(`/dashboard/${workspaceId}?tab=chat`);
              }} />
            </>
          )}
        </div>
      )}

      {!hasSearched && (
        <div className="text-center py-12 text-muted-foreground">
          <Search className="w-12 h-12 mx-auto mb-4 opacity-50" />
          <p className="text-lg mb-2">输入关键词开始搜索</p>
          <p className="text-sm">支持按类型分组查看并跳转</p>
        </div>
      )}
    </div>
  );
}

function ResultGroup({
  title,
  items,
  onSelect,
}: {
  title: string;
  items: SearchResult[];
  onSelect: (item: SearchResult) => void;
}) {
  if (!items.length) {
    return null;
  }

  return (
    <div className="space-y-2">
      <div className="text-xs uppercase tracking-wider text-muted-foreground">{title}</div>
      {items.map((result) => (
        <button
          key={result.id}
          onClick={() => onSelect(result)}
          className="w-full text-left block p-4 rounded-xl bg-card border border-border hover:border-border/80 transition-colors group"
        >
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 rounded-lg bg-accent flex items-center justify-center shrink-0 mt-0.5">
              {result.source_type === 'workspace' ? (
                <FolderOpen className="w-4 h-4 text-indigo-400" />
              ) : result.source_type === 'session' ? (
                <MessageSquare className="w-4 h-4 text-purple-400" />
              ) : (
                <FileText className="w-4 h-4 text-blue-400" />
              )}
            </div>

            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <h3 className="font-medium group-hover:text-primary transition-colors truncate">{result.title}</h3>
                {result.score ? (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-primary/20 text-primary shrink-0">
                    {Math.round(result.score * 100)}%
                  </span>
                ) : null}
              </div>

              {result.summary ? <p className="text-sm text-muted-foreground mt-1 line-clamp-2">{result.summary}</p> : null}
              {result.content ? (
                <p className="text-xs text-muted-foreground mt-2 line-clamp-3 font-mono">{result.content.substring(0, 200)}...</p>
              ) : null}
            </div>

            <ArrowRight className="w-4 h-4 text-muted-foreground group-hover:text-primary transition-colors shrink-0" />
          </div>
        </button>
      ))}
    </div>
  );
}
