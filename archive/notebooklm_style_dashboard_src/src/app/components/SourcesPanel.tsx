import React from 'react';
import { FileText, Globe, Image, Plus, Link2 } from 'lucide-react';

const sources = [
  {
    id: 1,
    title: 'Product Strategy Q3.pdf',
    type: 'pdf',
    domain: '12 pages',
    icon: <FileText size={16} className="text-rose-500" />
  },
  {
    id: 2,
    title: 'User Interview Transcripts',
    type: 'doc',
    domain: 'Google Docs',
    icon: <FileText size={16} className="text-blue-500" />
  },
  {
    id: 3,
    title: 'Perplexity & NotebookLM UX Study',
    type: 'web',
    domain: 'uxdesign.cc',
    icon: <Globe size={16} className="text-emerald-500" />
  },
  {
    id: 4,
    title: 'Competitor Feature Matrix',
    type: 'link',
    domain: 'notion.so',
    icon: <Link2 size={16} className="text-zinc-600" />
  }
];

export const SourcesPanel: React.FC = () => {
  return (
    <div className="flex items-center gap-3 overflow-x-auto pb-4 -mx-4 px-4 scrollbar-hide w-full" style={{ scrollbarWidth: 'none' }}>
      {sources.map((source) => (
        <button
          key={source.id}
          className="flex-shrink-0 flex items-center gap-3 w-48 p-3 bg-[#F9F9F9] border border-zinc-200 rounded-xl hover:bg-zinc-100 hover:border-zinc-300 transition-all text-left group shadow-sm hover:shadow-md"
        >
          <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-white shadow-sm border border-zinc-100 shrink-0">
            {source.icon}
          </div>
          <div className="flex flex-col overflow-hidden w-full">
            <span className="text-[13px] font-medium text-zinc-900 truncate group-hover:text-blue-600 transition-colors">
              {source.title}
            </span>
            <span className="text-[11px] text-zinc-500 truncate mt-0.5">
              {source.domain}
            </span>
          </div>
        </button>
      ))}

      {/* Add Source Button */}
      <button className="flex-shrink-0 flex items-center gap-3 w-32 p-3 border border-dashed border-zinc-300 rounded-xl hover:bg-zinc-50 hover:border-zinc-400 transition-all text-left justify-center flex-col shadow-sm">
        <div className="flex items-center justify-center w-8 h-8 rounded-full bg-zinc-100 text-zinc-500">
          <Plus size={16} />
        </div>
        <span className="text-xs font-medium text-zinc-500">Add Source</span>
      </button>
    </div>
  );
};
