import { describe, expect, it } from 'vitest';
import {
  getUploadFileExtension,
  isSupportedUploadFileName,
  partitionSupportedUploadFiles,
  SUPPORTED_UPLOAD_ACCEPT,
} from './upload-file-validation';

describe('upload-file-validation', () => {
  it('extracts normalized extension from file name', () => {
    expect(getUploadFileExtension('Report.PDF')).toBe('.pdf');
    expect(getUploadFileExtension('no-extension')).toBe('');
  });

  it('detects supported file names', () => {
    expect(isSupportedUploadFileName('a.pdf')).toBe(true);
    expect(isSupportedUploadFileName('a.docx')).toBe(true);
    expect(isSupportedUploadFileName('a.xlsx')).toBe(true);
    expect(isSupportedUploadFileName('a.doc')).toBe(false);
    expect(isSupportedUploadFileName('a.pptx')).toBe(false);
  });

  it('partitions supported and unsupported files', () => {
    const result = partitionSupportedUploadFiles([
      { name: 'one.pdf' },
      { name: 'two.doc' },
      { name: 'three.txt' },
      { name: 'four.pptx' },
    ]);

    expect(result.supported.map((file) => file.name)).toEqual(['one.pdf', 'three.txt']);
    expect(result.unsupported.map((file) => file.name)).toEqual(['two.doc', 'four.pptx']);
  });

  it('exports accept value for file input', () => {
    expect(SUPPORTED_UPLOAD_ACCEPT).toContain('.pdf');
    expect(SUPPORTED_UPLOAD_ACCEPT).toContain('.docx');
    expect(SUPPORTED_UPLOAD_ACCEPT).toContain('.xlsx');
    expect(SUPPORTED_UPLOAD_ACCEPT).not.toContain('.pptx');
  });
});
