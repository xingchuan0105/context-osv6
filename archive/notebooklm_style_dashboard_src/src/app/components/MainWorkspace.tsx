import React from 'react';
import { SourcesPanel } from './SourcesPanel';
import { AudioOverviewCard } from './AudioOverviewCard';
import { ChatBox } from './ChatBox';
import { Share, MoreHorizontal } from 'lucide-react';

interface MainWorkspaceProps {
  activeNotebook: string;
}

export const MainWorkspace: React.FC<MainWorkspaceProps> = ({ activeNotebook }) => {
  return (
    <div className="flex-1 flex flex-col h-full bg-white relative">
      {/* Header */}
      <div className="flex items-center justify-between px-8 py-5 border-b border-zinc-100 bg-white/80 backdrop-blur-md z-10 sticky top-0">
        <h1 className="text-xl font-semibold tracking-tight text-zinc-900">{activeNotebook}</h1>
        <div className="flex items-center gap-3">
          <button className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-zinc-600 bg-zinc-50 border border-zinc-200 rounded-lg hover:bg-zinc-100 transition-colors shadow-sm">
            <Share size={15} />
            Share
          </button>
          <button className="p-1.5 text-zinc-400 hover:text-zinc-600 hover:bg-zinc-50 rounded-md transition-colors">
            <MoreHorizontal size={20} />
          </button>
        </div>
      </div>

      {/* Main Content Area (Scrollable) */}
      <div className="flex-1 overflow-y-auto w-full flex justify-center pb-32">
        <div className="max-w-4xl w-full px-8 py-8 flex flex-col gap-10">

          {/* Sources Section (Perplexity Style Cards) */}
          <section className="animate-in fade-in slide-in-from-bottom-4 duration-500">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-sm font-semibold tracking-wider text-zinc-500 uppercase">Sources</h2>
              <button className="text-xs font-medium text-blue-600 hover:text-blue-700">View all</button>
            </div>
            <SourcesPanel />
          </section>

          {/* Audio Overview (NotebookLM Signature) */}
          <section className="animate-in fade-in slide-in-from-bottom-5 duration-500 delay-150 fill-mode-both">
            <h2 className="text-sm font-semibold tracking-wider text-zinc-500 uppercase mb-4">Audio Overview</h2>
            <AudioOverviewCard />
          </section>

          {/* Studio Notes / Study Guide */}
          <section className="animate-in fade-in slide-in-from-bottom-6 duration-700 delay-300 fill-mode-both">
            <h2 className="text-sm font-semibold tracking-wider text-zinc-500 uppercase mb-4">Study Guide</h2>
            <div className="prose prose-zinc max-w-none text-[15px] leading-relaxed">
              <p>
                Based on your uploaded sources, here is a comprehensive overview of <strong>{activeNotebook}</strong>.
              </p>
              <h3 className="text-lg font-semibold mt-6 mb-2 text-zinc-800">Key Themes</h3>
              <ul className="space-y-2 text-zinc-700 list-disc pl-5">
                <li>Integration of AI into daily workflows without disrupting existing processes.</li>
                <li>Emphasis on security and data privacy when processing user data through LLMs.</li>
                <li>Importance of a clean, distraction-free interface (similar to Perplexity and NotebookLM).</li>
              </ul>

              <h3 className="text-lg font-semibold mt-6 mb-2 text-zinc-800">Action Items</h3>
              <ul className="space-y-2 text-zinc-700 list-disc pl-5">
                <li>Review the competitive analysis document for Q3.</li>
                <li>Draft the updated UI specifications based on user feedback.</li>
                <li>Finalize the branding guidelines for the "NotebookAI" product launch.</li>
              </ul>
            </div>
          </section>
        </div>
      </div>

      {/* Fixed Bottom Chat Box (Perplexity Style) */}
      <ChatBox />
    </div>
  );
};
