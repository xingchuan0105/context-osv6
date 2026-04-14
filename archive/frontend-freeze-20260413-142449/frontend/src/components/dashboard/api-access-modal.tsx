'use client';

import { useEffect, useMemo, useState } from 'react';
import { Copy, KeyRound, Loader2, PlugZap, Shield, Trash2, X } from 'lucide-react';
import { kbApi } from '@/lib/api/client';
import type { KnowledgeBase, NotebookAPIKey } from '@/types';
import { toast } from '@/components/ui/toaster';
import { useTranslation } from 'react-i18next';

interface APIAccessModalProps {
  open: boolean;
  workspace: KnowledgeBase | null;
  onOpenChange: (open: boolean) => void;
}

type PermissionValue = 'query' | 'index' | 'admin';

const DEFAULT_RATE_LIMIT_RPM = 60;

export function APIAccessModal({
  open,
  workspace,
  onOpenChange,
}: APIAccessModalProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [revokingKeyID, setRevokingKeyID] = useState<string | null>(null);
  const [keys, setKeys] = useState<NotebookAPIKey[]>([]);
  const [keyName, setKeyName] = useState('');
  const [permission, setPermission] = useState<PermissionValue>('query');
  const [rateLimitInput, setRateLimitInput] = useState<string>(String(DEFAULT_RATE_LIMIT_RPM));
  const [latestPlaintextKey, setLatestPlaintextKey] = useState('');

  const baseURL = useMemo(() => {
    if (typeof window === 'undefined') {
      return '';
    }
    return window.location.origin;
  }, []);

  useEffect(() => {
    if (!open || !workspace) {
      return;
    }

    let cancelled = false;
    const loadKeys = async () => {
      setLoading(true);
      try {
        const response = await kbApi.listAPIKeys(workspace.id);
        if (!cancelled) {
          if (response.success) {
            setKeys(response.data || []);
          } else {
            toast.error(response.error || t('apiAccess.loadFailed'));
          }
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void loadKeys();
    return () => {
      cancelled = true;
    };
  }, [open, t, workspace]);

  useEffect(() => {
    if (!open) {
      setLatestPlaintextKey('');
      setKeyName('');
      setPermission('query');
      setRateLimitInput(String(DEFAULT_RATE_LIMIT_RPM));
    }
  }, [open]);

  const curlSnippet = useMemo(() => {
    if (!workspace || !baseURL) {
      return '';
    }
    return [
      `curl -X POST '${baseURL}/api/v1/notebooks/${workspace.id}/query' \\`,
      "  -H 'Authorization: Bearer <NOTEBOOK_API_KEY>' \\",
      "  -H 'Content-Type: application/json' \\",
      `  -d '{"query":"Summarize the uploaded documents"}'`,
    ].join('\n');
  }, [baseURL, workspace]);

  const openAISnippet = useMemo(() => {
    if (!workspace || !baseURL) {
      return '';
    }
    return [
      'from openai import OpenAI',
      '',
      `client = OpenAI(api_key="<NOTEBOOK_API_KEY>", base_url="${baseURL}/v1/notebooks/${workspace.id}")`,
      'resp = client.chat.completions.create(',
      '    model="context-osv5",',
      '    messages=[{"role": "user", "content": "Summarize the notebook"}],',
      ')',
      'print(resp.choices[0].message.content)',
    ].join('\n');
  }, [baseURL, workspace]);

  const mcpSnippet = useMemo(() => {
    if (!workspace || !baseURL) {
      return '';
    }
    return [
      '{',
      '  "mcpServers": {',
      '    "context-osv5": {',
      `      "url": "${baseURL}/mcp/notebooks/${workspace.id}",`,
      '      "headers": {',
      '        "Authorization": "Bearer <NOTEBOOK_API_KEY>"',
      '      }',
      '    }',
      '  }',
      '}',
    ].join('\n');
  }, [baseURL, workspace]);

  if (!open || !workspace) {
    return null;
  }

  const handleCreateKey = async () => {
    if (!workspace || !keyName.trim()) {
      toast.error(t('apiAccess.nameRequired'));
      return;
    }
    setCreating(true);
    try {
      const response = await kbApi.createAPIKey(workspace.id, {
        name: keyName.trim(),
        permissions: [permission],
        rate_limit_rpm:
          Number.parseInt(rateLimitInput, 10) > 0
            ? Number.parseInt(rateLimitInput, 10)
            : DEFAULT_RATE_LIMIT_RPM,
      });
      if (!response.success || !response.data) {
        toast.error(response.error || t('apiAccess.createFailed'));
        return;
      }
      setKeys((prev) => [response.data.api_key, ...prev]);
      setLatestPlaintextKey(response.data.plaintext_key);
      setKeyName('');
      setPermission('query');
      setRateLimitInput(String(DEFAULT_RATE_LIMIT_RPM));
      toast.success(t('apiAccess.createSuccess'));
    } finally {
      setCreating(false);
    }
  };

  const handleRevokeKey = async (keyID: string) => {
    if (!workspace) {
      return;
    }
    setRevokingKeyID(keyID);
    try {
      const response = await kbApi.revokeAPIKey(workspace.id, keyID);
      if (!response.success) {
        toast.error(response.error || t('apiAccess.revokeFailed'));
        return;
      }
      setKeys((prev) => prev.filter((item) => item.id !== keyID));
      toast.success(t('apiAccess.revokeSuccess'));
    } finally {
      setRevokingKeyID(null);
    }
  };

  const copyText = async (value: string, successMessage: string) => {
    if (!navigator.clipboard?.writeText || !value) {
      toast.error(t('apiAccess.copyFailed'));
      return;
    }
    try {
      await navigator.clipboard.writeText(value);
      toast.success(successMessage);
    } catch {
      toast.error(t('apiAccess.copyFailed'));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div className="w-full max-w-5xl rounded-3xl border border-border bg-card/94 shadow-[var(--shadow-lg)] backdrop-blur-xl">
        <div className="p-4 border-b border-border flex items-center justify-between gap-3">
          <div>
            <h3 className="text-base font-semibold">{t('apiAccess.title')}</h3>
            <p className="text-sm text-muted-foreground">{workspace.title}</p>
          </div>
          <button
            className="p-2 rounded-lg hover:bg-accent"
            onClick={() => onOpenChange(false)}
            aria-label={t('common.close')}
            type="button"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="grid gap-6 p-4 lg:grid-cols-[1.05fr_0.95fr]">
          <section className="space-y-4">
            <div className="rounded-2xl border border-border bg-background/50 p-4">
              <div className="flex items-center gap-2 text-sm font-medium">
                <KeyRound className="w-4 h-4" />
                {t('apiAccess.createKey')}
              </div>
              <div className="mt-4 grid gap-3 sm:grid-cols-2">
                <div className="sm:col-span-2">
                  <label htmlFor="api-access-key-name" className="mb-1 block text-sm text-muted-foreground">{t('apiAccess.keyName')}</label>
                  <input
                    id="api-access-key-name"
                    type="text"
                    value={keyName}
                    onChange={(event) => setKeyName(event.target.value)}
                    className="w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 text-sm"
                    placeholder={t('apiAccess.keyNamePlaceholder')}
                  />
                </div>
                <div>
                  <label htmlFor="api-access-permission" className="mb-1 block text-sm text-muted-foreground">{t('apiAccess.permission')}</label>
                  <select
                    id="api-access-permission"
                    value={permission}
                    onChange={(event) => setPermission(event.target.value as PermissionValue)}
                    className="w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 text-sm"
                  >
                    <option value="query">{t('apiAccess.permissionQuery')}</option>
                    <option value="index">{t('apiAccess.permissionIndex')}</option>
                    <option value="admin">{t('apiAccess.permissionAdmin')}</option>
                  </select>
                </div>
                <div>
                  <label htmlFor="api-access-rate-limit" className="mb-1 block text-sm text-muted-foreground">{t('apiAccess.rateLimit')}</label>
                  <input
                    id="api-access-rate-limit"
                    type="number"
                    min={1}
                    value={rateLimitInput}
                    onChange={(event) => setRateLimitInput(event.target.value)}
                    className="w-full rounded-xl border border-border bg-background/65 px-3 py-2.5 text-sm"
                  />
                </div>
              </div>
              <div className="mt-4 flex items-center justify-between gap-3">
                <p className="text-xs text-muted-foreground">{t('apiAccess.keyHint')}</p>
                <button
                  onClick={() => void handleCreateKey()}
                  disabled={creating}
                  className="inline-flex items-center gap-2 rounded-xl bg-primary px-3 py-2 text-sm text-primary-foreground disabled:opacity-60"
                  type="button"
                >
                  {creating ? <Loader2 className="w-4 h-4 animate-spin" /> : <Shield className="w-4 h-4" />}
                  {t('apiAccess.createKey')}
                </button>
              </div>
              {latestPlaintextKey ? (
                <div className="mt-4 rounded-2xl border border-emerald-500/30 bg-emerald-500/10 p-3">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="text-sm font-medium">{t('apiAccess.plaintextKey')}</div>
                      <div className="text-xs text-muted-foreground">{t('apiAccess.copyNow')}</div>
                    </div>
                    <button
                      onClick={() => void copyText(latestPlaintextKey, t('apiAccess.copySuccess'))}
                      className="rounded-lg border border-border px-2.5 py-1.5 text-xs hover:bg-accent"
                      type="button"
                    >
                      <Copy className="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <pre className="mt-2 overflow-x-auto rounded-xl bg-background/70 p-3 text-xs">{latestPlaintextKey}</pre>
                </div>
              ) : null}
            </div>

            <div className="rounded-2xl border border-border bg-background/50 p-4">
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <PlugZap className="w-4 h-4" />
                  {t('apiAccess.activeKeys')}
                </div>
                {loading ? <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" /> : null}
              </div>
              <div className="mt-3 space-y-3">
                {!loading && keys.length === 0 ? (
                  <div className="rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                    {t('apiAccess.empty')}
                  </div>
                ) : null}
                {keys.map((key) => (
                  <div key={key.id} className="rounded-xl border border-border p-3">
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <div className="text-sm font-medium">{key.name}</div>
                        <div className="mt-1 text-xs text-muted-foreground">
                          {key.key_prefix} · {key.permissions.join(', ')} · {t('apiAccess.rateLimitValue', { rpm: key.rate_limit_rpm })}
                        </div>
                        <div className="mt-1 text-xs text-muted-foreground">
                          {t('apiAccess.lastUsed')}: {key.last_used_at ? new Date(key.last_used_at).toLocaleString() : t('apiAccess.neverUsed')}
                        </div>
                      </div>
                      <button
                        onClick={() => void handleRevokeKey(key.id)}
                        disabled={revokingKeyID === key.id}
                        className="inline-flex items-center gap-1 rounded-lg border border-border px-2.5 py-1.5 text-xs hover:bg-accent disabled:opacity-60"
                        type="button"
                      >
                        {revokingKeyID === key.id ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Trash2 className="w-3.5 h-3.5" />}
                        {t('apiAccess.revoke')}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </section>

          <section className="space-y-4">
            <SnippetCard
              title={t('apiAccess.restExample')}
              value={curlSnippet}
              onCopy={() => void copyText(curlSnippet, t('apiAccess.copySuccess'))}
            />
            <SnippetCard
              title={t('apiAccess.openAIExample')}
              value={openAISnippet}
              onCopy={() => void copyText(openAISnippet, t('apiAccess.copySuccess'))}
            />
            <SnippetCard
              title={t('apiAccess.mcpExample')}
              value={mcpSnippet}
              onCopy={() => void copyText(mcpSnippet, t('apiAccess.copySuccess'))}
            />
          </section>
        </div>
      </div>
    </div>
  );
}

function SnippetCard({
  title,
  value,
  onCopy,
}: {
  title: string;
  value: string;
  onCopy: () => void;
}) {
  return (
    <div className="rounded-2xl border border-border bg-background/50 p-4">
      <div className="flex items-center justify-between gap-3">
        <div className="text-sm font-medium">{title}</div>
        <button
          onClick={onCopy}
          className="rounded-lg border border-border px-2.5 py-1.5 text-xs hover:bg-accent"
          type="button"
        >
          <Copy className="w-3.5 h-3.5" />
        </button>
      </div>
      <pre className="mt-3 overflow-x-auto rounded-xl bg-card p-3 text-xs leading-5">{value}</pre>
    </div>
  );
}
