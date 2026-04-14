export interface User {
  id: string;
  email: string;
  full_name?: string;
  avatar_url?: string;
}

export interface KnowledgeBase {
  id: string;
  user_id: string;
  title: string;
  icon?: string;
  description?: string;
  created_at: string;
}

// Rust API and PRD use "Notebook" as the canonical backend term.
// Keep the existing KnowledgeBase shape as a transitional UI alias.
export type Notebook = KnowledgeBase;

export interface NotebookAPIKey {
  id: string;
  org_id: string;
  notebook_id?: string;
  key_prefix: string;
  name: string;
  permissions: string[];
  rate_limit_rpm: number;
  expires_at?: string;
  last_used_at?: string;
  is_active: boolean;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface Notification {
  id: string;
  org_id: string;
  user_id: string;
  event_type: string;
  title: string;
  body: string;
  data: Record<string, unknown>;
  read_at?: string;
  created_at: string;
  updated_at: string;
}

export type BillingUsageMetric =
  | 'pages_processed'
  | 'embedding_tokens'
  | 'llm_input_tokens'
  | 'llm_output_tokens'
  | 'storage_bytes';

export type BillingUsage = Record<BillingUsageMetric, number>;

export interface BillingPlanQuota {
  metric_type: BillingUsageMetric;
  soft_limit?: number;
  hard_limit?: number;
}

export interface BillingPlan {
  plan_id: string;
  name: string;
  description: string;
  price_label: string;
  interval: string;
  checkout_available: boolean;
  current: boolean;
  quotas: BillingPlanQuota[];
}

export interface BillingSubscription {
  id: string;
  org_id: string;
  stripe_subscription_id?: string;
  stripe_price_id?: string;
  plan_id: string;
  status: string;
  current_period_start?: string;
  current_period_end?: string;
  cancel_at_period_end: boolean;
  created_at?: string;
  updated_at?: string;
}

export interface FavoriteKnowledgeBase extends KnowledgeBase {
  is_favorite: true;
  share_token: string;
  share_url?: string;
  favorite_id?: string;
  favorite_alias?: string;
  origin_title?: string;
  share_permission?: 'full' | 'partial';
  share_expires_at?: string | null;
  favorited_at?: string;
}

export type FavoriteNotebook = FavoriteKnowledgeBase;

export interface ShareTokenInfo {
  token: string;
  access_level: string;
  expires_at?: string;
  revoked_at?: string;
  access_count: number;
}

export interface ShareMember {
  id: string;
  notebook_id: string;
  user_id?: string;
  email?: string;
  access_level: string;
  invite_status: string;
  invited_by?: string;
  invited_at: number;
  accepted_at?: number;
}

export interface ShareSettings {
  access_level: string;
  share_tokens: ShareTokenInfo[];
  members: ShareMember[];
}

export interface Note {
  id: string;
  // Transitional field name. Backend contract is notebook_id.
  kb_id: string;
  user_id: string;
  title?: string;
  content: string;
  note_type?: 'draft' | 'committed';
  is_shared: boolean;
  created_at: string;
  updated_at: string;
}

export interface ChatSession {
  id: string;
  // Transitional field name. Backend contract is notebook_id.
  kb_id: string;
  user_id: string;
  title?: string;
  summary?: string;
  source_type?: 'owner' | 'share' | 'favorite';
  source_token?: string;
  created_at: string;
  updated_at: string;
}

export interface ChatMessage {
  id: number;
  session_id: string;
  role: 'user' | 'assistant';
  content: string;
  agent_id?: string;
  agent_name?: string;
  agent_icon?: string;
  citations?: Citation[];
  created_at: string;
}

export interface CitationLookupScope {
  session_id: string;
  message_id: number;
}

export interface Citation {
  citation_id: number;
  doc_id: string;
  chunk_id?: string;
  page?: number;
  doc_name: string;
  preview?: string;
  content?: string;
  score: number;
  layer?: string;
  chunk_type?: string;
  asset_id?: string;
  caption?: string;
  image_url?: string;
  lookup_scope?: CitationLookupScope;
}

export interface CitationLookupResponse {
  doc_name?: string;
  content?: string;
  doc_id?: string;
  chunk_id?: string;
  page?: number;
  chunk_type?: string;
  asset_id?: string;
  caption?: string;
  image_url?: string;
}

export interface RAGTraceItem {
  priority: number;
  item_type: string;
  retrieval_mode: string;
  purpose: string;
  query?: string;
  recall_budget: number;
  bm25_k: number;
  dense_k: number;
  rerank_budget: number;
  source_count: number;
  source_ids?: string[];
}

export interface RAGTraceSummary {
  item_count: number;
  total_candidate_budget?: number;
  max_rerank_docs?: number;
  max_final_chunks?: number;
  top_k_returned?: number;
  summary_mode?: string;
  items: RAGTraceItem[];
}

export interface Agent {
  id: string;
  name: string;
  description: string;
  icon: string;
}

export interface Document {
  id: string;
  // Transitional field name. Backend contract is notebook_id.
  kb_id: string;
  user_id: string;
  file_name: string;
  storage_path?: string;
  mime_type?: string;
  file_size?: number;
  status: 'pending' | 'enqueueing' | 'queued' | 'processing' | 'completed' | 'failed';
  summary_global?: string;
  content?: string;
  chunk_count: number;
  created_at: string;
}

export interface SearchResult {
  id: string;
  title: string;
  parent_id?: string;
  // Search responses may still return workspace_id from the legacy UI layer.
  workspace_id?: string;
  score: number;
  created?: string;
  updated?: string;
  source_type?: string;
  summary?: string;
  content?: string;
}

export interface AuthResponse {
  success: boolean;
  data?: {
    token: string;
    user: User;
    reset_ticket?: string; // For password reset flow (M5)
  };
  error?: string;
  error_code?: string;
  message?: string;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}
