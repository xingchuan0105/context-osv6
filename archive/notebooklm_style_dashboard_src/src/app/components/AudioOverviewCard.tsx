import React, { useState } from 'react';
import { Play, Pause, FastForward, Rewind, Volume2, MoreHorizontal, FileAudio } from 'lucide-react';
import { motion } from 'motion/react';

export const AudioOverviewCard: React.FC = () => {
  const [isPlaying, setIsPlaying] = useState(false);

  return (
    <div className="relative overflow-hidden w-full max-w-xl bg-gradient-to-br from-indigo-50 via-purple-50 to-pink-50 border border-indigo-100 rounded-2xl p-6 shadow-sm group hover:shadow-md transition-shadow">
      {/* Decorative Blob */}
      <div className="absolute top-0 right-0 -mr-16 -mt-16 w-48 h-48 bg-purple-200/40 rounded-full blur-3xl opacity-60 pointer-events-none" />

      <div className="flex justify-between items-start mb-6 relative z-10">
        <div>
          <h3 className="text-lg font-semibold text-indigo-950 flex items-center gap-2">
            <FileAudio size={18} className="text-indigo-600" />
            Deep Dive
          </h3>
          <p className="text-sm text-indigo-700/80 mt-1 font-medium">12 mins • AI Hosts</p>
        </div>
        <button className="p-2 bg-white/60 hover:bg-white rounded-full text-indigo-900 transition-colors backdrop-blur-sm">
          <MoreHorizontal size={18} />
        </button>
      </div>

      <div className="flex items-center gap-4 relative z-10">
        {/* Play Button */}
        <button
          onClick={() => setIsPlaying(!isPlaying)}
          className="flex items-center justify-center w-14 h-14 bg-indigo-600 hover:bg-indigo-700 text-white rounded-full shadow-lg shadow-indigo-600/30 transition-transform active:scale-95 flex-shrink-0"
        >
          {isPlaying ? <Pause size={24} fill="currentColor" /> : <Play size={24} fill="currentColor" className="ml-1" />}
        </button>

        {/* Player Controls & Progress */}
        <div className="flex-1 flex flex-col gap-2">
          {/* Progress Bar (Visual only) */}
          <div className="relative h-2 w-full bg-white/60 rounded-full overflow-hidden cursor-pointer backdrop-blur-sm shadow-inner">
            <motion.div
              className="absolute top-0 left-0 h-full bg-indigo-500 rounded-full"
              initial={{ width: '0%' }}
              animate={{ width: isPlaying ? '100%' : '35%' }}
              transition={{ duration: isPlaying ? 600 : 0.5, ease: 'linear' }}
            />
          </div>

          <div className="flex justify-between items-center text-xs font-medium text-indigo-800/70">
            <span>4:15</span>

            <div className="flex items-center gap-4">
              <button className="hover:text-indigo-900 transition-colors"><Rewind size={16} /></button>
              <button className="hover:text-indigo-900 transition-colors"><FastForward size={16} /></button>
              <button className="hover:text-indigo-900 transition-colors"><Volume2 size={16} /></button>
            </div>

            <span>12:00</span>
          </div>
        </div>
      </div>
    </div>
  );
};
