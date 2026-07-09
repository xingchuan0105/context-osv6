type AdminInlineCopy = {
  en: string;
  zh: string;
};

const INLINE_COPY = {
  "common.actions": {
    zh: "操作",
    en: "Actions",
  },
  "common.actor": {
    zh: "执行者",
    en: "Actor",
  },
  "common.action": {
    zh: "动作",
    en: "Action",
  },
  "common.admins": {
    zh: "管理员",
    en: "Admins",
  },
  "common.allStatuses": {
    zh: "全部状态",
    en: "All statuses",
  },
  "common.back": {
    zh: "返回",
    en: "Back",
  },
  "common.config": {
    zh: "配置",
    en: "Config",
  },
  "common.created": {
    zh: "创建",
    en: "Created",
  },
  "common.current": {
    zh: "当前",
    en: "Current",
  },
  "common.currentView": {
    zh: "当前视图：",
    en: "Current view: ",
  },
  "common.documents30d": {
    zh: "文档（30d）",
    en: "Documents (30d)",
  },
  "common.email": {
    zh: "邮箱",
    en: "Email",
  },
  "common.emptyData": {
    zh: "暂时没有可显示的数据。",
    en: "No data is available yet.",
  },
  "common.failedDocs": {
    zh: "失败文档",
    en: "Failed docs",
  },
  "common.loading": {
    zh: "正在加载...",
    en: "Loading...",
  },
  "common.missing": {
    zh: "缺失",
    en: "Missing",
  },
  "common.name": {
    zh: "名称",
    en: "Name",
  },
  "common.never": {
    zh: "从未",
    en: "Never",
  },
  "common.neverActive": {
    zh: "从未活跃",
    en: "Never active",
  },
  "common.notebooks": {
    zh: "知识库",
    en: "Workspaces",
  },
  "common.owners": {
    zh: "所有者",
    en: "Owners",
  },
  "common.off": {
    zh: "关闭",
    en: "Off",
  },
  "common.on": {
    zh: "开启",
    en: "On",
  },
  "common.organizationId": {
    zh: "组织 ID",
    en: "Organization ID",
  },
  "common.page": {
    zh: "页码",
    en: "Page",
  },
  "common.pendingRequest": {
    zh: "有待处理请求",
    en: "Pending request",
  },
  "common.period7dRequests": {
    zh: "请求（7d）",
    en: "Requests (7d)",
  },
  "common.period30dRequests": {
    zh: "请求（30d）",
    en: "Requests (30d)",
  },
  "common.platformStatistics": {
    zh: "平台统计",
    en: "Platform statistics",
  },
  "common.processing": {
    zh: "处理中...",
    en: "Processing...",
  },
  "common.queued": {
    zh: "排队中",
    en: "Queued",
  },
  "common.queuedTasks": {
    zh: "排队任务",
    en: "Queued tasks",
  },
  "common.ready": {
    zh: "就绪",
    en: "Ready",
  },
  "common.requestsPerUser30d": {
    zh: "30 天人均请求",
    en: "Requests per user (30d)",
  },
  "common.resource": {
    zh: "资源",
    en: "Resource",
  },
  "common.resourceId": {
    zh: "资源 ID",
    en: "Resource ID",
  },
  "common.reviewedBy": {
    zh: "审核人：",
    en: "Reviewed by: ",
  },
  "common.runtime": {
    zh: "运行模式",
    en: "Runtime",
  },
  "common.scope": {
    zh: "范围",
    en: "Scope",
  },
  "common.selectOrganization": {
    zh: "选择组织",
    en: "Select an organization",
  },
  "common.service": {
    zh: "服务",
    en: "Service",
  },
  "common.serviceStatus": {
    zh: "系统状态",
    en: "System status",
  },
  "common.status": {
    zh: "状态",
    en: "Status",
  },
  "common.submitting": {
    zh: "提交中...",
    en: "Submitting...",
  },
  "common.time": {
    zh: "时间",
    en: "Time",
  },
  "common.timeWindow": {
    zh: "时间窗口：",
    en: "Time window: ",
  },
  "common.tokens30d": {
    zh: "令牌（30d）",
    en: "Tokens (30d)",
  },
  "common.totalFlags": {
    zh: "开关总数",
    en: "Total flags",
  },
  "common.totalIndexedDocuments": {
    zh: "已索引文档总数",
    en: "Total indexed documents",
  },
  "common.totalTokens": {
    zh: "总令牌数",
    en: "Total tokens",
  },
  "common.totalTokensProcessed": {
    zh: "已处理令牌总数",
    en: "Total tokens processed",
  },
  "common.updated": {
    zh: "更新于",
    en: "Updated",
  },
  "common.version": {
    zh: "版本",
    en: "Version",
  },
  "audit.allTime": {
    zh: "全部时间",
    en: "All time",
  },
  "audit.empty": {
    zh: "没有审计日志匹配当前筛选。",
    en: "No audit logs match the current filters.",
  },
  "audit.exportCsv": {
    zh: "导出 CSV",
    en: "Export CSV",
  },
  "audit.last24h": {
    zh: "最近 24 小时",
    en: "Last 24h",
  },
  "audit.last30d": {
    zh: "最近 30 天",
    en: "Last 30 days",
  },
  "audit.last7d": {
    zh: "最近 7 天",
    en: "Last 7 days",
  },
  "audit.last90d": {
    zh: "最近 90 天",
    en: "Last 90 days",
  },
  "audit.matchingLogs": {
    zh: "匹配日志",
    en: "Matching logs",
  },
  "audit.next": {
    zh: "下一页",
    en: "Next",
  },
  "audit.orgId": {
    zh: "组织 ID",
    en: "Org ID",
  },
  "audit.previous": {
    zh: "上一页",
    en: "Previous",
  },
  "billing.active": {
    zh: "活跃订阅",
    en: "Active",
  },
  "billing.canceled": {
    zh: "已取消",
    en: "Canceled",
  },
  "billing.pastDue": {
    zh: "逾期未付",
    en: "Past due",
  },
  "billing.unpaid": {
    zh: "未支付",
    en: "Unpaid",
  },
  "degradation.guardEvents24h": {
    zh: "Guard 事件（24h）",
    en: "Guard events (24h)",
  },
  "degradation.shareAccessEvents24h": {
    zh: "分享访问事件（24h）",
    en: "Share access events (24h)",
  },
  "featureFlags.allRequests": {
    zh: "全部请求",
    en: "All requests",
  },
  "featureFlags.approveExecute": {
    zh: "批准并执行",
    en: "Approve & execute",
  },
  "featureFlags.changeRequestsTitle": {
    zh: "变更请求",
    en: "Change requests",
  },
  "featureFlags.configBlockers": {
    zh: "配置阻塞",
    en: "Config blockers",
  },
  "featureFlags.desired": {
    zh: "期望：",
    en: "Desired: ",
  },
  "featureFlags.drift": {
    zh: "期望/生效漂移",
    en: "Desired/effective drift",
  },
  "featureFlags.effective": {
    zh: "生效：",
    en: "Effective: ",
  },
  "featureFlags.filterPlaceholder": {
    zh: "按 key、描述、分类或来源筛选",
    en: "Filter by key, description, category, or source",
  },
  "featureFlags.empty": {
    zh: "还没有可配置的功能开关。",
    en: "No configurable feature flags yet.",
  },
  "featureFlags.matchingEmpty": {
    zh: "没有功能开关匹配当前搜索。",
    en: "No feature flags match the current search.",
  },
  "featureFlags.noRequests": {
    zh: "还没有功能开关变更请求。",
    en: "No feature flag change requests yet.",
  },
  "featureFlags.noRequestsForFilter": {
    zh: "当前筛选下没有变更请求。",
    en: "No change requests match the current filter.",
  },
  "featureFlags.optionalReviewNote": {
    zh: "可选：填写审核备注",
    en: "Optional review note",
  },
  "featureFlags.pendingRequests": {
    zh: "待处理请求",
    en: "Pending requests",
  },
  "featureFlags.reasonPlaceholder": {
    zh: "填写本次变更请求的原因",
    en: "Reason for this change request",
  },
  "featureFlags.reject": {
    zh: "拒绝",
    en: "Reject",
  },
  "featureFlags.requestDisable": {
    zh: "请求关闭",
    en: "Request disable",
  },
  "featureFlags.requestEnable": {
    zh: "请求开启",
    en: "Request enable",
  },
  "featureFlags.requested": {
    zh: "请求变更为：",
    en: "Requested: ",
  },
  "featureFlags.requestedBy": {
    zh: "请求人：",
    en: "Requested by: ",
  },
  "featureFlags.reviewNote": {
    zh: "审核备注：",
    en: "Review note: ",
  },
  "featureFlags.seeded": {
    zh: "初始种子",
    en: "Seeded",
  },
  "featureFlags.source": {
    zh: "来源：",
    en: "Source: ",
  },
  "organizations.activeOrganizations": {
    zh: "正常组织",
    en: "Active organizations",
  },
  "organizations.blockOrganization": {
    zh: "封禁组织",
    en: "Block organization",
  },
  "organizations.blockedOrganizations": {
    zh: "封禁组织",
    en: "Blocked organizations",
  },
  "organizations.empty": {
    zh: "没有找到组织。",
    en: "No organizations found.",
  },
  "organizations.filterByNameIdPlan": {
    zh: "按名称、ID 或计划筛选",
    en: "Filter by name, ID, or plan",
  },
  "organizations.loading": {
    zh: "正在加载组织...",
    en: "Loading organizations...",
  },
  "organizations.matching": {
    zh: "匹配组织",
    en: "Matching organizations",
  },
  "organizations.noMatch": {
    zh: "没有组织匹配当前筛选。",
    en: "No organizations match the current filters.",
  },
  "organizations.sort.nameAsc": {
    zh: "名称 A-Z",
    en: "Name A-Z",
  },
  "organizations.sort.notebooksDesc": {
    zh: "知识库数优先",
    en: "Workspaces desc",
  },
  "organizations.sort.queriesDesc": {
    zh: "请求数优先",
    en: "Queries desc",
  },
  "organizations.sort.usersDesc": {
    zh: "用户数优先",
    en: "Users desc",
  },
  "organizations.statusFilterLabel": {
    zh: "组织状态",
    en: "Organization status",
  },
  "organizations.subtitle": {
    zh: "查看组织、团队规模、知识库数量和访问状态。",
    en: "Review organizations, team size, notebooks, and access status.",
  },
  "organizations.totalWorkspaces": {
    zh: "知识库总数",
    en: "Total notebooks",
  },
  "organizations.unblockOrganization": {
    zh: "解除封禁组织",
    en: "Unblock organization",
  },
  "organizations.usersCovered": {
    zh: "覆盖用户",
    en: "Users covered",
  },
  "organizationDetail.loading": {
    zh: "正在加载组织...",
    en: "Loading organization...",
  },
  "organizationDetail.loadingInsights": {
    zh: "正在加载组织洞察...",
    en: "Loading organization insights...",
  },
  "organizationDetail.notFound": {
    zh: "未找到组织。",
    en: "Organization not found.",
  },
  "organizationDetail.operationalEfficiency": {
    zh: "运营效率",
    en: "Operational efficiency",
  },
  "organizationDetail.notebooksPerUser": {
    zh: "人均知识库",
    en: "Workspaces per user",
  },
  "organizationDetail.subtitle": {
    zh: "查看组织配置、成员构成和近期 7/30 天用量。",
    en: "Review organization configuration, member composition, and recent 7/30 day usage.",
  },
  "organizationDetail.teamComposition": {
    zh: "团队构成",
    en: "Team composition",
  },
  "organizationDetail.title": {
    zh: "组织详情",
    en: "Organization detail",
  },
  "organizationDetail.users": {
    zh: "位成员",
    en: "members",
  },
  "organizationsInAggregate": {
    zh: "个组织参与聚合",
    en: "organizations in aggregate",
  },
  "usage.aggregateScope": {
    zh: "全部组织（聚合）",
    en: "All organizations (aggregate)",
  },
  "usage.loading": {
    zh: "正在加载用量数据...",
    en: "Loading usage...",
  },
  "usage.noData": {
    zh: "暂时没有可显示的用量数据。",
    en: "No usage data is available yet.",
  },
  "usage.subtitle": {
    zh: "默认显示全部组织的聚合结果，并支持切换时间窗口。",
    en: "The default view aggregates all organizations and supports time-window switching.",
  },
  "users.allRoles": {
    zh: "全部角色",
    en: "All roles",
  },
  "users.chooseOrganization": {
    zh: "请选择一个组织以查看用户。",
    en: "Choose an organization to inspect users.",
  },
  "users.currentOrganization": {
    zh: "当前组织",
    en: "Current organization",
  },
  "users.filterPlaceholder": {
    zh: "按邮箱、姓名或角色筛选",
    en: "Filter by email, name, or role",
  },
  "users.latestActive": {
    zh: "最近活跃优先",
    en: "Latest active",
  },
  "users.loading": {
    zh: "正在加载用户...",
    en: "Loading users...",
  },
  "users.memberRoles": {
    zh: "成员角色",
    en: "Member roles",
  },
  "users.members": {
    zh: "成员",
    en: "Members",
  },
  "users.name": {
    zh: "姓名",
    en: "Name",
  },
  "users.newestFirst": {
    zh: "最新创建优先",
    en: "Newest first",
  },
  "users.sort.emailAsc": {
    zh: "邮箱 A-Z",
    en: "Email A-Z",
  },
  "users.noMatch": {
    zh: "没有用户匹配当前筛选。",
    en: "No users match the current filters.",
  },
  "users.noOrganizationSelected": {
    zh: "尚未选择组织",
    en: "No organization selected",
  },
  "users.roleGrouping": {
    zh: "角色分组",
    en: "Role grouping",
  },
  "users.subtitle": {
    zh: "先选择组织，再按邮箱、角色或创建时间查看成员。",
    en: "Select an organization, then inspect members by email, role, or creation time.",
  },
  "audit.actorIdPlaceholder": {
    zh: "执行者 ID",
    en: "Actor ID",
  },
  "ops.empty": {
    zh: "暂时没有可显示的数据。",
    en: "No data is available yet.",
  },
  "ops.failedDocuments": {
    zh: "失败文档",
    en: "Failed documents",
  },
  "ops.guardEvents": {
    zh: "Guard 事件",
    en: "Guard events",
  },
  "ops.processing": {
    zh: "处理中",
    en: "Processing",
  },
  "rag.subtitle": {
    zh: "查看失败文档、排队任务和近期 Guard 事件。",
    en: "Review failed documents, queued tasks, and recent guard events.",
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
  "admin.blockAction": {
    zh: "封禁",
    en: "Block",
  },
  "admin.unblockAction": {
    zh: "解除封禁",
    en: "Unblock",
  },
  "admin.loadError": {
    zh: "加载后台数据失败。",
    en: "Failed to load admin data.",
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
  "admin.nav.ragHealth": {
    zh: "RAG 健康",
    en: "RAG Health",
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
} satisfies Record<string, AdminInlineCopy>;



export type { AdminInlineCopy };
export { INLINE_COPY };
export type AdminCopyKey = keyof typeof INLINE_COPY;
