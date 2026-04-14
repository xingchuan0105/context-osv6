'use client';

import { ReactNode } from 'react';
import { TopNav } from '@/components/layout/top-nav';

interface DashboardLayoutProps {
  children: ReactNode;
}

export function DashboardLayout({ children }: DashboardLayoutProps) {
  return (
    <div className="h-screen flex flex-col bg-background">
      <TopNav />
      <div className="flex-1 flex overflow-hidden">
        {children}
      </div>
    </div>
  );
}
