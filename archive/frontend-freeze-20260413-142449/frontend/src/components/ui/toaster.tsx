'use client';

import { useEffect, useState } from 'react';

type ToastVariant = 'success' | 'error' | 'info';

interface ToastPayload {
  message: string;
  variant?: ToastVariant;
}

interface ToastItem {
  id: number;
  message: string;
  variant: ToastVariant;
}

const TOAST_EVENT = 'context-os:toast';

function emitToast(payload: ToastPayload) {
  if (typeof window === 'undefined') {
    return;
  }
  window.dispatchEvent(new CustomEvent<ToastPayload>(TOAST_EVENT, { detail: payload }));
}

export const toast = {
  success: (message: string) => emitToast({ message, variant: 'success' }),
  error: (message: string) => emitToast({ message, variant: 'error' }),
  info: (message: string) => emitToast({ message, variant: 'info' }),
};

export function Toaster() {
  const [items, setItems] = useState<ToastItem[]>([]);

  useEffect(() => {
    const handleToast = (event: Event) => {
      const detail = (event as CustomEvent<ToastPayload>).detail;
      if (!detail?.message) {
        return;
      }

      const id = Date.now() + Math.floor(Math.random() * 1000);
      const next: ToastItem = {
        id,
        message: detail.message,
        variant: detail.variant || 'info',
      };

      setItems((prev) => [...prev, next]);
      window.setTimeout(() => {
        setItems((prev) => prev.filter((item) => item.id !== id));
      }, 3200);
    };

    window.addEventListener(TOAST_EVENT, handleToast);
    return () => window.removeEventListener(TOAST_EVENT, handleToast);
  }, []);

  return (
    <div className="fixed right-4 top-4 z-[100] flex max-w-sm flex-col gap-2">
      {items.map((item) => (
        <div
          key={item.id}
          className={`rounded-lg border px-3 py-2 text-sm shadow-lg backdrop-blur-sm ${
            item.variant === 'success'
              ? 'border-green-500/40 bg-green-500/10 text-green-300'
              : item.variant === 'error'
                ? 'border-red-500/40 bg-red-500/10 text-red-300'
                : 'border-indigo-500/40 bg-indigo-500/10 text-indigo-200'
          }`}
        >
          {item.message}
        </div>
      ))}
    </div>
  );
}
