'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useAdminStore } from '@/stores/useAdminStore';
import { metricsApi, type MetricsData } from '@/lib/api/admin';

function sortedEntries(values?: Record<string, number>): [string, number][] {
  return Object.entries(values || {})
    .sort((a, b) => b[1] - a[1])
    .slice(0, 8);
}

function MetricList({
  title,
  values,
  emptyLabel,
  valueClassName = 'text-sm font-semibold',
}: {
  title: string;
  values?: Record<string, number>;
  emptyLabel: string;
  valueClassName?: string;
}) {
  const entries = sortedEntries(values);

  return (
    <div>
      <h3 className="text-sm font-semibold text-gray-700 mb-2">{title}</h3>
      <div className="space-y-3">
        {entries.map(([key, value]) => (
          <div key={key} className="flex items-center justify-between border-b border-gray-100 pb-2 last:border-0">
            <span className="text-sm text-gray-700 break-all pr-3">{key}</span>
            <span className={valueClassName}>{value.toLocaleString()}</span>
          </div>
        ))}
        {entries.length === 0 && (
          <div className="text-sm text-gray-500">{emptyLabel}</div>
        )}
      </div>
    </div>
  );
}

export default function AdminDashboardPage() {
  const router = useRouter();
  const { isAuthenticated, isLoading: authChecking, checkAuth, logout } = useAdminStore();
  const [metrics, setMetrics] = useState<MetricsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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

    const fetchMetrics = async () => {
      try {
        const response = await metricsApi.getMetrics();
        if (response.success && response.data) {
          setMetrics(response.data);
          setError(null);
        } else {
          setError(response.error || 'Failed to load metrics');
        }
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : 'Failed to load metrics';
        setError(message);
      } finally {
        setLoading(false);
      }
    };

    void fetchMetrics();
  }, [authChecking, isAuthenticated, router]);

  const handleLogout = async () => {
    await logout();
    router.push('/admin/login');
  };

  if (loading || authChecking) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-gray-600">Loading...</div>
      </div>
    );
  }

  return (
    <div>
      <div className="flex justify-between items-center mb-6">
        <div>
          <h1 className="text-2xl font-bold">Observability</h1>
          <p className="text-sm text-gray-500 mt-1">
            {metrics?.generated_at ? `Snapshot: ${new Date(metrics.generated_at).toLocaleString()}` : 'Live system snapshot'}
          </p>
        </div>
        <button
          onClick={handleLogout}
          className="bg-red-600 hover:bg-red-700 text-white px-4 py-2 rounded"
        >
          Logout
        </button>
      </div>

      {error && (
        <div className="mb-4 p-3 bg-red-100 border border-red-400 text-red-700 rounded">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4 mb-8">
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Database</h3>
          <p className="text-3xl font-bold">{metrics?.database ? 'OK' : 'DOWN'}</p>
        </div>
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Active Tenants</h3>
          <p className="text-3xl font-bold">{metrics?.active_tenants ?? '-'}</p>
        </div>
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Documents</h3>
          <p className="text-3xl font-bold">{metrics?.documents?.toLocaleString() ?? '-'}</p>
        </div>
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Active Subscriptions</h3>
          <p className="text-3xl font-bold">{metrics?.active_subscriptions ?? '-'}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4 mb-8">
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Eino Compiles</h3>
          <p className="text-2xl font-bold">{metrics?.observability?.summary?.eino_graph_compiles_total?.toLocaleString() ?? '-'}</p>
        </div>
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Eino Runs</h3>
          <p className="text-2xl font-bold">{metrics?.observability?.summary?.eino_runs_total?.toLocaleString() ?? '-'}</p>
        </div>
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Eino Run Errors</h3>
          <p className="text-2xl font-bold">{metrics?.observability?.summary?.eino_run_errors_total?.toLocaleString() ?? '-'}</p>
        </div>
        <div className="bg-white p-6 rounded-lg shadow">
          <h3 className="text-gray-500 text-sm">Fallback Events</h3>
          <p className="text-2xl font-bold">{metrics?.observability?.summary?.fallback_events_total?.toLocaleString() ?? '-'}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        <div className="bg-white p-6 rounded-lg shadow">
          <MetricList
            title="Top Eino Graph Runs"
            values={metrics?.observability?.eino_graph_run_total}
            emptyLabel="No Eino run metrics yet."
          />
        </div>

        <div className="bg-white p-6 rounded-lg shadow">
          <MetricList
            title="Top Eino Compile Counts"
            values={metrics?.observability?.eino_graph_compile_total}
            emptyLabel="No Eino compile metrics yet."
          />
        </div>

        <div className="bg-white p-6 rounded-lg shadow">
          <MetricList
            title="Top Node Errors"
            values={metrics?.observability?.eino_graph_run_error_total}
            emptyLabel="No Eino errors recorded."
            valueClassName="text-sm font-semibold text-red-700"
          />
        </div>

        <div className="bg-white p-6 rounded-lg shadow">
          <MetricList
            title="Top Fallback Events"
            values={metrics?.observability?.fallback_events_total}
            emptyLabel="No fallback metrics recorded."
            valueClassName="text-sm font-semibold text-amber-700"
          />
        </div>
      </div>
    </div>
  );
}
