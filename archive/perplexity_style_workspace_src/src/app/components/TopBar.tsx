import React, { useState, useRef, useEffect } from 'react';
import { Plus, BarChart2, Share2, Code, Settings, UserCircle } from 'lucide-react';

interface TopBarProps {
  onNewNotebook?: () => void;
}

export function TopBar({ onNewNotebook, isNewWorkspace = false }: TopBarProps & { isNewWorkspace?: boolean }) {
  const [isEditing, setIsEditing] = useState(false);
  const [projectName, setProjectName] = useState(isNewWorkspace ? 'Untitled Project' : 'Research Project Alpha');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isEditing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [isEditing]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      setIsEditing(false);
    } else if (e.key === 'Escape') {
      setIsEditing(false);
    }
  };

  return (
    <div className="flex items-center justify-between px-4 py-2 border-b border-gray-200 bg-white shadow-sm shrink-0 z-10 h-14">
      {/* Left section */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2 text-gray-900 font-bold text-lg cursor-pointer tracking-tight">
          <svg width="28" height="28" viewBox="0 0 28 28" fill="none" xmlns="http://www.w3.org/2000/svg" className="shrink-0 shadow-sm rounded-md">
            <rect width="28" height="28" rx="6" fill="#111111"/>
            {/* AI Sparkles */}
            <path d="M14 5L13.5 7.5L11 8L13.5 8.5L14 11L14.5 8.5L17 8L14.5 7.5L14 5Z" fill="white"/>
            <path d="M19 11L18.5 12.5L17 13L18.5 13.5L19 15L19.5 13.5L21 13L19.5 12.5L19 11Z" fill="white"/>
            {/* Organic Brain/Node Structure */}
            <path d="M9 18C7.34315 18 6 16.6569 6 15C6 13.5936 6.96752 12.413 8.2785 12.0886C8.65487 10.3195 10.2335 9 12.1429 9C13.7709 9 15.1558 10.0506 15.6382 11.5028C16.0264 11.176 16.5401 11 17.0952 11C18.702 11 20 12.3431 20 14C20 15.6569 18.702 17 17.0952 17C16.652 17 16.2346 16.8931 15.8663 16.706C15.3262 17.4721 14.421 18 13.4286 18H9Z" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            <circle cx="13" cy="14" r="1.5" fill="white"/>
            <circle cx="9.5" cy="15" r="1" fill="white"/>
            <circle cx="16.5" cy="14" r="1" fill="white"/>
            <path d="M13 14L9.5 15" stroke="white" strokeWidth="1" strokeLinecap="round"/>
            <path d="M13 14L16.5 14" stroke="white" strokeWidth="1" strokeLinecap="round"/>
          </svg>
          <span>Context-OS</span>
        </div>
        <div className="h-6 w-[1px] bg-gray-300 mx-2" />
        <div className="flex items-center gap-1 hover:bg-gray-100 px-2 py-1 rounded-md cursor-pointer transition-colors min-w-[100px]">
          {isEditing ? (
            <input
              ref={inputRef}
              type="text"
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
              onBlur={() => setIsEditing(false)}
              onKeyDown={handleKeyDown}
              className="text-gray-800 font-medium bg-transparent border-none focus:outline-none focus:ring-1 focus:ring-gray-300 rounded px-1 w-full"
            />
          ) : (
            <span 
              className="text-gray-800 font-medium"
              onClick={() => setIsEditing(true)}
            >
              {projectName}
            </span>
          )}
        </div>
      </div>

      {/* Right section */}
      <div className="flex items-center gap-1 sm:gap-2">
        <button 
          onClick={onNewNotebook}
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-100 rounded-md transition-colors"
        >
          <Plus className="w-4 h-4" />
          <span className="hidden sm:inline">New Notebook</span>
        </button>
        <button className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-100 rounded-md transition-colors">
          <BarChart2 className="w-4 h-4" />
          <span className="hidden sm:inline">Analyze</span>
        </button>
        <button className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-100 rounded-md transition-colors">
          <Share2 className="w-4 h-4" />
          <span className="hidden sm:inline">Share</span>
        </button>
        <button className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-100 rounded-md transition-colors">
          <Code className="w-4 h-4" />
          <span className="hidden sm:inline">API</span>
        </button>
        <button className="p-1.5 text-gray-600 hover:bg-gray-100 rounded-full transition-colors">
          <Settings className="w-5 h-5" />
        </button>
        <button className="p-1.5 text-gray-600 hover:bg-gray-100 rounded-full transition-colors ml-1">
          <UserCircle className="w-6 h-6" />
        </button>
      </div>
    </div>
  );
}
