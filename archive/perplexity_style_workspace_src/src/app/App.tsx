import React, { useState } from 'react';
import { SidebarLeft } from './components/SidebarLeft';
import { ChatArea } from './components/ChatArea';
import { SidebarRight } from './components/SidebarRight';
import { TopBar } from './components/TopBar';

export interface Thread {
  id: number;
  title: string;
}

export const INITIAL_THREADS: Thread[] = [
  { id: 1, title: 'Generative AI trends 2024' },
  { id: 2, title: 'React Performance Optimization' },
  { id: 3, title: 'Vite build configurations' },
  { id: 4, title: 'Kubernetes vs Docker Swarm' },
  { id: 5, title: 'Figma to Code plugin features' },
  { id: 6, title: 'Tailwind grid system layout' },
];

export default function App() {
  const [workspaceId, setWorkspaceId] = useState(Date.now());
  const [isNewWorkspace, setIsNewWorkspace] = useState(false);
  const [threads, setThreads] = useState<Thread[]>(INITIAL_THREADS);
  const [activeThreadId, setActiveThreadId] = useState<number>(INITIAL_THREADS[0].id);

  const handleNewThread = () => {
    const newThread = {
      id: Date.now(),
      title: 'New Thread',
    };
    setThreads([newThread, ...threads]);
    setActiveThreadId(newThread.id);
  };

  const handleNewNotebook = () => {
    setWorkspaceId(Date.now());
    setIsNewWorkspace(true);
    setThreads([{ id: Date.now(), title: 'New Thread' }]);
    setActiveThreadId(Date.now());
  };

  return (
    <div className="flex flex-col h-screen w-full bg-[#fcfcfc] text-[#111111] overflow-hidden font-sans">
      <TopBar onNewNotebook={handleNewNotebook} isNewWorkspace={isNewWorkspace} key={`topbar-${workspaceId}`} />
      <div className="flex flex-1 overflow-hidden" key={`workspace-${workspaceId}`}>
        <SidebarLeft 
          threads={threads} 
          activeThreadId={activeThreadId} 
          onSelectThread={setActiveThreadId} 
          onNewThread={handleNewThread} 
        />
        <ChatArea activeThreadId={activeThreadId} />
        <SidebarRight isNewWorkspace={isNewWorkspace} />
      </div>
    </div>
  );
}
