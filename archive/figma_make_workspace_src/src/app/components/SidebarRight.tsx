import React, { useState, useRef, useEffect } from 'react';
import { Plus, CheckSquare, MoreVertical, Pin, Trash2, Square, Check, ArrowRightLeft, X, Save, FileText, UploadCloud, Link as LinkIcon, Copy as CopyIcon, File, Undo, Redo, Bold, Italic, List, ListOrdered, Heading1, Heading2, Type } from 'lucide-react';
import { RichTextEditor } from './RichTextEditor';

const INITIAL_SOURCES = [
  { id: 1, title: 'Q3_Financial_Report.pdf', selected: true },
  { id: 2, title: 'Project_Scope_v2.docx', selected: true },
  { id: 3, title: 'Competitor Analysis - Wikipedia', selected: false },
  { id: 4, title: 'User_Research_Interviews.pdf', selected: true },
];

const INITIAL_NOTES = [
  { id: 1, title: 'Summary of Q3 Goals', content: 'The main goal for Q3 is to increase user retention by 15% through targeted onboarding improvements and a new referral program.', preview: 'The main goal for Q3 is to increase user retention by 15% through...' },
  { id: 2, title: 'Key Risks Identified', content: 'Technical debt in the legacy payment system poses a significant risk to our planned rollout in November. We need to allocate 20% of engineering bandwidth to refactoring.', preview: 'Technical debt in the legacy payment system poses a significant risk to...' },
];

interface Note {
  id: number;
  title: string;
  content: string;
  preview: string;
}

export function SidebarRight({ isNewWorkspace = false }: { isNewWorkspace?: boolean }) {
  const [sources, setSources] = useState(isNewWorkspace ? [] : INITIAL_SOURCES);
  const [notes, setNotes] = useState<Note[]>(isNewWorkspace ? [] : INITIAL_NOTES);
  const [activeMenuSource, setActiveMenuSource] = useState<number | null>(null);
  const [activeMenuNote, setActiveMenuNote] = useState<number | null>(null);

  // Editor State
  const [isEditingNote, setIsEditingNote] = useState(false);
  const [currentNote, setCurrentNote] = useState<Partial<Note>>({ title: '', content: '' });

  // Source Modal State
  const [showSourceModal, setShowSourceModal] = useState(false);
  const [sourceTab, setSourceTab] = useState<'file' | 'link' | 'text'>('file');
  const [isDragging, setIsDragging] = useState(false);

  const toggleSource = (id: number) => {
    setSources(sources.map(s => s.id === id ? { ...s, selected: !s.selected } : s));
  };

  const allSelected = sources.every(s => s.selected);
  const toggleSelectAll = () => {
    setSources(sources.map(s => ({ ...s, selected: !allSelected })));
  };

  // Note Actions
  const handleNewNote = () => {
    setCurrentNote({ title: '', content: '' });
    setIsEditingNote(true);
  };

  const handleEditNote = (note: Note) => {
    setCurrentNote(note);
    setIsEditingNote(true);
  };

  const handleSaveNote = () => {
    if (!currentNote.title?.trim() && !currentNote.content?.trim()) {
      setIsEditingNote(false);
      return;
    }
    
    // Strip HTML for preview
    const rawText = currentNote.content?.replace(/<[^>]*>?/gm, ' ') || '';
    const cleanPreview = rawText.replace(/\s+/g, ' ').trim().substring(0, 80) + '...';

    if (currentNote.id) {
      setNotes(notes.map(n => n.id === currentNote.id ? {
        ...n,
        title: currentNote.title || 'Untitled Note',
        content: currentNote.content || '',
        preview: cleanPreview
      } : n));
    } else {
      const newNote: Note = {
        id: Date.now(),
        title: currentNote.title || 'Untitled Note',
        content: currentNote.content || '',
        preview: cleanPreview
      };
      setNotes([newNote, ...notes]);
    }
    setIsEditingNote(false);
  };

  const handleDeleteNote = (id?: number) => {
    if (id) {
      setNotes(notes.filter(n => n.id !== id));
    }
    setIsEditingNote(false);
  };

  const handleConvertToSource = () => {
    if (!currentNote.title?.trim() && !currentNote.content?.trim()) return;
    
    const newSource = {
      id: Date.now(),
      title: (currentNote.title || 'Untitled Note') + '.txt',
      selected: true,
    };
    setSources([newSource, ...sources]);
    
    // Optionally delete the note after converting
    if (currentNote.id) {
      handleDeleteNote(currentNote.id);
    } else {
      setIsEditingNote(false);
    }
  };

  // Drag & Drop Handlers
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };
  
  const handleDragLeave = () => {
    setIsDragging(false);
  };
  
  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const files = Array.from(e.dataTransfer.files);
      const newSources = files.map((f, i) => ({
        id: Date.now() + i,
        title: f.name,
        selected: true
      }));
      setSources([...newSources, ...sources]);
      setShowSourceModal(false);
    }
  };

  const handleManualUpload = () => {
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.multiple = true;
    fileInput.onchange = (e: any) => {
      if (e.target.files && e.target.files.length > 0) {
        const files = Array.from(e.target.files as FileList);
        const newSources = files.map((f: File, i: number) => ({
          id: Date.now() + i,
          title: f.name,
          selected: true
        }));
        setSources([...newSources, ...sources]);
        setShowSourceModal(false);
      }
    };
    fileInput.click();
  };

  if (isEditingNote) {
    return (
      <div className="w-[340px] border-l border-gray-200 bg-white flex flex-col h-full shrink-0 shadow-[-4px_0_12px_rgba(0,0,0,0.02)]">
        <div className="px-4 py-3 border-b border-gray-200 flex items-center justify-between bg-[#fcfcfc]">
          <h3 className="font-semibold text-gray-800">
            {currentNote.id ? 'Edit Note' : 'New Note'}
          </h3>
          <button 
            onClick={() => setIsEditingNote(false)}
            className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded-md transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
        
        <div className="flex-1 flex flex-col p-4 overflow-hidden gap-3">
          <input 
            type="text"
            placeholder="Note Title"
            value={currentNote.title || ''}
            onChange={(e) => setCurrentNote({...currentNote, title: e.target.value})}
            className="w-full text-lg font-bold text-gray-900 placeholder-gray-400 border-none focus:ring-0 focus:outline-none px-1"
          />
          
          <div className="flex-1 w-full min-h-0 border border-gray-200 rounded-lg overflow-hidden bg-white flex flex-col">
            <RichTextEditor
              content={currentNote.content || ''}
              onChange={(content) => setCurrentNote({...currentNote, content})}
              placeholder="Start typing your note here..."
            />
          </div>
        </div>

        <div className="p-4 border-t border-gray-200 bg-[#f9f9f9] flex flex-col gap-2">
          <button 
            onClick={handleSaveNote}
            className="flex items-center justify-center gap-2 w-full bg-gray-900 hover:bg-black text-white font-medium py-2 px-4 rounded-lg transition-colors shadow-sm"
          >
            <Save className="w-4 h-4" />
            <span>Save Note</span>
          </button>
          <button 
            onClick={handleConvertToSource}
            className="flex items-center justify-center gap-2 w-full bg-white hover:bg-gray-50 text-gray-800 border border-gray-300 font-medium py-2 px-4 rounded-lg transition-colors shadow-sm"
          >
            <ArrowRightLeft className="w-4 h-4" />
            <span>Convert to Source</span>
          </button>
          <button 
            onClick={() => handleDeleteNote(currentNote.id)}
            className="flex items-center justify-center gap-2 w-full hover:bg-red-50 text-red-600 font-medium py-2 px-4 rounded-lg transition-colors mt-1"
          >
            <Trash2 className="w-4 h-4" />
            <span>Delete Note</span>
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="w-[340px] border-l border-gray-200 bg-[#f9f9f9] flex flex-col h-full shrink-0 relative">
      {/* Source Modal Overlay */}
      {showSourceModal && (
        <div className="fixed inset-0 bg-black/40 z-50 flex items-center justify-center backdrop-blur-sm">
          <div className="bg-white rounded-2xl shadow-2xl w-[480px] max-w-[90vw] overflow-hidden flex flex-col animate-in fade-in zoom-in duration-200">
            <div className="px-5 py-4 border-b border-gray-100 flex items-center justify-between bg-[#fcfcfc]">
              <h2 className="text-lg font-bold text-gray-900">Add New Source</h2>
              <button 
                onClick={() => setShowSourceModal(false)}
                className="p-1.5 text-gray-500 hover:bg-gray-100 hover:text-gray-900 rounded-md transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>
            
            <div className="flex border-b border-gray-200 px-2 pt-2 bg-[#fcfcfc]">
              <button 
                onClick={() => setSourceTab('file')}
                className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors flex items-center gap-2 ${sourceTab === 'file' ? 'border-gray-900 text-gray-900' : 'border-transparent text-gray-500 hover:text-gray-700'}`}
              >
                <File className="w-4 h-4" /> Upload File
              </button>
              <button 
                onClick={() => setSourceTab('link')}
                className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors flex items-center gap-2 ${sourceTab === 'link' ? 'border-gray-900 text-gray-900' : 'border-transparent text-gray-500 hover:text-gray-700'}`}
              >
                <LinkIcon className="w-4 h-4" /> Web Link
              </button>
              <button 
                onClick={() => setSourceTab('text')}
                className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors flex items-center gap-2 ${sourceTab === 'text' ? 'border-gray-900 text-gray-900' : 'border-transparent text-gray-500 hover:text-gray-700'}`}
              >
                <CopyIcon className="w-4 h-4" /> Paste Text
              </button>
            </div>

            <div className="p-6 bg-white min-h-[240px] flex flex-col">
              {sourceTab === 'file' && (
                <div 
                  className={`flex-1 border-2 border-dashed rounded-xl flex flex-col items-center justify-center p-6 transition-colors ${isDragging ? 'border-gray-900 bg-gray-50' : 'border-gray-300 hover:border-gray-400 bg-gray-50/50'}`}
                  onDragOver={handleDragOver}
                  onDragLeave={handleDragLeave}
                  onDrop={handleDrop}
                >
                  <div className="w-12 h-12 bg-white rounded-full flex items-center justify-center shadow-sm border border-gray-100 mb-4">
                    <UploadCloud className="w-6 h-6 text-gray-600" />
                  </div>
                  <p className="text-gray-800 font-medium text-center mb-1">
                    Drag and drop your files here
                  </p>
                  <p className="text-sm text-gray-500 text-center mb-4">
                    Supports PDF, DOCX, TXT, CSV (Max 50MB)
                  </p>
                  <button 
                    onClick={handleManualUpload}
                    className="bg-white border border-gray-300 text-gray-800 hover:bg-gray-50 px-4 py-2 rounded-lg text-sm font-medium transition-colors shadow-sm"
                  >
                    Browse Files
                  </button>
                </div>
              )}
              
              {sourceTab === 'link' && (
                <div className="flex-1 flex flex-col gap-4">
                  <p className="text-sm text-gray-600">Context-OS will scrape and process the content from the provided URL.</p>
                  <input 
                    type="url" 
                    placeholder="https://example.com/article" 
                    className="w-full border border-gray-300 rounded-lg px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 focus:border-transparent transition-all"
                    autoFocus
                  />
                  <button 
                    onClick={() => {
                      setSources([{ id: Date.now(), title: 'Web Article', selected: true }, ...sources]);
                      setShowSourceModal(false);
                    }}
                    className="mt-auto w-full bg-gray-900 hover:bg-black text-white font-medium py-2.5 px-4 rounded-lg transition-colors shadow-sm"
                  >
                    Add Link
                  </button>
                </div>
              )}

              {sourceTab === 'text' && (
                <div className="flex-1 flex flex-col gap-4">
                  <textarea 
                    placeholder="Paste plain text here..." 
                    className="w-full flex-1 border border-gray-300 rounded-lg px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 focus:border-transparent transition-all resize-none min-h-[120px]"
                    autoFocus
                  />
                  <input 
                    type="text" 
                    placeholder="Document Title (optional)" 
                    className="w-full border border-gray-300 rounded-lg px-4 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 focus:border-transparent transition-all"
                  />
                  <button 
                    onClick={() => {
                      setSources([{ id: Date.now(), title: 'Pasted Text Snippet', selected: true }, ...sources]);
                      setShowSourceModal(false);
                    }}
                    className="mt-auto w-full bg-gray-900 hover:bg-black text-white font-medium py-2.5 px-4 rounded-lg transition-colors shadow-sm"
                  >
                    Save as Source
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Top Half: Sources */}
      <div className="flex-[0.5] border-b border-gray-200 flex flex-col overflow-hidden">
        <div className="px-4 py-3 border-b border-gray-100 bg-white/50">
          <h3 className="font-semibold text-gray-800 flex items-center gap-2">
            Sources
            <span className="bg-gray-200 text-gray-700 text-xs px-2 py-0.5 rounded-full font-medium">{sources.length}</span>
          </h3>
        </div>
        
        <div className="p-4 space-y-3">
          <button 
            onClick={() => setShowSourceModal(true)}
            className="flex items-center gap-2 bg-white hover:bg-gray-50 text-gray-900 border border-gray-200 font-medium py-2 px-4 rounded-full transition-colors w-full justify-center shadow-sm"
          >
            <Plus className="w-5 h-5" />
            <span>New Source</span>
          </button>

          <div className="flex items-center justify-between px-1 text-sm text-gray-600">
            <span className="font-medium">Select all</span>
            <button 
              onClick={toggleSelectAll}
              className={`p-0.5 rounded transition-colors ${allSelected ? 'text-gray-900' : 'text-gray-400'}`}
            >
              {allSelected ? <CheckSquare className="w-5 h-5" /> : <Square className="w-5 h-5" />}
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-2 scrollbar-thin scrollbar-thumb-gray-200">
          {sources.map((source) => (
            <div 
              key={source.id} 
              className={`group relative flex items-center gap-3 p-2 rounded-lg cursor-pointer transition-colors border border-transparent ${source.selected ? 'bg-white shadow-sm border-gray-200' : 'hover:bg-gray-100'}`}
              onMouseLeave={() => setActiveMenuSource(null)}
              onClick={() => toggleSource(source.id)}
            >
              <div className={`shrink-0 transition-colors ${source.selected ? 'text-gray-900' : 'text-gray-300'}`}>
                {source.selected ? <CheckSquare className="w-4 h-4" /> : <Square className="w-4 h-4" />}
              </div>
              <p className="text-sm font-medium text-gray-800 truncate flex-1">{source.title}</p>
              
              <button 
                className="p-1 text-gray-400 hover:text-gray-900 opacity-0 group-hover:opacity-100 transition-opacity"
                onClick={(e) => {
                  e.stopPropagation();
                  setActiveMenuSource(activeMenuSource === source.id ? null : source.id);
                }}
              >
                <MoreVertical className="w-3.5 h-3.5" />
              </button>

              {activeMenuSource === source.id && (
                <div className="absolute right-2 top-full mt-1 w-32 bg-white border border-gray-200 rounded-lg shadow-xl z-20 py-1 overflow-hidden" onClick={e => e.stopPropagation()}>
                  <button className="w-full text-left px-3 py-1.5 text-xs hover:bg-gray-100 flex items-center gap-2 text-gray-700">
                    <Pin className="w-3.5 h-3.5" />
                    Pin
                  </button>
                  <button 
                    onClick={() => {
                      setSources(sources.filter(s => s.id !== source.id));
                      setActiveMenuSource(null);
                    }}
                    className="w-full text-left px-3 py-1.5 text-xs hover:bg-gray-100 flex items-center gap-2 text-red-600"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    Delete
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Bottom Half: Notes */}
      <div className="flex-[0.5] flex flex-col bg-[#f5f5f5] overflow-hidden">
        <div className="px-4 py-3 border-b border-gray-200 flex items-center justify-between bg-white/50">
          <h3 className="font-semibold text-gray-800">Notes</h3>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-4 scrollbar-thin scrollbar-thumb-gray-200">
          
          <button 
            onClick={handleNewNote}
            className="flex items-center gap-2 bg-gray-900 hover:bg-black text-white font-medium py-2 px-4 rounded-full transition-colors w-full justify-center shadow-sm"
          >
            <Plus className="w-5 h-5" />
            <span>New Note</span>
          </button>

          {/* Saved Notes Section */}
          <div className="space-y-3 mt-4">
            <div className="flex items-center justify-between">
              <h4 className="text-xs font-semibold text-gray-500 uppercase tracking-wider px-1">Saved Notes</h4>
            </div>
            
            {notes.map((note) => (
              <div 
                key={note.id} 
                onClick={() => handleEditNote(note)}
                className="relative bg-white p-3 rounded-lg shadow-sm border border-gray-200 cursor-pointer hover:border-gray-400 hover:shadow-md transition-all group"
                onMouseLeave={() => setActiveMenuNote(null)}
              >
                <div className="flex items-start justify-between gap-2 mb-1">
                  <h5 className="font-semibold text-sm text-gray-800 group-hover:text-black transition-colors line-clamp-1">{note.title}</h5>
                  <button 
                    className="p-1 text-gray-400 hover:text-gray-900 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                    onClick={(e) => {
                      e.stopPropagation();
                      setActiveMenuNote(activeMenuNote === note.id ? null : note.id);
                    }}
                  >
                    <MoreVertical className="w-3.5 h-3.5" />
                  </button>
                </div>
                <p className="text-xs text-gray-500 line-clamp-2 leading-relaxed">{note.preview}</p>

                {activeMenuNote === note.id && (
                  <div className="absolute right-2 top-8 w-40 bg-white border border-gray-200 rounded-lg shadow-xl z-20 py-1 overflow-hidden" onClick={e => e.stopPropagation()}>
                    <button 
                      onClick={(e) => {
                        e.stopPropagation();
                        // Convert specific note
                        const newSource = {
                          id: Date.now(),
                          title: note.title + '.txt',
                          selected: true,
                        };
                        setSources([newSource, ...sources]);
                        setActiveMenuNote(null);
                      }}
                      className="w-full text-left px-3 py-1.5 text-xs hover:bg-gray-100 flex items-center gap-2 text-gray-700"
                    >
                      <ArrowRightLeft className="w-3.5 h-3.5" />
                      转换为内容源
                    </button>
                    <button 
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteNote(note.id);
                      }}
                      className="w-full text-left px-3 py-1.5 text-xs hover:bg-gray-100 flex items-center gap-2 text-red-600"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                      删除
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>

        </div>
      </div>
    </div>
  );
}
