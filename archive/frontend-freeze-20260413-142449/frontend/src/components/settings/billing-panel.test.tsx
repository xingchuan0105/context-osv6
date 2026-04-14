// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BillingPanel } from './billing-panel';
import { billingApi } from '@/lib/api/client';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock('@/lib/api/client', () => ({
  billingApi: {
    listPlans: vi.fn(),
    getSubscription: vi.fn(),
    getUsage: vi.fn(),
    createCheckoutSession: vi.fn(),
    createPortalSession: vi.fn(),
  },
}));

describe('BillingPanel', () => {
  const listPlansMock = vi.mocked(billingApi.listPlans);
  const getSubscriptionMock = vi.mocked(billingApi.getSubscription);
  const getUsageMock = vi.mocked(billingApi.getUsage);
  const createCheckoutSessionMock = vi.mocked(billingApi.createCheckoutSession);
  const createPortalSessionMock = vi.mocked(billingApi.createPortalSession);
  const assignMock = vi.fn();

  beforeEach(() => {
    vi.resetAllMocks();
    assignMock.mockReset();
    vi.stubGlobal('location', {
      assign: assignMock,
    });
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it('renders current plan and usage after successful load', async () => {
    listPlansMock.mockResolvedValue({
      success: true,
      data: {
        current_plan_id: 'pro',
        plans: [
          {
            plan_id: 'free',
            name: 'Free',
            description: 'Starter',
            price_label: 'Free',
            interval: 'month',
            checkout_available: false,
            current: false,
            quotas: [
              { metric_type: 'pages_processed', hard_limit: 100 },
              { metric_type: 'storage_bytes', hard_limit: 1073741824 },
            ],
          },
          {
            plan_id: 'pro',
            name: 'Pro',
            description: 'For active teams',
            price_label: '$20/month',
            interval: 'month',
            checkout_available: true,
            current: true,
            quotas: [
              { metric_type: 'pages_processed', hard_limit: 1000 },
              { metric_type: 'storage_bytes', hard_limit: 10737418240 },
            ],
          },
        ],
      },
    });
    getSubscriptionMock.mockResolvedValue({
      success: true,
      data: {
        id: 'sub-1',
        org_id: 'org-1',
        plan_id: 'pro',
        status: 'active',
        current_period_end: '2026-04-01T00:00:00Z',
        cancel_at_period_end: false,
      },
    });
    getUsageMock.mockResolvedValue({
      success: true,
      data: {
        pages_processed: 120,
        embedding_tokens: 2400,
        llm_input_tokens: 3600,
        llm_output_tokens: 1800,
        storage_bytes: 1073741824,
      },
    });

    render(<BillingPanel onBack={vi.fn()} />);

    expect(await screen.findAllByText('Pro')).toHaveLength(2);
    expect(screen.getByText('billing.manageBilling')).toBeTruthy();
    expect(screen.getByText('billing.metricStorage')).toBeTruthy();
    expect(screen.getByText('1.00 GB')).toBeTruthy();
    expect(screen.getByText('billing.status: billing.statusActive')).toBeTruthy();
  });

  it('creates a checkout session and redirects for upgrade', async () => {
    const user = userEvent.setup();

    listPlansMock.mockResolvedValue({
      success: true,
      data: {
        current_plan_id: 'free',
        plans: [
          {
            plan_id: 'free',
            name: 'Free',
            description: 'Starter',
            price_label: 'Free',
            interval: 'month',
            checkout_available: false,
            current: true,
            quotas: [],
          },
          {
            plan_id: 'pro',
            name: 'Pro',
            description: 'For active teams',
            price_label: '$20/month',
            interval: 'month',
            checkout_available: true,
            current: false,
            quotas: [],
          },
        ],
      },
    });
    getSubscriptionMock.mockResolvedValue({
      success: true,
      data: {
        id: '',
        org_id: 'org-1',
        plan_id: 'free',
        status: 'active',
        cancel_at_period_end: false,
      },
    });
    getUsageMock.mockResolvedValue({
      success: true,
      data: {
        pages_processed: 2,
        embedding_tokens: 4,
        llm_input_tokens: 6,
        llm_output_tokens: 8,
        storage_bytes: 1024,
      },
    });
    createCheckoutSessionMock.mockResolvedValue({
      success: true,
      data: {
        url: 'https://billing.example.test/checkout/pro',
        session_id: 'cs_pro',
      },
    });

    render(<BillingPanel onBack={vi.fn()} />);

    await user.click(await screen.findByRole('button', { name: 'billing.upgrade' }));

    await waitFor(() => {
      expect(createCheckoutSessionMock).toHaveBeenCalledWith('pro');
      expect(assignMock).toHaveBeenCalledWith('https://billing.example.test/checkout/pro');
    });
  });

  it('creates a portal session for the current paid plan', async () => {
    const user = userEvent.setup();

    listPlansMock.mockResolvedValue({
      success: true,
      data: {
        current_plan_id: 'pro',
        plans: [
          {
            plan_id: 'pro',
            name: 'Pro',
            description: 'For active teams',
            price_label: '$20/month',
            interval: 'month',
            checkout_available: true,
            current: true,
            quotas: [],
          },
        ],
      },
    });
    getSubscriptionMock.mockResolvedValue({
      success: true,
      data: {
        id: 'sub-1',
        org_id: 'org-1',
        plan_id: 'pro',
        status: 'active',
        cancel_at_period_end: false,
      },
    });
    getUsageMock.mockResolvedValue({
      success: true,
      data: {
        pages_processed: 3,
        embedding_tokens: 5,
        llm_input_tokens: 7,
        llm_output_tokens: 9,
        storage_bytes: 2048,
      },
    });
    createPortalSessionMock.mockResolvedValue({
      success: true,
      data: {
        url: 'https://billing.example.test/portal',
      },
    });

    render(<BillingPanel onBack={vi.fn()} />);

    await user.click(await screen.findByRole('button', { name: 'billing.manageBilling' }));

    await waitFor(() => {
      expect(createPortalSessionMock).toHaveBeenCalledOnce();
      expect(assignMock).toHaveBeenCalledWith('https://billing.example.test/portal');
    });
  });

  it('shows an inline error card when billing load fails', async () => {
    listPlansMock.mockResolvedValue({
      success: false,
      error: 'boom',
    });
    getSubscriptionMock.mockResolvedValue({
      success: true,
      data: {
        id: '',
        org_id: 'org-1',
        plan_id: 'free',
        status: 'active',
        cancel_at_period_end: false,
      },
    });
    getUsageMock.mockResolvedValue({
      success: true,
      data: {
        pages_processed: 0,
        embedding_tokens: 0,
        llm_input_tokens: 0,
        llm_output_tokens: 0,
        storage_bytes: 0,
      },
    });

    render(<BillingPanel onBack={vi.fn()} />);

    expect(await screen.findByText('billing.loadFailed')).toBeTruthy();
    expect(screen.getByText('boom')).toBeTruthy();
  });
});
