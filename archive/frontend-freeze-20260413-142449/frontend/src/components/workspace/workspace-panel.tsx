'use client';

import { useState } from 'react';
import { Plus, FolderOpen, Trash2 } from 'lucide-react';
import { kbApi } from '@/lib/api/client';
import { useAppStore } from '@/stores/useAppStore';
import type { KnowledgeBase } from '@/types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent } from '@/components/ui/card';
import { toast } from '@/components/ui/toaster';

export function WorkspacePanel() {
  const { currentWorkspace, setCurrentWorkspace } = useAppStore();
  const [workspaces, setWorkspaces] = useState<KnowledgeBase[]>([]);
  const [loading, setLoading] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [newTitle, setNewTitle] = useState('');

  // Load workspaces
  const loadWorkspaces = async () => {
    setLoading(true);
    try {
      const response = await kbApi.list();
      if (response.success) {
        setWorkspaces(response.data || []);
      }
    } catch (error) {
      console.error('Failed to load workspaces:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleCreate = async () => {
    if (!newTitle.trim()) return;
    
    try {
      const response = await kbApi.create(newTitle.trim());
      if (!response.success || !response.data) {
        toast.error(response.error || '创建工作区失败');
        return;
      }
      setNewTitle('');
      setShowCreate(false);
      void loadWorkspaces();
      setCurrentWorkspace(response.data);
      toast.success('工作区已创建');
    } catch (error) {
      console.error('Failed to create workspace:', error);
      toast.error('创建工作区失败');
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('确定要删除这个工作区吗？')) return;
    
    try {
      const response = await kbApi.delete(id);
      if (response.success) {
        loadWorkspaces();
        if (currentWorkspace?.id === id) {
          setCurrentWorkspace(null);
        }
      }
    } catch (error) {
      console.error('Failed to delete workspace:', error);
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border">
        <h2 className="text-sm font-medium text-foreground/80">工作区</h2>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setShowCreate(!showCreate)}
        >
          <Plus className="w-4 h-4" />
        </Button>
      </div>

      {/* Create Form */}
      {showCreate && (
        <div className="p-4 border-b border-border">
          <div className="flex gap-2">
            <Input
              placeholder="工作区名称"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
            />
            <Button size="sm" onClick={handleCreate}>
              创建
            </Button>
          </div>
        </div>
      )}

      {/* Workspace List */}
      <div className="flex-1 overflow-auto p-4">
        {loading ? (
          <div className="text-center text-muted-foreground/80 py-8">加载中...</div>
        ) : workspaces.length === 0 ? (
          <div className="text-center text-muted-foreground/80 py-8">
            <FolderOpen className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>暂无工作区</p>
            <p className="text-sm">点击 + 创建第一个工作区</p>
          </div>
        ) : (
          <div className="space-y-2">
            {workspaces.map((ws) => (
              <Card
                key={ws.id}
                className={`cursor-pointer transition-colors ${
                  currentWorkspace?.id === ws.id
                    ? 'border-indigo-500 bg-indigo-500/10'
                    : 'hover:border-border/80'
                }`}
                onClick={() => setCurrentWorkspace(ws)}
              >
                <CardContent className="p-3 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <FolderOpen className="w-4 h-4 text-indigo-400" />
                    <span className="text-sm">{ws.title}</span>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDelete(ws.id);
                    }}
                    className="h-7 w-7 p-0 text-muted-foreground/80 hover:text-red-400"
                  >
                    <Trash2 className="w-3 h-3" />
                  </Button>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
