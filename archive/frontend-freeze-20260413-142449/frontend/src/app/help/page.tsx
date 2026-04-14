'use client';

import { useState } from 'react';
import { BookOpen, Zap, FileText, MessageSquare, StickyNote, Search as SearchIcon, Settings, ChevronRight, Moon, Sun, Monitor } from 'lucide-react';
import { useTheme } from 'next-themes';

const navItems = [
  { id: 'getting-started', label: '快速开始', icon: Zap },
  { id: 'workspace', label: '工作区管理', icon: BookOpen },
  { id: 'document', label: '文档上传', icon: FileText },
  { id: 'chat', label: 'AI 对话', icon: MessageSquare },
  { id: 'note', label: '笔记管理', icon: StickyNote },
  { id: 'search', label: '全局搜索', icon: SearchIcon },
  { id: 'settings', label: '设置', icon: Settings },
];

export default function HelpPage() {
  const [activeSection, setActiveSection] = useState('getting-started');
  const { theme, setTheme, resolvedTheme } = useTheme();

  const renderContent = () => {
    switch (activeSection) {
      case 'getting-started':
        return <GettingStartedContent />;
      case 'workspace':
        return <WorkspaceContent />;
      case 'document':
        return <DocumentContent />;
      case 'chat':
        return <ChatContent />;
      case 'note':
        return <NoteContent />;
      case 'search':
        return <SearchContent />;
      case 'settings':
        return <SettingsContent />;
      default:
        return <GettingStartedContent />;
    }
  };

  return (
    <div className="min-h-screen bg-background text-foreground">
      {/* Header */}
      <header className="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="max-w-7xl mx-auto px-4 h-14 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <BookOpen className="w-5 h-5 text-primary" />
            <span className="font-semibold">帮助文档</span>
          </div>
          <div className="flex items-center gap-4">
            <div className="flex gap-1">
              <button
                onClick={() => setTheme('dark')}
                className={`p-2 rounded-lg transition-colors ${resolvedTheme === 'dark' ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:text-foreground'}`}
              >
                <Moon className="w-4 h-4" />
              </button>
              <button
                onClick={() => setTheme('light')}
                className={`p-2 rounded-lg transition-colors ${resolvedTheme === 'light' ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:text-foreground'}`}
              >
                <Sun className="w-4 h-4" />
              </button>
              <button
                onClick={() => setTheme('system')}
                className={`p-2 rounded-lg transition-colors ${theme === 'system' ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:text-foreground'}`}
              >
                <Monitor className="w-4 h-4" />
              </button>
            </div>
          </div>
        </div>
      </header>

      <div className="max-w-7xl mx-auto flex">
        {/* Sidebar */}
        <aside className="w-64 shrink-0 border-r border-border py-6 hidden md:block sticky top-14 h-[calc(100vh-3.5rem)] overflow-y-auto">
          <nav className="space-y-1 px-3">
            {navItems.map((item) => (
              <button
                key={item.id}
                onClick={() => setActiveSection(item.id)}
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left transition-colors ${
                  activeSection === item.id
                    ? 'bg-primary/10 text-primary font-medium'
                    : 'text-muted-foreground hover:text-foreground hover:bg-accent'
                }`}
              >
                <item.icon className="w-4 h-4" />
                {item.label}
                {activeSection === item.id && <ChevronRight className="w-4 h-4 ml-auto" />}
              </button>
            ))}
          </nav>
        </aside>

        {/* Mobile Nav */}
        <div className="md:hidden fixed bottom-0 left-0 right-0 border-t border-border bg-background z-40">
          <nav className="flex overflow-x-auto py-2 px-2 gap-1">
            {navItems.map((item) => (
              <button
                key={item.id}
                onClick={() => setActiveSection(item.id)}
                className={`flex-shrink-0 flex flex-col items-center gap-1 px-3 py-2 rounded-lg text-xs ${
                  activeSection === item.id
                    ? 'text-primary'
                    : 'text-muted-foreground'
                }`}
              >
                <item.icon className="w-4 h-4" />
                {item.label}
              </button>
            ))}
          </nav>
        </div>

        {/* Main Content */}
        <main className="flex-1 py-8 px-4 md:px-8 pb-24 md:pb-8">
          <div className="max-w-3xl mx-auto">
            {renderContent()}
          </div>
        </main>
      </div>
    </div>
  );
}

// Quick Start Content
function GettingStartedContent() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold mb-4">快速开始</h1>
        <p className="text-muted-foreground mb-6">欢迎使用 Context OS！本指南将帮助你快速上手核心功能。</p>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold flex items-center gap-2">
          <span className="w-8 h-8 rounded-lg bg-primary/10 text-primary flex items-center justify-center text-sm">1</span>
          创建工作区
        </h2>
        <div className="ml-10 space-y-2 text-muted-foreground">
          <p>工作区是组织和管理知识库的核心单位。每个工作区可以包含多个文档、笔记和对话记录。</p>
          <ul className="list-disc list-inside space-y-1">
            <li>在左侧边栏点击 <span className="text-foreground font-medium">+</span> 按钮创建新工作区</li>
            <li>输入工作区名称和描述</li>
            <li>创建完成后会自动进入该工作区</li>
          </ul>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold flex items-center gap-2">
          <span className="w-8 h-8 rounded-lg bg-primary/10 text-primary flex items-center justify-center text-sm">2</span>
          上传文档
        </h2>
        <div className="ml-10 space-y-2 text-muted-foreground">
          <p>支持多种文档格式上传，系统会自动进行向量化处理以支持 AI 问答。</p>
          <ul className="list-disc list-inside space-y-1">
            <li>进入工作区后，点击 <span className="text-foreground font-medium">文档</span> 标签页</li>
            <li>点击上传按钮或拖拽文件到上传区域</li>
            <li>支持格式：PDF、DOC、DOCX、PPT、PPTX、HTML、MD、TXT、XLSX</li>
            <li>上传后系统会自动处理，状态会显示为「处理中」→ 「已完成」</li>
          </ul>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold flex items-center gap-2">
          <span className="w-8 h-8 rounded-lg bg-primary/10 text-primary flex items-center justify-center text-sm">3</span>
          开始 AI 对话
        </h2>
        <div className="ml-10 space-y-2 text-muted-foreground">
          <p>基于上传的文档内容，你可以与 AI 进行智能对话。</p>
          <ul className="list-disc list-inside space-y-1">
            <li>点击 <span className="text-foreground font-medium">对话</span> 标签页进入聊天界面</li>
            <li>在输入框中输入问题，按 Enter 发送</li>
            <li>AI 会根据文档内容给出回答，并显示引用来源</li>
            <li>可以点击引用标记查看原文</li>
          </ul>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold flex items-center gap-2">
          <span className="w-8 h-8 rounded-lg bg-primary/10 text-primary flex items-center justify-center text-sm">4</span>
          使用全局搜索
        </h2>
        <div className="ml-10 space-y-2 text-muted-foreground">
          <p>按 <kbd className="px-1.5 py-0.5 bg-muted rounded text-xs">Ctrl</kbd> + <kbd className="px-1.5 py-0.5 bg-muted rounded text-xs">K</kbd> 打开全局搜索，快速找到工作区、文档、笔记和对话。</p>
        </div>
      </section>
    </div>
  );
}

// Workspace Content
function WorkspaceContent() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold mb-4">工作区管理</h1>
        <p className="text-muted-foreground mb-6">工作区是 Context OS 的核心概念，用于组织和管理你的知识库。</p>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">什么是工作区？</h2>
        <div className="text-muted-foreground space-y-2">
          <p>工作区相当于一个独立的知识库，可以包含：</p>
          <ul className="list-disc list-inside space-y-1 ml-4">
            <li>多个文档文件</li>
            <li>AI 对话记录</li>
            <li>个人笔记</li>
            <li>引用来源</li>
          </ul>
          <p className="mt-4">不同工作区之间的数据是相互隔离的，非常适合管理不同的项目或团队。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">创建工作区</h2>
        <div className="text-muted-foreground space-y-2">
          <ol className="list-decimal list-inside space-y-2 ml-4">
            <li>在仪表盘页面，点击「新建工作区」按钮</li>
            <li>输入工作区名称（必填）</li>
            <li>输入工作区描述（可选）</li>
            <li>点击「确定」完成创建</li>
          </ol>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">切换工作区</h2>
        <div className="text-muted-foreground">
          <p>点击左侧边栏的工作区名称，在下拉列表中选择目标工作区即可切换。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">编辑工作区</h2>
        <div className="text-muted-foreground">
          <p>在工作区卡片上点击「···」按钮，选择「编辑」可以修改工作区名称和描述。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">删除工作区</h2>
        <div className="text-muted-foreground">
          <p>在工作区卡片上点击「···」按钮，选择「删除」。删除后该工作区的所有数据将被永久清除，请谨慎操作。</p>
        </div>
      </section>
    </div>
  );
}

// Document Content
function DocumentContent() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold mb-4">文档上传</h1>
        <p className="text-muted-foreground mb-6">文档是 AI 对话的知识来源。上传文档后，系统会自动处理并建立向量索引。</p>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">支持的文件格式</h2>
        <div className="grid grid-cols-2 md:grid-cols-3 gap-2 text-sm">
          {['PDF', 'DOC', 'DOCX', 'PPT', 'PPTX', 'HTML', 'MD', 'TXT', 'XLSX'].map((ext) => (
            <div key={ext} className="px-3 py-2 bg-muted/50 rounded-lg text-center">{ext}</div>
          ))}
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">上传步骤</h2>
        <div className="text-muted-foreground space-y-2">
          <ol className="list-decimal list-inside space-y-2 ml-4">
            <li>进入工作区后，点击「文档」标签页</li>
            <li>点击上传按钮或直接将文件拖拽到上传区域</li>
            <li>等待文件上传完成</li>
            <li>文件会自动进入「处理中」状态</li>
            <li>处理完成后状态变为「已完成」即可用于 AI 对话</li>
          </ol>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">文档状态说明</h2>
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <span className="px-2 py-1 rounded-full text-xs bg-amber-500/20 text-amber-400">处理中</span>
            <span className="text-sm text-muted-foreground">文档正在向量化处理中，暂不可用于问答</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="px-2 py-1 rounded-full text-xs bg-green-500/20 text-green-400">已完成</span>
            <span className="text-sm text-muted-foreground">文档处理完成，可以用于 AI 对话</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="px-2 py-1 rounded-full text-xs bg-red-500/20 text-red-400">失败</span>
            <span className="text-sm text-muted-foreground">文档处理失败，请重新上传</span>
          </div>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">删除文档</h2>
        <div className="text-muted-foreground">
          <p>在文档列表中，点击文档右侧的删除按钮即可删除文档。删除后该文档将不再用于 AI 对话。</p>
        </div>
      </section>
    </div>
  );
}

// Chat Content
function ChatContent() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold mb-4">AI 对话</h1>
        <p className="text-muted-foreground mb-6">基于 RAG 技术，让 AI 根据你的文档内容回答问题。</p>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">选择 AI 助手</h2>
        <div className="text-muted-foreground space-y-2">
          <p>系统提供多个 AI 助手：</p>
          <ul className="list-disc list-inside space-y-1 ml-4">
            <li><strong className="text-foreground">知识库助手</strong> - 基于工作区文档进行问答</li>
            <li><strong className="text-foreground">通用助手</strong> - 自由对话，不限定知识库</li>
            <li><strong className="text-foreground">搜索助手</strong> - 专注于信息检索</li>
          </ul>
          <p className="mt-2">点击输入框左侧的助手图标或输入 <code className="px-1.5 py-0.5 bg-muted rounded text-sm">@助手名</code> 切换。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">发送消息</h2>
        <div className="text-muted-foreground">
          <p>在输入框中输入问题，按 Enter 或点击发送按钮。AI 会根据当前工作区的文档内容生成回答。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">查看引用来源</h2>
        <div className="text-muted-foreground space-y-2">
          <p>AI 回答中会包含引用标记，格式为 <code className="px-1.5 py-0.5 bg-muted rounded text-sm">[[1]]</code>。点击引用标记可以查看原文内容。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">从对话提取笔记</h2>
        <div className="text-muted-foreground">
          <p>对于 AI 的回答，可以点击「提取」按钮将其保存为笔记，方便后续查阅。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">复制回答</h2>
        <div className="text-muted-foreground">
          <p>点击 AI 回答下方的「复制」按钮可以将回答内容复制到剪贴板。</p>
        </div>
      </section>
    </div>
  );
}

// Note Content
function NoteContent() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold mb-4">笔记管理</h1>
        <p className="text-muted-foreground mb-6">笔记功能让你可以创建、编辑和管理个人笔记。</p>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">创建笔记</h2>
        <div className="text-muted-foreground">
          <p>点击「笔记」标签页，点击「新建笔记」按钮即可创建新笔记。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">编辑笔记</h2>
        <div className="text-muted-foreground space-y-2">
          <ul className="list-disc list-inside space-y-1 ml-4">
            <li>点击笔记标题进入编辑页面</li>
            <li>支持 Markdown 语法</li>
            <li>编辑内容会自动保存</li>
            <li>点击预览按钮查看渲染效果</li>
          </ul>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">自动保存</h2>
        <div className="text-muted-foreground">
          <p>笔记编辑时会自动保存，你可以在底部状态栏看到保存状态。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">归档笔记</h2>
        <div className="text-muted-foreground">
          <p>不再需要的笔记可以选择归档，归档后的笔记不会显示在列表中但仍保留数据。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">删除笔记</h2>
        <div className="text-muted-foreground">
          <p>在笔记列表中，点击笔记右侧的删除按钮即可删除笔记。删除后无法恢复，请谨慎操作。</p>
        </div>
      </section>
    </div>
  );
}

// Search Content
function SearchContent() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold mb-4">全局搜索</h1>
        <p className="text-muted-foreground mb-6">全局搜索帮助你快速找到工作区、文档、笔记和对话记录。</p>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">打开搜索</h2>
        <div className="text-muted-foreground space-y-2">
          <p>使用快捷键快速打开搜索：</p>
          <ul className="list-disc list-inside space-y-1 ml-4">
            <li>Windows/Linux: <kbd className="px-1.5 py-0.5 bg-muted rounded text-sm">Ctrl</kbd> + <kbd className="px-1.5 py-0.5 bg-muted rounded text-sm">K</kbd></li>
            <li>Mac: <kbd className="px-1.5 py-0.5 bg-muted rounded text-sm">Cmd</kbd> + <kbd className="px-1.5 py-0.5 bg-muted rounded text-sm">K</kbd></li>
          </ul>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">搜索范围</h2>
        <div className="text-muted-foreground">
          <p>全局搜索支持跨工作区搜索，可以找到：</p>
          <ul className="list-disc list-inside space-y-1 ml-4 mt-2">
            <li>工作区</li>
            <li>文档</li>
            <li>笔记</li>
            <li>对话记录</li>
          </ul>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">键盘操作</h2>
        <div className="text-muted-foreground space-y-2">
          <ul className="list-disc list-inside space-y-1 ml-4">
            <li><kbd className="px-1.5 py-0.5 bg-muted rounded text-xs">↑</kbd> <kbd className="px-1.5 py-0.5 text-xs">↓ bg-muted rounded</kbd> - 导航结果</li>
            <li><kbd className="px-1.5 py-0.5 bg-muted rounded text-xs">Enter</kbd> - 跳转到选中结果</li>
            <li><kbd className="px-1.5 py-0.5 bg-muted rounded text-xs">Esc</kbd> - 关闭搜索</li>
          </ul>
        </div>
      </section>
    </div>
  );
}

// Settings Content
function SettingsContent() {
  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-bold mb-4">设置</h1>
        <p className="text-muted-foreground mb-6">管理你的账户偏好和主题设置。</p>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">外观设置</h2>
        <div className="text-muted-foreground space-y-2">
          <p>支持三种主题模式：</p>
          <ul className="list-disc list-inside space-y-1 ml-4">
            <li><strong className="text-foreground">深色</strong> - 适合夜间使用</li>
            <li><strong className="text-foreground">浅色</strong> - 适合白天使用</li>
            <li><strong className="text-foreground">跟随系统</strong> - 自动跟随设备设置</li>
          </ul>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">个人资料</h2>
        <div className="text-muted-foreground">
          <p>在设置面板中可以修改你的昵称。邮箱地址不可修改。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">修改密码</h2>
        <div className="text-muted-foreground">
          <p>在设置面板中可以修改登录密码。新密码至少需要 6 位字符。</p>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold">退出登录</h2>
        <div className="text-muted-foreground">
          <p>点击设置面板底部的「退出登录」按钮即可退出当前账户。</p>
        </div>
      </section>
    </div>
  );
}
