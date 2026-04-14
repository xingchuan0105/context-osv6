'use client';

import { useEffect, useState } from 'react';
import {
  AlertTriangle,
  ArrowUpRight,
  CheckCircle2,
  CreditCard,
  Loader2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { billingApi } from '@/lib/api/client';
import type {
  BillingPlan,
  BillingPlanQuota,
  BillingSubscription,
  BillingUsage,
  BillingUsageMetric,
} from '@/types';
import { toast } from '@/components/ui/toaster';

const EMPTY_USAGE: BillingUsage = {
  pages_processed: 0,
  embedding_tokens: 0,
  llm_input_tokens: 0,
  llm_output_tokens: 0,
  storage_bytes: 0,
};

const USAGE_METRICS: BillingUsageMetric[] = [
  'pages_processed',
  'embedding_tokens',
  'llm_input_tokens',
  'llm_output_tokens',
  'storage_bytes',
];

interface BillingPanelProps {
  onBack: () => void;
}

function formatDate(value?: string): string {
  if (!value) {
    return '';
  }
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleDateString();
}

function metricLabel(t: (key: string) => string, metric: BillingUsageMetric): string {
  switch (metric) {
    case 'pages_processed':
      return t('billing.metricPagesProcessed');
    case 'embedding_tokens':
      return t('billing.metricEmbeddingTokens');
    case 'llm_input_tokens':
      return t('billing.metricLLMInputTokens');
    case 'llm_output_tokens':
      return t('billing.metricLLMOutputTokens');
    case 'storage_bytes':
      return t('billing.metricStorage');
    default:
      return metric;
  }
}

function formatMetricValue(metric: BillingUsageMetric, value: number): string {
  if (metric === 'storage_bytes') {
    return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
  return value.toLocaleString();
}

function formatStatus(t: (key: string) => string, value: string): string {
  switch (value.trim().toLowerCase()) {
    case 'active':
      return t('billing.statusActive');
    case 'past_due':
      return t('billing.statusPastDue');
    case 'unpaid':
      return t('billing.statusUnpaid');
    case 'canceled':
      return t('billing.statusCanceled');
    default:
      return value || t('billing.statusUnknown');
  }
}

function quotaLimit(quota?: BillingPlanQuota): number | undefined {
  if (!quota) {
    return undefined;
  }
  if (typeof quota.hard_limit === 'number') {
    return quota.hard_limit;
  }
  if (typeof quota.soft_limit === 'number') {
    return quota.soft_limit;
  }
  return undefined;
}

function formatQuotaValue(
  t: (key: string) => string,
  metric: BillingUsageMetric,
  quota?: BillingPlanQuota,
): string {
  const limit = quotaLimit(quota);
  if (typeof limit !== 'number') {
    return t('billing.unlimited');
  }
  return formatMetricValue(metric, limit);
}

export function BillingPanel({ onBack }: BillingPanelProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [redirectingAction, setRedirectingAction] = useState('');
  const [plans, setPlans] = useState<BillingPlan[]>([]);
  const [currentPlanID, setCurrentPlanID] = useState('free');
  const [subscription, setSubscription] = useState<BillingSubscription | null>(
    null,
  );
  const [usage, setUsage] = useState<BillingUsage>(EMPTY_USAGE);

  useEffect(() => {
    let cancelled = false;

    const loadBilling = async () => {
      setLoading(true);
      setError('');

      const [plansResult, subscriptionResult, usageResult] = await Promise.all([
        billingApi.listPlans(),
        billingApi.getSubscription(),
        billingApi.getUsage(),
      ]);

      if (cancelled) {
        return;
      }

      if (!plansResult.success) {
        setError(plansResult.error || t('billing.loadFailed'));
        setLoading(false);
        return;
      }
      if (!subscriptionResult.success) {
        setError(subscriptionResult.error || t('billing.loadFailed'));
        setLoading(false);
        return;
      }
      if (!usageResult.success) {
        setError(usageResult.error || t('billing.loadFailed'));
        setLoading(false);
        return;
      }

      setPlans(plansResult.data.plans);
      setSubscription(subscriptionResult.data);
      setUsage(usageResult.data);
      setCurrentPlanID(
        plansResult.data.current_plan_id || subscriptionResult.data.plan_id || 'free',
      );
      setLoading(false);
    };

    void loadBilling();
    return () => {
      cancelled = true;
    };
  }, [t]);

  const currentPlan =
    plans.find((plan) => plan.plan_id === currentPlanID) ||
    plans.find((plan) => plan.current) ||
    null;

  const handleCheckout = async (planID: string) => {
    setRedirectingAction(`checkout:${planID}`);
    try {
      const result = await billingApi.createCheckoutSession(planID);
      if (!result.success || !result.data.url) {
        toast.error(result.error || t('billing.checkoutFailed'));
        return;
      }
      window.location.assign(result.data.url);
    } finally {
      setRedirectingAction('');
    }
  };

  const handlePortal = async () => {
    setRedirectingAction('portal');
    try {
      const result = await billingApi.createPortalSession();
      if (!result.success || !result.data.url) {
        toast.error(result.error || t('billing.portalFailed'));
        return;
      }
      window.location.assign(result.data.url);
    } finally {
      setRedirectingAction('');
    }
  };

  return (
    <div className="space-y-4">
      <button
        onClick={onBack}
        className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        type="button"
      >
        ← {t('common.back')}
      </button>

      <div className="space-y-1">
        <h3 className="flex items-center gap-2 text-base font-semibold">
          <CreditCard className="h-4 w-4" />
          {t('billing.title')}
        </h3>
        <p className="text-sm text-muted-foreground">{t('billing.subtitle')}</p>
      </div>

      {loading ? (
        <div className="flex min-h-[220px] items-center justify-center rounded-2xl border border-border bg-background/40 text-sm text-muted-foreground">
          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          {t('billing.loading')}
        </div>
      ) : error ? (
        <div className="rounded-2xl border border-destructive/20 bg-destructive/10 p-4">
          <div className="flex items-start gap-3">
            <AlertTriangle className="mt-0.5 h-4 w-4 text-destructive" />
            <div className="space-y-1">
              <div className="text-sm font-medium text-destructive">
                {t('billing.loadFailed')}
              </div>
              <div className="text-sm text-destructive/80">{error}</div>
            </div>
          </div>
        </div>
      ) : (
        <div className="space-y-4">
          <section className="rounded-2xl border border-border bg-background/45 p-4 shadow-[var(--shadow-sm)]">
            <div className="flex items-start justify-between gap-3">
              <div className="space-y-2">
                <div className="flex flex-wrap items-center gap-2">
                  <h4 className="text-sm font-semibold text-foreground">
                    {currentPlan?.name || t('billing.freePlan')}
                  </h4>
                  <span className="rounded-full bg-primary/12 px-2 py-0.5 text-[11px] font-medium text-primary">
                    {t('billing.currentBadge')}
                  </span>
                </div>
                <div className="text-sm text-muted-foreground">
                  {currentPlan?.price_label || t('billing.freePlan')}
                </div>
                <div className="text-sm text-muted-foreground">
                  {t('billing.status')}: {formatStatus(t, subscription?.status || 'active')}
                </div>
                {subscription?.current_period_end ? (
                  <div className="text-sm text-muted-foreground">
                    {subscription.cancel_at_period_end
                      ? t('billing.cancelsOn')
                      : t('billing.renewsOn')}
                    : {formatDate(subscription.current_period_end)}
                  </div>
                ) : null}
              </div>

              {currentPlanID !== 'free' ? (
                <button
                  type="button"
                  onClick={() => void handlePortal()}
                  disabled={redirectingAction === 'portal'}
                  className="inline-flex items-center gap-2 rounded-xl border border-border px-3 py-2 text-sm hover:bg-accent disabled:opacity-60"
                >
                  {redirectingAction === 'portal' ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <ArrowUpRight className="h-4 w-4" />
                  )}
                  {t('billing.manageBilling')}
                </button>
              ) : null}
            </div>
          </section>

          <section className="space-y-3">
            <h4 className="text-sm font-medium text-muted-foreground">
              {t('billing.usageTitle')}
            </h4>
            <div className="grid gap-3">
              {USAGE_METRICS.map((metric) => {
                const quota = currentPlan?.quotas.find(
                  (item) => item.metric_type === metric,
                );
                return (
                  <div
                    key={metric}
                    className="rounded-2xl border border-border bg-background/45 p-4 shadow-[var(--shadow-sm)]"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <div className="text-sm font-medium text-foreground">
                          {metricLabel(t, metric)}
                        </div>
                        <div className="mt-1 text-xs text-muted-foreground">
                          {t('billing.quota')}: {formatQuotaValue(t, metric, quota)}
                        </div>
                      </div>
                      <div className="text-right text-sm font-semibold text-foreground">
                        {formatMetricValue(metric, usage[metric])}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          <section className="space-y-3">
            <h4 className="text-sm font-medium text-muted-foreground">
              {t('billing.plansTitle')}
            </h4>
            <div className="grid gap-3">
              {plans.map((plan) => {
                const isCurrent = plan.plan_id === currentPlanID || plan.current;
                const actionKey = `checkout:${plan.plan_id}`;
                return (
                  <div
                    key={plan.plan_id}
                    className={`rounded-2xl border p-4 shadow-[var(--shadow-sm)] ${
                      isCurrent
                        ? 'border-primary/30 bg-primary/5'
                        : 'border-border bg-background/45'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="space-y-2">
                        <div className="flex flex-wrap items-center gap-2">
                          <div className="text-sm font-semibold text-foreground">
                            {plan.name}
                          </div>
                          {isCurrent ? (
                            <span className="inline-flex items-center gap-1 rounded-full bg-primary/12 px-2 py-0.5 text-[11px] font-medium text-primary">
                              <CheckCircle2 className="h-3 w-3" />
                              {t('billing.currentBadge')}
                            </span>
                          ) : null}
                        </div>
                        <div className="text-sm text-muted-foreground">
                          {plan.description}
                        </div>
                        <div className="text-sm text-foreground">
                          {plan.price_label}
                        </div>
                      </div>

                      <button
                        type="button"
                        onClick={
                          !isCurrent && plan.checkout_available
                            ? () => void handleCheckout(plan.plan_id)
                            : undefined
                        }
                        disabled={isCurrent || !plan.checkout_available || redirectingAction === actionKey}
                        className="inline-flex items-center gap-2 rounded-xl border border-border px-3 py-2 text-sm hover:bg-accent disabled:opacity-60"
                      >
                        {redirectingAction === actionKey ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : null}
                        {isCurrent
                          ? t('billing.currentBadge')
                          : plan.checkout_available
                            ? t('billing.upgrade')
                            : t('billing.contactSupport')}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
