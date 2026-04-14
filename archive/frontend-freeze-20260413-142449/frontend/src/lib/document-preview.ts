type PreviewFailure = {
  error?: string;
  error_code?: string;
};

export function getDocumentPreviewErrorMessage(failure?: PreviewFailure | null): string {
  if (failure?.error_code === 'RATE_LIMITED') {
    return '预览请求过于频繁，请稍后再试';
  }
  return failure?.error || '无法获取解析文本';
}

export function shouldAttemptParsedPreviewFallback(failure?: PreviewFailure | null): boolean {
  return failure?.error_code !== 'RATE_LIMITED';
}
