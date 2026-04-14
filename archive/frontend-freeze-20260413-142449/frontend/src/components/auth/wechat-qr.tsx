'use client';

import { useEffect, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { authApi } from '@/lib/api/client';
import { Loader2, QrCode, CheckCircle2, AlertCircle } from 'lucide-react';

interface WechatQRProps {
  loginId: string;
  qrUrl?: string;
  onSuccess: (token: string) => void;
  onNeedBind: (bindTicket: string) => void;
  onError: (error: string) => void;
}

// QR status types
type QRStatus = 'pending' | 'authorized' | 'exchange' | 'success' | 'need_bind' | 'expired' | 'error';

const POLL_INTERVAL = 2000; // 2 seconds

export function WechatQR({ loginId, qrUrl, onSuccess, onNeedBind, onError }: WechatQRProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<QRStatus>('pending');
  const [error, setError] = useState<string | null>(null);
  const [countdown, setCountdown] = useState(120); // 2 minutes timeout
  const pollTimerRef = useRef<NodeJS.Timeout | null>(null);
  const countdownTimerRef = useRef<NodeJS.Timeout | null>(null);

  const clearTimers = useCallback(() => {
    if (pollTimerRef.current) {
      clearInterval(pollTimerRef.current);
      pollTimerRef.current = null;
    }
    if (countdownTimerRef.current) {
      clearInterval(countdownTimerRef.current);
      countdownTimerRef.current = null;
    }
  }, []);

  const handleError = useCallback((err: string) => {
    setError(err);
    setStatus('error');
    clearTimers();
    onError(err);
  }, [clearTimers, onError]);

  const pollStatus = useCallback(async () => {
    try {
      const response = await authApi.wechatGetStatus(loginId);
      
      if (!response.success || !response.data) {
        handleError(response.error || 'Failed to get status');
        return;
      }

      const currentStatus = response.data.status as QRStatus;
      setStatus(currentStatus);

      switch (currentStatus) {
        case 'pending':
          // Still waiting, continue polling
          break;

        case 'authorized':
        case 'exchange':
          // User confirmed, exchange for token
          if (!response.data.login_code) {
            handleError('Missing login code');
            return;
          }
          try {
            const exchangeResponse = await authApi.wechatExchange(loginId, response.data.login_code);
            if (exchangeResponse.success && exchangeResponse.data?.token) {
              setStatus('success');
              clearTimers();
              onSuccess(exchangeResponse.data.token);
            } else {
              handleError(exchangeResponse.error || 'Exchange failed');
            }
          } catch (err: any) {
            handleError(err.message || 'Exchange failed');
          }
          break;

        case 'need_bind':
          // User needs to bind an account
          if (!response.data.bind_ticket) {
            handleError('Missing bind ticket');
            return;
          }
          clearTimers();
          onNeedBind(response.data.bind_ticket);
          break;

        case 'expired':
          handleError('QR code expired');
          break;

        case 'error':
          handleError('Scan failed');
          break;

        default:
          // Unknown status
          console.warn('Unknown QR status:', currentStatus);
      }
    } catch (err: any) {
      console.error('Poll error:', err);
      // Don't treat network errors as fatal, just continue polling
    }
  }, [loginId, handleError, clearTimers, onSuccess, onNeedBind]);

  useEffect(() => {
    // Start polling
    pollTimerRef.current = setInterval(pollStatus, POLL_INTERVAL);

    // Countdown timer for QR expiration
    countdownTimerRef.current = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          handleError('QR code expired');
          return 0;
        }
        return prev - 1;
      });
    }, 1000);

    return () => {
      clearTimers();
    };
  }, [pollStatus, clearTimers, handleError]);

  // Render status UI
  const renderStatus = () => {
    switch (status) {
      case 'pending':
        return (
          <div className="flex flex-col items-center gap-3 py-4">
            <QrCode className="h-12 w-12 text-muted-foreground" />
            <p className="text-muted-foreground">{t('auth.waitingScan')}</p>
          </div>
        );

      case 'authorized':
      case 'exchange':
        return (
          <div className="flex flex-col items-center gap-3 py-4">
            <Loader2 className="h-12 w-12 animate-spin text-primary" />
            <p className="text-muted-foreground">{t('auth.scanSuccess')}</p>
          </div>
        );

      case 'success':
        return (
          <div className="flex flex-col items-center gap-3 py-4">
            <CheckCircle2 className="h-12 w-12 text-green-500" />
            <p className="text-green-500">{t('auth.exchangeSuccess')}</p>
          </div>
        );

      case 'error':
      case 'expired':
        return (
          <div className="flex flex-col items-center gap-3 py-4">
            <AlertCircle className="h-12 w-12 text-destructive" />
            <p className="text-destructive">{error || t('auth.exchangeFailed')}</p>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className="flex flex-col items-center gap-4">
      {/* QR Code Image */}
      <div className="relative flex items-center justify-center rounded-lg bg-white p-4">
        {qrUrl ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img 
            src={qrUrl} 
            alt="WeChat QR Code" 
            className="h-[200px] w-[200px]"
          />
        ) : (
          <div className="flex h-[200px] w-[200px] items-center justify-center border-2 border-dashed border-gray-300">
            <QrCode className="h-16 w-16 text-gray-400" />
          </div>
        )}
      </div>

      {/* Instructions */}
      <p className="text-center text-sm text-muted-foreground">
        {t('auth.scanQRCode')}
      </p>

      {/* Status */}
      {renderStatus()}

      {/* Countdown */}
      <p className="text-xs text-muted-foreground">
        {Math.floor(countdown / 60)}:{(countdown % 60).toString().padStart(2, '0')}
      </p>
    </div>
  );
}
