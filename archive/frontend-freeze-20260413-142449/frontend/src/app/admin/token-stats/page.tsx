'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useAdminStore } from '@/stores/useAdminStore';
import { tokenStatsApi, type TokenStats as TokenStatsType } from '@/lib/api/admin';

export default function AdminTokenStatsPage() {
  const router = useRouter();
  const { isAuthenticated, isLoading: authChecking, checkAuth } = useAdminStore();
  const [stats, setStats] = useState<TokenStatsType[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const limit = 20;

  useEffect(() => {
    void checkAuth();
  }, [checkAuth]);

  useEffect(() => {
    if (authChecking) {
      return;
    }
    if (!isAuthenticated) {
      router.push('/admin/login');
      return;
    }

    const fetchStats = async () => {
      setLoading(true);
      try {
        const response = await tokenStatsApi.getTokenStats({
          page,
          limit,
          start_date: startDate || undefined,
          end_date: endDate || undefined,
        });
        if (response.success) {
          setStats(response.data);
          setTotal(response.meta.total);
          setError(null);
        } else {
          setError('Failed to load usage records');
        }
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : 'Failed to load usage records');
      } finally {
        setLoading(false);
      }
    };

    void fetchStats();
  }, [authChecking, isAuthenticated, router, page, startDate, endDate]);

  if (loading || authChecking) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-gray-600">Loading...</div>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-2xl font-bold mb-6">Usage Records</h1>

      {error && (
        <div className="mb-4 p-3 bg-red-100 border border-red-400 text-red-700 rounded">
          {error}
        </div>
      )}

      <form
        onSubmit={(e) => {
          e.preventDefault();
          setPage(1);
        }}
        className="mb-4"
      >
        <div className="flex gap-4 items-end">
          <div>
            <label className="block text-sm text-gray-600 mb-1">Start Date</label>
            <input
              type="date"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
              className="px-4 py-2 border rounded"
            />
          </div>
          <div>
            <label className="block text-sm text-gray-600 mb-1">End Date</label>
            <input
              type="date"
              value={endDate}
              onChange={(e) => setEndDate(e.target.value)}
              className="px-4 py-2 border rounded"
            />
          </div>
          <button type="submit" className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700">
            Filter
          </button>
          <button
            type="button"
            onClick={() => {
              setStartDate('');
              setEndDate('');
              setPage(1);
            }}
            className="px-4 py-2 bg-gray-200 rounded hover:bg-gray-300"
          >
            Clear
          </button>
        </div>
      </form>

      <div className="bg-white rounded-lg shadow overflow-hidden">
        <table className="min-w-full divide-y divide-gray-200">
          <thead className="bg-gray-50">
            <tr>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">Period Start</th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">Period End</th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">Metric Type</th>
              <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase">Quantity</th>
            </tr>
          </thead>
          <tbody className="bg-white divide-y divide-gray-200">
            {stats.length === 0 ? (
              <tr>
                <td colSpan={4} className="px-6 py-4 text-center text-gray-500">
                  No usage records available
                </td>
              </tr>
            ) : (
              stats.map((stat, index) => (
                <tr key={`${stat.period_start}-${stat.metric_type}-${index}`} className="hover:bg-gray-50">
                  <td className="px-6 py-4 whitespace-nowrap text-sm">
                    {new Date(stat.period_start).toLocaleString()}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap text-sm">
                    {new Date(stat.period_end).toLocaleString()}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap text-sm">{stat.metric_type}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-sm text-right font-medium">
                    {stat.quantity.toLocaleString()}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {total > limit && (
        <div className="mt-4 flex justify-center gap-2">
          <button
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page === 1}
            className="px-4 py-2 bg-gray-200 rounded disabled:opacity-50"
          >
            Previous
          </button>
          <span className="px-4 py-2">
            Page {page} of {Math.ceil(total / limit)}
          </span>
          <button
            onClick={() => setPage((p) => p + 1)}
            disabled={page >= Math.ceil(total / limit)}
            className="px-4 py-2 bg-gray-200 rounded disabled:opacity-50"
          >
            Next
          </button>
        </div>
      )}
    </div>
  );
}
