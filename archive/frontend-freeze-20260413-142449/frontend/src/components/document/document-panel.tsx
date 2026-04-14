'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { Upload, FileText, Trash2, CheckCircle, XCircle, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { documentsApi, pollDocumentStatus } from '@/lib/api/client';
import { partitionSupportedUploadFiles, SUPPORTED_UPLOAD_ACCEPT } from '@/lib/upload-file-validation';
import { useAppStore } from '@/stores/useAppStore';
import type { Document } from '@/types';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { toast } from '@/components/ui/toaster';
import { DocumentViewer } from './document-viewer';

export function DocumentPanel() {
  const { t } = useTranslation();
  const { currentWorkspace } = useAppStore();
  const [documents, setDocuments] = useState<Document[]>([]);
  const [loading, setLoading] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [selectedDocument, setSelectedDocument] = useState<Document | null>(null);
  const [viewerOpen, setViewerOpen] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Load documents
  const loadDocuments = useCallback(async () => {
    if (!currentWorkspace) return;

    setLoading(true);
    try {
      const response = await documentsApi.list(currentWorkspace.id);
      if (response.success) {
        setDocuments(response.data || []);
      } else {
        setDocuments([]);
        toast.error(response.error || t('document.refreshFailed'));
      }
    } catch (error) {
      console.error('Failed to load documents:', error);
      setDocuments([]);
      toast.error(t('document.refreshFailed'));
    } finally {
      setLoading(false);
    }
  }, [currentWorkspace, t]);

  // Handle file upload
  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;
    const fileArray = Array.from(files);
    const { supported, unsupported } = partitionSupportedUploadFiles(fileArray);

    if (unsupported.length > 0) {
      const unsupportedNames = unsupported.map((file) => file.name).join(', ');
      toast.error(t('document.unsupportedFileType', { files: unsupportedNames }));
    }
    if (supported.length === 0) {
      e.target.value = '';
      return;
    }

    if (!currentWorkspace) {
      toast.error(t('workspace.select'));
      return;
    }

    setUploading(true);
    try {
      for (const file of supported) {
        const response = await documentsApi.upload(currentWorkspace.id, file);
        if (response.success) {
          const docId = response.data?.id || response.data?.document_id;
          if (docId) {
            try {
              const finalStatus = await pollDocumentStatus(docId);
              if (finalStatus.status === 'failed') {
                toast.error(t('document.processingFailed'));
              }
            } catch (pollError) {
              // Polling timeout - doc may still be processing
              console.warn('Document status polling timed out:', pollError);
            }
          }
          await loadDocuments();
          toast.success(t('document.submitted', { count: 1 }));
        } else {
          toast.error(response.error || t('errors.serverError'));
        }
      }
    } catch (error) {
      console.error('Failed to upload:', error);
      toast.error(t('errors.networkError'));
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  // Handle delete
  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!window.confirm(t('document.deleteConfirm'))) return;
    
    try {
      const response = await documentsApi.delete(id, currentWorkspace?.id);
      if (response.success) {
        await loadDocuments();
        toast.success(t('document.deleted'));
      } else {
        toast.error(response.error || t('errors.serverError'));
      }
    } catch (error) {
      console.error('Failed to delete:', error);
      toast.error(t('errors.networkError'));
    }
  };

  // Handle preview
  const handlePreview = (doc: Document) => {
    if (doc.status === 'completed') {
      setSelectedDocument(doc);
      setViewerOpen(true);
    }
  };

  // Get status icon
  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle className="w-4 h-4 text-green-400" />;
      case 'processing':
        return <Loader2 className="w-4 h-4 text-yellow-400 animate-spin" />;
      case 'failed':
        return <XCircle className="w-4 h-4 text-red-400" />;
      default:
        return <Loader2 className="w-4 h-4 text-muted-foreground animate-pulse" />;
    }
  };

  // Load when workspace changes
  useEffect(() => {
    if (currentWorkspace) {
      void loadDocuments();
    }
  }, [currentWorkspace, loadDocuments]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border">
        <h2 className="text-sm font-medium text-foreground/80">{t('document.upload')}</h2>
        <div>
          <input
            ref={fileInputRef}
            type="file"
            multiple
            accept={SUPPORTED_UPLOAD_ACCEPT}
            onChange={handleUpload}
            className="hidden"
          />
          <Button
            variant="ghost"
            size="sm"
            onClick={() => fileInputRef.current?.click()}
            disabled={uploading || !currentWorkspace}
          >
            {uploading ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : (
              <Upload className="w-4 h-4" />
            )}
          </Button>
        </div>
      </div>

      {/* Document List */}
      <div className="flex-1 overflow-auto p-4">
        {!currentWorkspace ? (
          <div className="text-center text-muted-foreground/80 py-8">
            <p>{t('workspace.select')}</p>
          </div>
        ) : loading ? (
          <div className="text-center text-muted-foreground/80 py-8">{t('common.loading')}</div>
        ) : documents.length === 0 ? (
          <div className="text-center text-muted-foreground/80 py-8">
            <Upload className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>{t('document.noDocuments')}</p>
            <p className="text-sm">{t('document.uploadHint')}</p>
          </div>
        ) : (
          <div className="space-y-2">
            {documents.map((doc) => (
              <Card 
                key={doc.id} 
                className={`hover:border-border/80 transition-colors cursor-pointer ${doc.status !== 'completed' ? 'opacity-60' : ''}`}
                onClick={() => handlePreview(doc)}
              >
                <CardContent className="p-3 flex items-center justify-between">
                  <div className="flex items-center gap-2 min-w-0">
                    {getStatusIcon(doc.status)}
                    <FileText className="w-4 h-4 text-blue-400 shrink-0" />
                    <span className="text-sm truncate">{doc.file_name}</span>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={(e) => handleDelete(doc.id, e)}
                    className="h-7 w-7 p-0 text-muted-foreground/80 hover:text-red-400 shrink-0"
                  >
                    <Trash2 className="w-3 h-3" />
                  </Button>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>

      {/* Document Viewer */}
      <DocumentViewer
        document={selectedDocument}
        open={viewerOpen}
        onClose={() => {
          setViewerOpen(false);
          setSelectedDocument(null);
        }}
      />
    </div>
  );
}
