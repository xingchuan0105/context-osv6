
import { useState, useRef, useEffect } from 'react';
import { 
  Edit, 
  FolderOpen, 
  Trash2, 
  Download, 
  X,
  Loader2
} from 'lucide-react';
import { kbApi, chatApi } from '@/lib/api/client';
import type { KnowledgeBase, ChatSession } from '@/types';

// Preset emojis for workspace icon
const EMOJI_PICKER = [
  '📁', '📂', '🗂️', '📚', '📖', '📝', '📋', '📌',
  '💼', '🎯', '🚀', '⚡', '🔥', '💡', '✨', '🌟',
  '💻', '🖥️', '📱', '🔧', '⚙️', '🛠️', '🔬', '🧪',
  '🏠', '🏢', '🌍', '🌉', '🏔️', '🌊', '☀️', '🌙',
  '🎨', '🎭', '🎪', '🎬', '🎵', '🎮', '🎲', '🃏',
  '🍕', '☕', '🍺', '🎂', '🎁', '❤️', '💌', '💬'
];

export function SettingsModal({
  kb,
  sessions,
  onClose,
  onUpdateKB,
  onDeleteGroup
}: {
  kb: KnowledgeBase | null;
  sessions: ChatSession[];
  onClose: () => void;
  onUpdateKB: (title: string, description: string, icon?: string) => void;
  onDeleteGroup: () => void;
}) {
  const [activeTab, setActiveTab] = useState<'general' | 'members' | 'danger'>('general');
  const [title, setTitle] = useState(kb?.title || '');
  const [description, setDescription] = useState(kb?.description || '');
  const [icon, setIcon] = useState(kb?.icon || '📁');
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const emojiPickerRef = useRef<HTMLDivElement>(null);

  // Close emoji picker when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (emojiPickerRef.current && !emojiPickerRef.current.contains(e.target as Node)) {
        setShowEmojiPicker(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleSave = async () => {
    if (!title.trim()) return;
    setSaving(true);
    try {
      await kbApi.update(kb!.id, { title, description, icon });
      onUpdateKB(title, description, icon);
      onClose();
    } catch (error) {
      console.error('Failed to update:', error);
    } finally {
      setSaving(false);
    }
  };

  const handleExport = () => {
    const data = {
      groupName: kb?.title,
      description: kb?.description,
      exportedAt: new Date().toISOString(),
      sessions: sessions.map(s => ({
        title: s.title,
        summary: s.summary,
        createdAt: s.created_at
      }))
    };
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${kb?.title || '讨论组'}-导出.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleClearHistory = async () => {
    if (!confirm('确定要清空所有对话历史吗？此操作不可恢复。')) return;
    
    try {
      for (const session of sessions) {
        const response = await chatApi.deleteSession(session.id);
        if (!response.success) {
          throw new Error(response.error || 'delete-session-failed');
        }
      }
      alert('对话历史已清空');
      onClose();
      window.location.reload();
    } catch (error) {
      console.error('Failed to clear history:', error);
    }
  };

  const handleDeleteGroup = async () => {
    if (!showDeleteConfirm) {
      setShowDeleteConfirm(true);
      return;
    }
    
    setDeleting(true);
    try {
      const response = await kbApi.delete(kb!.id);
      if (!response.success) {
        throw new Error(response.error || 'delete-kb-failed');
      }
      onDeleteGroup();
    } catch (error) {
      console.error('Failed to delete:', error);
      setShowDeleteConfirm(false);
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60" onClick={onClose} />
      
      <div className="relative w-full max-w-2xl h-[600px] bg-card border border-border rounded-xl flex overflow-hidden">
        {/* Sidebar */}
        <div className="w-48 border-r border-border p-4">
          <div className="flex items-center justify-between mb-6">
            <h2 className="font-semibold text-foreground">设置</h2>
            <button onClick={onClose} className="text-muted-foreground hover:text-foreground/90">
              <X className="w-5 h-5" />
            </button>
          </div>
          
          <nav className="space-y-1">
            <button
              onClick={() => setActiveTab('general')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
                activeTab === 'general' 
                  ? 'bg-indigo-600/20 text-indigo-400' 
                  : 'text-muted-foreground hover:text-foreground/90 hover:bg-accent'
              }`}
            >
              <Edit className="w-4 h-4" />
              通用设置
            </button>
            <button
              onClick={() => setActiveTab('members')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
                activeTab === 'members' 
                  ? 'bg-indigo-600/20 text-indigo-400' 
                  : 'text-muted-foreground hover:text-foreground/90 hover:bg-accent'
              }`}
            >
              <FolderOpen className="w-4 h-4" />
              成员管理
            </button>
            <button
              onClick={() => setActiveTab('danger')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
                activeTab === 'danger' 
                  ? 'bg-red-600/20 text-red-400' 
                  : 'text-muted-foreground hover:text-foreground/90 hover:bg-accent'
              }`}
            >
              <Trash2 className="w-4 h-4" />
              危险区域
            </button>
          </nav>
        </div>

        {/* Content */}
        <div className="flex-1 p-6 overflow-auto">
          {activeTab === 'general' && (
            <div className="space-y-6">
              <div>
                <h3 className="text-lg font-medium text-foreground mb-4">通用设置</h3>
                
                <div className="space-y-4">
                  {/* Icon Picker */}
                  <div className="relative">
                    <label className="block text-sm font-medium text-muted-foreground mb-2">
                      工作区图标
                    </label>
                    <div className="flex items-center gap-3">
                      <button
                        type="button"
                        onClick={() => setShowEmojiPicker(!showEmojiPicker)}
                        className="w-16 h-16 flex items-center justify-center text-3xl bg-accent rounded-lg border border-border hover:border-indigo-500 transition-colors"
                      >
                        {icon}
                      </button>
                      <span className="text-sm text-muted-foreground">点击选择图标</span>
                    </div>
                    
                    {/* Emoji Picker Dropdown */}
                    {showEmojiPicker && (
                      <div 
                        ref={emojiPickerRef}
                        className="absolute z-10 mt-2 p-3 bg-card border border-border rounded-lg shadow-lg w-72 max-h-64 overflow-y-auto"
                      >
                        <div className="grid grid-cols-8 gap-1">
                          {EMOJI_PICKER.map((emoji, i) => (
                            <button
                              key={i}
                              type="button"
                              onClick={() => {
                                setIcon(emoji);
                                setShowEmojiPicker(false);
                              }}
                              className={`w-8 h-8 flex items-center justify-center text-xl rounded hover:bg-accent transition-colors ${
                                icon === emoji ? 'bg-indigo-600/30' : ''
                              }`}
                            >
                              {emoji}
                            </button>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                  
                  <div>
                    <label className="block text-sm font-medium text-muted-foreground mb-2">
                      名称
                    </label>
                    <input
                      type="text"
                      value={title}
                      onChange={(e) => setTitle(e.target.value)}
                      className="w-full h-10 px-3 rounded-lg bg-background border border-border text-foreground focus:outline-none focus:ring-2 focus:ring-indigo-500"
                    />
                  </div>
                  
                  <div>
                    <label className="block text-sm font-medium text-muted-foreground mb-2">
                      描述
                    </label>
                    <textarea
                      value={description}
                      onChange={(e) => setDescription(e.target.value)}
                      rows={3}
                      className="w-full px-3 py-2 rounded-lg bg-background border border-border text-foreground focus:outline-none focus:ring-2 focus:ring-indigo-500 resize-none"
                      placeholder="输入工作区描述..."
                    />
                  </div>
                </div>
              </div>

              <div className="flex justify-end gap-3 pt-4">
                <button
                  onClick={onClose}
                  className="h-10 px-4 rounded-lg border border-border text-foreground/80 hover:bg-accent transition-colors"
                >
                  取消
                </button>
                <button
                  onClick={handleSave}
                  disabled={saving || !title.trim()}
                  className="h-10 px-4 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white font-medium transition-colors disabled:opacity-50 flex items-center gap-2"
                >
                  {saving && <Loader2 className="w-4 h-4 animate-spin" />}
                  保存
                </button>
              </div>
            </div>
          )}

          {activeTab === 'members' && (
            <div className="space-y-6">
              <div>
                <h3 className="text-lg font-medium text-foreground mb-4">成员管理</h3>
                
                <div className="text-center py-12 text-muted-foreground">
                  <FolderOpen className="w-12 h-12 mx-auto mb-4 opacity-50" />
                  <p>暂无成员管理功能</p>
                  <p className="text-sm mt-2">该功能即将推出</p>
                </div>
              </div>
            </div>
          )}

          {activeTab === 'danger' && (
            <div className="space-y-6">
              <div>
                <h3 className="text-lg font-medium text-foreground mb-4">危险区域</h3>
                
                <div className="space-y-3">
                  <button
                    onClick={handleExport}
                    className="w-full flex items-center gap-3 p-4 rounded-lg bg-accent hover:bg-accent/80 transition-colors"
                  >
                    <Download className="w-5 h-5 text-indigo-400" />
                    <div className="text-left">
                      <p className="text-foreground font-medium">导出数据</p>
                      <p className="text-sm text-muted-foreground/80">导出工作区和对话记录为 JSON 文件</p>
                    </div>
                  </button>

                  <button
                    onClick={handleClearHistory}
                    className="w-full flex items-center gap-3 p-4 rounded-lg bg-accent hover:bg-accent/80 transition-colors"
                  >
                    <Trash2 className="w-5 h-5 text-yellow-400" />
                    <div className="text-left">
                      <p className="text-foreground font-medium">清空对话历史</p>
                      <p className="text-sm text-muted-foreground/80">删除所有对话记录，此操作不可恢复</p>
                    </div>
                  </button>

                  <div className="p-4 rounded-lg bg-red-900/20 border border-red-800/50">
                    <div className="flex items-center gap-3 mb-3">
                      <Trash2 className="w-5 h-5 text-red-400" />
                      <div>
                        <p className="text-red-400 font-medium">删除工作区</p>
                        <p className="text-sm text-muted-foreground/80">删除整个工作区及其所有数据，此操作不可恢复</p>
                      </div>
                    </div>
                    
                    {!showDeleteConfirm ? (
                      <button
                        onClick={handleDeleteGroup}
                        className="w-full h-10 rounded-lg bg-red-600 hover:bg-red-700 text-white font-medium transition-colors"
                      >
                        删除工作区
                      </button>
                    ) : (
                      <div className="space-y-2">
                        <p className="text-sm text-red-400">
                          确定要删除工作区 “{kb?.title}” 吗？此操作不可恢复！
                        </p>
                        <div className="flex gap-2">
                          <button
                            onClick={() => setShowDeleteConfirm(false)}
                            className="flex-1 h-10 rounded-lg border border-border text-foreground/80 hover:bg-accent transition-colors"
                          >
                            取消
                          </button>
                          <button
                            onClick={handleDeleteGroup}
                            disabled={deleting}
                            className="flex-1 h-10 rounded-lg bg-red-600 hover:bg-red-700 text-white font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
                          >
                            {deleting && <Loader2 className="w-4 h-4 animate-spin" />}
                            确认删除
                          </button>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
