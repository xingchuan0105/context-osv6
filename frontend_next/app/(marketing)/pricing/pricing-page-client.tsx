"use client";

import { useRouter } from "next/navigation";

import { PricingCards } from "../../../components/billing/PricingCards";
import { createCheckoutSession } from "../../../lib/settings/client";
import { useAuth } from "../../../lib/auth/context";
import type { BillingPlan } from "../../../lib/billing/api";
import styles from "./pricing.module.css";

export function PricingPageClient({ plans }: { plans: BillingPlan[] }) {
  const auth = useAuth();
  const router = useRouter();

  async function handleSelect(planId: string) {
    if (planId === "free" || !auth.token) {
      return;
    }

    const checkout = await createCheckoutSession(auth.token, { plan_id: planId });
    if (checkout.url) {
      router.push(checkout.url);
    }
  }

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>选择适合你的方案</h1>
        <div className={styles.billingToggle}>
          <button type="button" className={`${styles.toggleButton} ${styles.toggleActive}`}>
            月付
          </button>
          <span className={styles.toggleHint} title="年付即将推出">
            年付暂未开放
          </span>
        </div>
      </header>

      <PricingCards plans={plans} highlightTier="plus" onSelect={handleSelect} />

      <section className={styles.faq}>
        <h2 className={styles.faqTitle}>❓ 常见问题</h2>
        <details className={styles.faqItem}>
          <summary>token 用量怎么算？</summary>
          <p>输入 + 输出按 DeepSeek 公开计费标准累计。每次问题消耗 = (input tokens + output tokens)。</p>
        </details>
        <details className={styles.faqItem}>
          <summary>限额会重置吗？</summary>
          <p>5 小时滚动窗口 + 7 天滚动窗口。窗口内最旧的消耗点过去后，限额自动释放。</p>
        </details>
        <details className={styles.faqItem}>
          <summary>升级后立即生效吗？</summary>
          <p>支付成功后立即生效。降级则在当前计费周期结束时生效。</p>
        </details>
      </section>
    </div>
  );
}
