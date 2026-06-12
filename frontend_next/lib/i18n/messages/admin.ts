import type { UiMessageDescriptor } from "./types";

export const adminMessages = {
  adminNavAuditLogs: {
    zh: "审计日志",
    en: "Audit Logs",
  },
  adminNavBilling: {
    zh: "账单",
    en: "Billing",
  },
  adminNavDegradation: {
    zh: "降级",
    en: "Degradation",
  },
  adminNavFeatureFlags: {
    zh: "功能开关",
    en: "Feature Flags",
  },
  adminNavHealth: {
    zh: "健康",
    en: "Health",
  },
  adminNavLabel: {
    zh: "后台导航",
    en: "Admin navigation",
  },
  adminNavOrganizations: {
    zh: "组织",
    en: "Organizations",
  },
  adminNavRagHealth: {
    zh: "RAG 健康",
    en: "RAG Health",
  },
  adminNavUsage: {
    zh: "用量",
    en: "Usage",
  },
  adminNavUsers: {
    zh: "用户",
    en: "Users",
  },
  adminNavWorkers: {
    zh: "执行器",
    en: "Workers",
  },
  adminShellTitle: {
    zh: "后台管理",
    en: "Admin",
  },
  "admin.shellTitle": {
    zh: "后台管理",
    en: "Admin",
  },
  "admin.navLabel": {
    zh: "后台导航",
    en: "Admin navigation",
  },
  "admin.nav.organizations": {
    zh: "组织",
    en: "Organizations",
  },
  "admin.nav.users": {
    zh: "用户",
    en: "Users",
  },
  "admin.nav.usage": {
    zh: "用量",
    en: "Usage",
  },
  "admin.nav.billing": {
    zh: "账单",
    en: "Billing",
  },
  "admin.nav.health": {
    zh: "健康",
    en: "Health",
  },
  "admin.nav.ragHealth": {
    zh: "RAG 健康",
    en: "RAG Health",
  },
  "admin.nav.featureFlags": {
    zh: "功能开关",
    en: "Feature flags",
  },
  "admin.nav.workers": {
    zh: "执行器",
    en: "Workers",
  },
  "admin.nav.degradation": {
    zh: "降级",
    en: "Degradation",
  },
  "admin.nav.auditLogs": {
    zh: "审计日志",
    en: "Audit logs",
  },
  "admin.pageSubtitle": {
    zh: "查看组织、用量、健康状态和系统级运营数据。",
    en: "Review organizations, usage, health, and system-wide operational signals.",
  },
  "admin.searchLabel": {
    zh: "搜索",
    en: "Search",
  },
  "admin.searchPlaceholder": {
    zh: "按名称、邮箱或资源 ID 筛选",
    en: "Filter by name, email, or resource ID",
  },
  "admin.filter.statusLabel": {
    zh: "状态",
    en: "Status",
  },
  "admin.filter.roleLabel": {
    zh: "角色",
    en: "Role",
  },
  "admin.filter.periodLabel": {
    zh: "周期",
    en: "Period",
  },
  "admin.filter.windowLabel": {
    zh: "时间窗口",
    en: "Time window",
  },
  "admin.filter.pageSizeLabel": {
    zh: "每页条数",
    en: "Rows per page",
  },
  "admin.filter.sortLabel": {
    zh: "排序",
    en: "Sort",
  },
  "admin.refreshAction": {
    zh: "刷新",
    en: "Refresh",
  },
  "admin.exportAction": {
    zh: "导出",
    en: "Export",
  },
  "admin.detailsAction": {
    zh: "查看详情",
    en: "View details",
  },
  "admin.blockAction": {
    zh: "封禁",
    en: "Block",
  },
  "admin.unblockAction": {
    zh: "解除封禁",
    en: "Unblock",
  },
  "admin.emptyTitle": {
    zh: "没有匹配结果",
    en: "No matching results",
  },
  "admin.emptyBody": {
    zh: "调整筛选条件后再试一次。",
    en: "Adjust the current filters and try again.",
  },
  "admin.loadError": {
    zh: "加载后台数据失败。",
    en: "Failed to load admin data.",
  },
  "admin.table.organization": {
    zh: "组织",
    en: "Organization",
  },
  "admin.table.plan": {
    zh: "计划",
    en: "Plan",
  },
  "admin.table.status": {
    zh: "状态",
    en: "Status",
  },
  "admin.table.users": {
    zh: "用户数",
    en: "Users",
  },
  "admin.table.requests": {
    zh: "请求数",
    en: "Requests",
  },
  "admin.table.createdAt": {
    zh: "创建时间",
    en: "Created at",
  },
  "admin.table.lastActive": {
    zh: "最近活跃",
    en: "Last active",
  },
  "admin.metrics.totalOrganizations": {
    zh: "组织总数",
    en: "Total organizations",
  },
  "admin.metrics.totalUsers": {
    zh: "用户总数",
    en: "Total users",
  },
  "admin.metrics.totalRequests": {
    zh: "请求总数",
    en: "Total requests",
  },
  "admin.metrics.totalDocuments": {
    zh: "文档总数",
    en: "Total documents",
  },
  "admin.health.sectionTitle": {
    zh: "系统健康",
    en: "System health",
  },
  "admin.health.sectionSubtitle": {
    zh: "检查服务状态、退化信号和恢复建议。",
    en: "Check service status, degradation signals, and recovery hints.",
  },
  "admin.billing.sectionTitle": {
    zh: "账单概览",
    en: "Billing overview",
  },
  "admin.billing.sectionSubtitle": {
    zh: "查看计划分布、收款状态和账单风险。",
    en: "Review plan mix, collection status, and billing risks.",
  },
  "admin.featureFlags.sectionTitle": {
    zh: "功能开关",
    en: "Feature flags",
  },
  "admin.featureFlags.sectionSubtitle": {
    zh: "管理开关状态、变更请求和审核流。",
    en: "Manage flag state, change requests, and review flow.",
  },
  "admin.auditLogs.sectionTitle": {
    zh: "审计日志",
    en: "Audit logs",
  },
  "admin.auditLogs.sectionSubtitle": {
    zh: "按动作、资源和执行者追踪后台操作。",
    en: "Trace admin activity by action, resource, and actor.",
  },
  "admin.workers.sectionTitle": {
    zh: "执行器状态",
    en: "Worker status",
  },
  "admin.workers.sectionSubtitle": {
    zh: "查看执行队列、处理能力和异常节点。",
    en: "Review queue health, capacity, and failing workers.",
  },
  "admin.degradation.sectionTitle": {
    zh: "降级状态",
    en: "Degradation status",
  },
  "admin.degradation.sectionSubtitle": {
    zh: "查看当前降级策略、触发原因和影响范围。",
    en: "Review active degradation policies, triggers, and blast radius.",
  },
  "admin.status.active": {
    zh: "正常",
    en: "Active",
  },
  "admin.status.blocked": {
    zh: "已封禁",
    en: "Blocked",
  },
  "admin.status.healthy": {
    zh: "健康",
    en: "Healthy",
  },
  "admin.status.degraded": {
    zh: "降级中",
    en: "Degraded",
  },
  "admin.status.unhealthy": {
    zh: "异常",
    en: "Unhealthy",
  },
} satisfies Record<string, UiMessageDescriptor>;
