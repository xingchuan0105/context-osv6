const SUPPORTED_UPLOAD_EXTENSIONS = [
  '.pdf',
  '.txt',
  '.md',
  '.markdown',
  '.docx',
  '.html',
  '.htm',
  '.xlsx',
  '.xls',
] as const;

const SUPPORTED_UPLOAD_EXTENSION_SET = new Set<string>(SUPPORTED_UPLOAD_EXTENSIONS);

export const SUPPORTED_UPLOAD_ACCEPT = SUPPORTED_UPLOAD_EXTENSIONS.join(',');

export function getUploadFileExtension(fileName: string): string {
  const trimmed = fileName.trim();
  const index = trimmed.lastIndexOf('.');
  if (index < 0) {
    return '';
  }
  return trimmed.slice(index).toLowerCase();
}

export function isSupportedUploadFileName(fileName: string): boolean {
  const extension = getUploadFileExtension(fileName);
  if (!extension) {
    return false;
  }
  return SUPPORTED_UPLOAD_EXTENSION_SET.has(extension);
}

export function partitionSupportedUploadFiles<T extends { name: string }>(files: T[]): {
  supported: T[];
  unsupported: T[];
} {
  const supported: T[] = [];
  const unsupported: T[] = [];

  for (const file of files) {
    if (isSupportedUploadFileName(file.name)) {
      supported.push(file);
      continue;
    }
    unsupported.push(file);
  }

  return {
    supported,
    unsupported,
  };
}
