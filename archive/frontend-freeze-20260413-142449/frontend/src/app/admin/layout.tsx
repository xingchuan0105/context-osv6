import React from 'react';
import { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Admin Dashboard',
  description: 'Administrator dashboard for managing users, tokens, and system performance',
};

export default function AdminLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="flex h-screen bg-gray-50">
      {/* Sidebar */}
      <aside className="w-64 bg-white shadow-md">
        <div className="p-4 border-b">
          <h1 className="text-xl font-bold text-gray-800">Admin Panel</h1>
        </div>
        <nav className="p-4">
          <ul className="space-y-2">
            <li>
              <a href="/admin/dashboard" className="block py-2 px-4 rounded hover:bg-gray-100 bg-blue-100 text-blue-700 font-medium">
                Dashboard
              </a>
            </li>
            <li>
              <a href="/admin/users" className="block py-2 px-4 rounded hover:bg-gray-100 text-gray-700">
                Users
              </a>
            </li>
            <li>
              <a href="/admin/token-stats" className="block py-2 px-4 rounded hover:bg-gray-100 text-gray-700">
                Token Usage Stats
              </a>
            </li>
            <li>
              <a href="/admin/admins" className="block py-2 px-4 rounded hover:bg-gray-100 text-gray-700">
                Admin Management
              </a>
            </li>
            <li>
              <a href="/admin/logs" className="block py-2 px-4 rounded hover:bg-gray-100 text-gray-700">
                Operation Logs
              </a>
            </li>
          </ul>
        </nav>
      </aside>

      {/* Main Content */}
      <main className="flex-1 overflow-x-hidden overflow-y-auto bg-gray-50">
        <div className="container mx-auto px-6 py-8">
          {children}
        </div>
      </main>
    </div>
  );
}