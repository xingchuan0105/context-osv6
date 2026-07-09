use avrag_billing::{BillingEvent, BillingProvider, Subscription, SubscriptionStatus};
use chrono::{TimeZone, Utc};

fn sample_subscription(status: SubscriptionStatus) -> Subscription {
    Subscription {
        id: "sub-1".to_string(),
        user_id: "user-1".to_string(),
        stripe_subscription_id: None,
        stripe_price_id: None,
        billing_provider: BillingProvider::Stripe,
        provider_subscription_id: None,
        provider_price_id: None,
        plan_id: "plus".to_string(),
        status,
        current_period_start: Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
        current_period_end: Some(Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap()),
        cancel_at_period_end: false,
        created_at: None,
        updated_at: None,
    }
}

#[test]
fn subscription_payment_failed_moves_active_to_past_due() {
    let subscription = sample_subscription(SubscriptionStatus::Active);

    let next = subscription
        .apply_transition(&BillingEvent::PaymentFailed)
        .expect("active subscriptions should enter past_due on payment failure");

    assert_eq!(next, SubscriptionStatus::PastDue);
}

#[test]
fn subscription_invoice_paid_rejects_earlier_period_end() {
    let subscription = sample_subscription(SubscriptionStatus::Active);
    let earlier = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();

    let error = subscription
        .apply_transition(&BillingEvent::InvoicePaid {
            new_period_end: Some(earlier),
        })
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("new_period_end cannot be earlier")
    );
}
