import type { UiLocale } from "../../../lib/ui-preferences";
import { INLINE_COPY, type AdminCopyKey } from "./copy";

export type { AdminCopyKey };

/** Single admin copy seam — always resolves from INLINE_COPY (not UI_MESSAGES). */
export function adminText(locale: UiLocale, key: AdminCopyKey) {
  const copy = INLINE_COPY[key];
  return locale === "zh-CN" ? copy.zh : copy.en;
}

export function formatAdminError(locale: UiLocale, error: unknown) {
  const detail = error instanceof Error ? error.message : String(error);
  return `${adminText(locale, "admin.loadError")} ${detail}`;
}

export function planLabel(locale: UiLocale, plan: string) {
  const normalizedPlan = plan.trim().toLowerCase();

  switch (normalizedPlan) {
    case "":
    case "n/a":
    case "unknown":
      return locale === "zh-CN" ? "未配置" : "Unset";
    case "free":
      return locale === "zh-CN" ? "免费版" : "Free";
    case "plus":
    case "starter":
    case "team":
    case "enterprise":
      return "Plus";
    case "pro":
      return locale === "zh-CN" ? "专业版" : "Pro";
    default:
      return plan;
  }
}

export function accountStatusLabel(locale: UiLocale, blocked: boolean) {
  return blocked
    ? adminText(locale, "admin.status.blocked")
    : adminText(locale, "admin.status.active");
}

export function userRoleLabel(locale: UiLocale, role: string) {
  switch (role) {
    case "owner":
      return locale === "zh-CN" ? "所有者" : "Owner";
    case "admin":
      return locale === "zh-CN" ? "管理员" : "Admin";
    case "member":
      return locale === "zh-CN" ? "成员" : "Member";
    case "viewer":
      return locale === "zh-CN" ? "查看者" : "Viewer";
    case "editor":
      return locale === "zh-CN" ? "编辑者" : "Editor";
    default:
      return role;
  }
}

export function healthStatusLabel(locale: UiLocale, status: string) {
  switch (status) {
    case "ok":
    case "healthy":
    case "ready":
      return adminText(locale, "admin.status.healthy");
    case "degraded":
      return locale === "zh-CN" ? "降级中" : "Degraded";
    case "error":
    case "failed":
    case "unhealthy":
      return locale === "zh-CN" ? "异常" : "Unhealthy";
    default:
      return status;
  }
}

export function featureFlagStatusLabel(locale: UiLocale, status: string) {
  switch (status) {
    case "pending":
      return locale === "zh-CN" ? "待处理" : "Pending";
    case "approved":
      return locale === "zh-CN" ? "已批准" : "Approved";
    case "rejected":
      return locale === "zh-CN" ? "已拒绝" : "Rejected";
    case "executed":
      return locale === "zh-CN" ? "已执行" : "Executed";
    default:
      return status;
  }
}

export function featureFlagCategoryLabel(locale: UiLocale, category: string) {
  switch (category) {
    case "chat":
      return locale === "zh-CN" ? "对话" : "Chat";
    case "retrieval":
    case "rag":
      return locale === "zh-CN" ? "检索" : "Retrieval";
    case "ingestion":
      return locale === "zh-CN" ? "入库" : "Ingestion";
    case "guard":
    case "guardrail":
    case "guardrails":
    case "safety":
      return locale === "zh-CN" ? "安全护栏" : "Guardrails";
    case "share":
    case "sharing":
      return locale === "zh-CN" ? "分享" : "Sharing";
    case "admin":
      return locale === "zh-CN" ? "后台" : "Admin";
    case "system":
      return locale === "zh-CN" ? "系统" : "System";
    case "billing":
      return locale === "zh-CN" ? "账单" : "Billing";
    default:
      return humanizeIdentifier(category);
  }
}

export function featureFlagSourceLabel(locale: UiLocale, source: string) {
  switch (source) {
    case "seed":
    case "seeded":
      return locale === "zh-CN" ? "初始种子" : "Seeded";
    case "env":
    case "environment":
      return locale === "zh-CN" ? "环境变量" : "Environment";
    case "config":
      return locale === "zh-CN" ? "配置文件" : "Config";
    case "db":
    case "database":
    case "postgres":
    case "postgresql":
      return locale === "zh-CN" ? "数据库" : "Database";
    case "request":
    case "change_request":
      return locale === "zh-CN" ? "变更请求" : "Change request";
    case "override":
    case "manual_override":
    case "emergency_override":
      return locale === "zh-CN" ? "紧急覆盖" : "Emergency override";
    case "runtime":
      return locale === "zh-CN" ? "运行时" : "Runtime";
    default:
      return humanizeIdentifier(source);
  }
}

export function workerRuntimeLabel(locale: UiLocale, runtime: string) {
  switch (runtime) {
    case "inline":
      return locale === "zh-CN" ? "内联" : "Inline";
    case "queue":
    case "queued":
      return locale === "zh-CN" ? "队列" : "Queued";
    case "worker":
      return locale === "zh-CN" ? "执行器" : "Worker";
    default:
      return runtime;
  }
}

export function auditActionLabel(locale: UiLocale, action: string) {
  switch (action) {
    case "task_enqueued":
      return locale === "zh-CN" ? "任务已入队" : "Task enqueued";
    case "task_started":
      return locale === "zh-CN" ? "任务开始执行" : "Task started";
    case "task_completed":
      return locale === "zh-CN" ? "任务执行完成" : "Task completed";
    case "task_failed":
      return locale === "zh-CN" ? "任务执行失败" : "Task failed";
    case "state_transition":
      return locale === "zh-CN" ? "状态迁移" : "State transition";
    case "input_guard_block":
      return locale === "zh-CN" ? "输入被 Guard 拦截" : "Input guard blocked";
    case "output_guard_block":
      return locale === "zh-CN" ? "输出被 Guard 拦截" : "Output guard blocked";
    case "output_guard_redact":
      return locale === "zh-CN" ? "输出被 Guard 脱敏" : "Output guard redacted";
    case "share_access":
      return locale === "zh-CN" ? "分享访问" : "Share access";
    default:
      return humanizeIdentifier(action);
  }
}

export function auditResourceTypeLabel(locale: UiLocale, resourceType: string) {
  switch (resourceType) {
    case "document":
      return locale === "zh-CN" ? "文档" : "Document";
    case "notebook":
      return locale === "zh-CN" ? "知识库" : "Workspace";
    case "task":
      return locale === "zh-CN" ? "任务" : "Task";
    case "share":
      return locale === "zh-CN" ? "分享" : "Share";
    case "guard":
      return locale === "zh-CN" ? "护栏" : "Guard";
    default:
      return humanizeIdentifier(resourceType);
  }
}

function humanizeIdentifier(value: string) {
  return value
    .split(/[_\-\s]+/)
    .filter(Boolean)
    .map((part) => part[0]?.toUpperCase() + part.slice(1).toLowerCase())
    .join(" ");
}
