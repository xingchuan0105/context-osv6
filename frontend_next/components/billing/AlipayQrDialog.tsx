"use client";

import { useEffect, useRef, useState } from "react";
import QRCode from "qrcode";
import styles from "./AlipayQrDialog.module.css";
import { getBillingOrderStatus } from "../../lib/settings/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";

export type AlipayQrDialogProps = {
  token: string;
  qrCode: string;
  orderId: string;
  planName: string;
  priceLabel: string;
  locale: UiLocale;
  onPaid: () => void;
  onCancel: () => void;
};

type PaymentState = "waiting" | "paid" | "timeout" | "cancelled";

const POLL_INTERVAL_MS = 2_000;
const POLL_TIMEOUT_MS = 5 * 60_000;

export function AlipayQrDialog({
  token,
  qrCode,
  orderId,
  planName,
  priceLabel,
  locale,
  onPaid,
  onCancel,
}: AlipayQrDialogProps) {
  const [qrImage, setQrImage] = useState("");
  const [state, setState] = useState<PaymentState>("waiting");
  const callbacksRef = useRef({ onPaid, onCancel });
  callbacksRef.current = { onPaid, onCancel };

  useEffect(() => {
    let cancelled = false;

    QRCode.toDataURL(qrCode, { width: 220, margin: 1 })
      .then((dataUrl) => {
        if (!cancelled) {
          setQrImage(dataUrl);
        }
      })
      .catch(() => {
        // Leave the placeholder visible if QR rendering fails.
      });

    return () => {
      cancelled = true;
    };
  }, [qrCode]);

  useEffect(() => {
    if (state !== "waiting") {
      return;
    }

    const startedAt = Date.now();
    const intervalId = setInterval(() => {
      void (async () => {
        if (Date.now() - startedAt >= POLL_TIMEOUT_MS) {
          setState("timeout");
          return;
        }

        try {
          const order = await getBillingOrderStatus(token, orderId);
          if (order.status === "paid") {
            setState("paid");
            callbacksRef.current.onPaid();
          }
        } catch {
          // Keep polling on transient errors until the timeout hits.
        }
      })();
    }, POLL_INTERVAL_MS);

    return () => clearInterval(intervalId);
  }, [state, token, orderId]);

  function handleCancel() {
    setState("cancelled");
    callbacksRef.current.onCancel();
  }

  return (
    <div className={styles.overlay}>
      <div
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-label={formatUiMessage(locale, "alipayQrTitle")}
      >
        <h2 className={styles.title}>{formatUiMessage(locale, "alipayQrTitle")}</h2>
        <p className={styles.planLine}>
          {planName}
          {priceLabel ? ` · ${priceLabel}` : ""}
        </p>
        {state === "timeout" ? (
          <p className={styles.timeoutText} role="alert">
            {formatUiMessage(locale, "alipayQrTimeout")}
          </p>
        ) : (
          <>
            {qrImage ? (
              <img
                className={styles.qrImage}
                src={qrImage}
                alt={formatUiMessage(locale, "alipayQrTitle")}
              />
            ) : (
              <div className={styles.qrPlaceholder} aria-hidden="true" />
            )}
            <p className={styles.hint}>{formatUiMessage(locale, "alipayQrScanHint")}</p>
            <p className={styles.status} role="status">
              {state === "paid"
                ? formatUiMessage(locale, "alipayQrPaid")
                : formatUiMessage(locale, "alipayQrWaiting")}
            </p>
          </>
        )}
        {state !== "paid" ? (
          <button type="button" className={styles.cancelButton} onClick={handleCancel}>
            {formatUiMessage(locale, "alipayQrCancel")}
          </button>
        ) : null}
      </div>
    </div>
  );
}
