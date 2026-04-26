import React, { useState } from 'react';
import { Header } from './Header';
import { NotebookCard } from './NotebookCard';
import { NotebookListRow } from './NotebookListRow';
import { Plus, Search, LayoutGrid, AlignJustify, ChevronDown } from 'lucide-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';

export const Dashboard: React.FC = () => {
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('list');
  const [sortBy, setSortBy] = useState<'recent' | 'title'>('recent');
  const [activeTab, setActiveTab] = useState<'all' | 'my'>('my');

  const notebooks = [
    { id: 1, title: '中核集团市场开发现状分析与战略调研模板', date: '2026年3月30日', rawDate: '2026-03-30', sources: 71, role: 'Owner', iconEmoji: '📋' },
    { id: 2, title: 'The Expert Interview Guide: Insight-Driven Research and Best Practices', date: '2026年3月20日', rawDate: '2026-03-20', sources: 20, role: 'Owner', iconEmoji: '🎙️' },
    { id: 3, title: 'CNNP Electricity Sales and Value-Added Services Management Standards', date: '2026年3月13日', rawDate: '2026-03-13', sources: 25, role: 'Owner', iconEmoji: '⚡' },
    { id: 4, title: 'China 2030 Power Market Outlook: Demand, Structure, and Costs', date: '2026年3月17日', rawDate: '2026-03-17', sources: 1, role: 'Owner', iconEmoji: '⚡' },
    { id: 5, title: 'CNNC Power Trading Operational Framework and Execution Flow', date: '2026年3月16日', rawDate: '2026-03-16', sources: 29, role: 'Owner', iconEmoji: '🔄' },
    { id: 6, title: 'The Dual Nature of Power: Commodity and System Logistics', date: '2026年3月13日', rawDate: '2026-03-13', sources: 3, role: 'Owner', iconEmoji: '⚡' },
    { id: 7, title: 'China National Nuclear Power Market Development Strategy 2026-2030', date: '2026年3月13日', rawDate: '2026-03-13', sources: 2, role: 'Owner', iconEmoji: '⚡' },
    { id: 8, title: "China's Unified Power Market and Energy Storage Evolution", date: '2026年3月9日', rawDate: '2026-03-09', sources: 46, role: 'Owner', iconEmoji: '⚡' },
    { id: 9, title: 'Description Logic and the Architecture of Self-Attention Networks', date: '2026年2月8日', rawDate: '2026-02-08', sources: 7, role: 'Owner', iconEmoji: '🤖' },
    { id: 10, title: 'Prospectus Analysis Framework: From Business Strategy to Valuation', date: '2026年1月25日', rawDate: '2026-01-25', sources: 13, role: 'Owner', iconEmoji: '🔍' },
    { id: 11, title: 'Strategic Framework for Prospectus and S-1 Analysis', date: '2026年3月10日', rawDate: '2026-03-10', sources: 3, role: 'Owner', iconEmoji: '📈' },
    { id: 12, title: 'Beyond Naive RAG: Hybrid Search and Collaborative AI Tutoring', date: '2026年3月6日', rawDate: '2026-03-06', sources: 21, role: 'Owner', iconEmoji: '📚' },
  ];

  const sortedNotebooks = [...notebooks].sort((a, b) => {
    if (sortBy === 'recent') {
      return new Date(b.rawDate).getTime() - new Date(a.rawDate).getTime();
    } else {
      return a.title.localeCompare(b.title, 'zh-CN');
    }
  });

  return (
    <div className="flex flex-col h-full min-h-screen bg-white font-sans selection:bg-zinc-100">
      <Header />

      <main className="flex-1 max-w-[1280px] w-full mx-auto px-8 py-8">
        {/* Navigation / Filters Bar */}
        <div className="flex items-center justify-between mb-10">
          {/* Tabs */}
          <div className="flex items-center gap-2">
            <button
              onClick={() => setActiveTab('all')}
              className={`px-5 py-2 text-[14px] font-medium rounded-full transition-colors ${activeTab === 'all' ? 'text-zinc-900 bg-zinc-100/80' : 'text-zinc-600 hover:bg-zinc-50'}`}
            >
              全部
            </button>
            <button
              onClick={() => setActiveTab('my')}
              className={`px-5 py-2 text-[14px] font-medium rounded-full transition-colors ${activeTab === 'my' ? 'text-zinc-900 bg-zinc-100/80' : 'text-zinc-600 hover:bg-zinc-50'}`}
            >
              我的笔记本
            </button>
          </div>

          {/* Right Controls */}
          <div className="flex items-center gap-3">
            {/* Search */}
            <button className="w-[38px] h-[38px] flex items-center justify-center rounded-full border border-zinc-200 text-zinc-500 hover:bg-zinc-50 transition-colors bg-white hover:text-zinc-800 shadow-sm">
              <Search size={18} strokeWidth={2.5} />
            </button>

            {/* View Toggle */}
            <div className="flex items-center bg-zinc-100/80 rounded-full p-1 border border-zinc-200/50 shadow-sm">
              <button
                onClick={() => setViewMode('grid')}
                className={`px-3 py-1.5 rounded-full flex items-center justify-center transition-colors ${viewMode === 'grid' ? 'bg-white shadow-sm text-zinc-900 font-medium' : 'text-zinc-400 hover:text-zinc-700'}`}
              >
                <LayoutGrid size={15} strokeWidth={2.5} />
              </button>
              <button
                onClick={() => setViewMode('list')}
                className={`px-3 py-1.5 rounded-full flex items-center justify-center transition-colors ${viewMode === 'list' ? 'bg-white shadow-sm text-zinc-900 font-medium' : 'text-zinc-400 hover:text-zinc-700'}`}
              >
                <AlignJustify size={16} strokeWidth={2.5} />
              </button>
            </div>

            {/* Sort Dropdown */}
            <DropdownMenu.Root>
              <DropdownMenu.Trigger asChild>
                <button className="flex items-center gap-2 px-5 py-2.5 bg-white border border-zinc-200 rounded-full text-[14px] font-medium text-zinc-700 hover:bg-zinc-50 transition-colors outline-none shadow-sm">
                  {sortBy === 'recent' ? '最近' : '标题'}
                  <ChevronDown size={14} strokeWidth={2.5} className="text-zinc-400" />
                </button>
              </DropdownMenu.Trigger>
              <DropdownMenu.Portal>
                <DropdownMenu.Content align="end" className="min-w-[120px] bg-white rounded-xl shadow-lg border border-zinc-100 p-1.5 z-50 animate-in fade-in zoom-in-95 duration-100">
                  <DropdownMenu.Item onClick={() => setSortBy('recent')} className="text-[14px] px-3 py-2 outline-none cursor-pointer hover:bg-zinc-50 rounded-lg text-zinc-800 font-medium transition-colors">
                    最近
                  </DropdownMenu.Item>
                  <DropdownMenu.Item onClick={() => setSortBy('title')} className="text-[14px] px-3 py-2 outline-none cursor-pointer hover:bg-zinc-50 rounded-lg text-zinc-800 font-medium transition-colors">
                    标题
                  </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Portal>
            </DropdownMenu.Root>

            {/* New Button */}
            <button className="flex items-center gap-2 px-6 py-2.5 bg-black text-white rounded-full text-[14px] font-medium hover:bg-zinc-800 transition-colors shadow-sm ml-1">
              <Plus size={16} strokeWidth={2.5} />
              新建
            </button>
          </div>
        </div>

        {/* Title */}
        <h1 className="text-2xl font-medium text-zinc-900 mb-8 px-1">我的笔记本</h1>

        {/* Conditional View Rendering */}
        {viewMode === 'grid' ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-5 pb-12">
            {/* New Notebook Card */}
            <div className="h-[180px] bg-zinc-50/50 border border-dashed border-zinc-300 rounded-2xl flex flex-col items-center justify-center cursor-pointer hover:bg-zinc-100 hover:border-zinc-400 transition-all group">
              <div className="w-11 h-11 bg-white shadow-sm text-zinc-600 rounded-full flex items-center justify-center mb-3 group-hover:scale-105 transition-transform border border-zinc-100">
                <Plus size={22} strokeWidth={2.5} />
              </div>
              <span className="text-zinc-600 font-medium text-[14.5px]">新建笔记本</span>
            </div>

            {sortedNotebooks.map((notebook) => (
              <NotebookCard key={notebook.id} notebook={notebook} />
            ))}
          </div>
        ) : (
          <div className="flex flex-col pb-12 w-full">
            {/* Table Header */}
            <div className="grid grid-cols-12 gap-4 pb-3 border-b border-zinc-200 text-[13.5px] font-medium text-zinc-500 px-4">
              <div className="col-span-6">标题</div>
              <div className="col-span-2">来源</div>
              <div className="col-span-2">创建日期</div>
              <div className="col-span-2 pr-6">角色</div>
            </div>

            {/* Table Body */}
            <div className="flex flex-col">
              {sortedNotebooks.map((notebook) => (
                <NotebookListRow key={notebook.id} notebook={notebook} />
              ))}
            </div>
          </div>
        )}
      </main>
    </div>
  );
};
