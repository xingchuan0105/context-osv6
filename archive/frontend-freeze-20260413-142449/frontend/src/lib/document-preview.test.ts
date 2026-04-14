import { describe, expect, it } from 'vitest';
import {
  getDocumentPreviewErrorMessage,
  shouldAttemptParsedPreviewFallback,
} from './document-preview';

describe('document-preview', () => {
  it('surfaces a friendly message for rate-limited preview requests', () => {
    expect(
      getDocumentPreviewErrorMessage({
        error: 'Request failed with status code 429',
        error_code: 'RATE_LIMITED',
      })
    ).toBe('预览请求过于频繁，请稍后再试');
  });

  it('prevents fallback fan-out when preview is rate limited', () => {
    expect(
      shouldAttemptParsedPreviewFallback({
        error: 'Request failed with status code 429',
        error_code: 'RATE_LIMITED',
      })
    ).toBe(false);
  });

  it('allows fallback for ordinary preview failures', () => {
    expect(
      shouldAttemptParsedPreviewFallback({
        error: 'document parsed preview failed',
      })
    ).toBe(true);
  });
});
