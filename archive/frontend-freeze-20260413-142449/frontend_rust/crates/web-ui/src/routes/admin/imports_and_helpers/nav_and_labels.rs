// Admin pages

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_params_map};
#[cfg(test)]
use serde::Serialize;
use std::cmp::Reverse;
use web_sdk::ApiClient;
use web_sdk::dtos::{
    AdminUsageResponse, AuditLogEntry, BillingOverview, DegradationStatusResponse,
    FeatureFlagChangeRequest, FeatureFlagEntry, HealthResponse, OrgResponse, OrgRow,
    RagHealthStatus, UserRow, WorkerStatusResponse,
};

use crate::api::api_base_url;
use crate::components::admin::{
    AdminMetricCard, HealthStatus, OrgDetailPanel, OrgListTable, UsageChart, UserListTable,
};
use crate::components::common::ErrorBanner;
use crate::i18n::{Locale, choose};
use crate::load::run_once_after_hydration;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

#[derive(Clone, Copy)]
enum AdminNavIcon {
    Building,
    Users,
    Usage,
    Heart,
    None,
}

#[derive(Clone, Copy)]
struct AdminNavItem {
    href: &'static str,
    prefixes: &'static [&'static str],
    icon: AdminNavIcon,
}

const ADMIN_NAV_ITEMS: &[AdminNavItem] = &[
    AdminNavItem {
        href: "/admin",
        prefixes: &["/admin", "/admin/organizations", "/admin/orgs"],
        icon: AdminNavIcon::Building,
    },
    AdminNavItem {
        href: "/admin/users",
        prefixes: &["/admin/users"],
        icon: AdminNavIcon::Users,
    },
    AdminNavItem {
        href: "/admin/usage",
        prefixes: &["/admin/usage"],
        icon: AdminNavIcon::Usage,
    },
    AdminNavItem {
        href: "/admin/billing",
        prefixes: &["/admin/billing"],
        icon: AdminNavIcon::None,
    },
    AdminNavItem {
        href: "/admin/health",
        prefixes: &["/admin/health"],
        icon: AdminNavIcon::Heart,
    },
    AdminNavItem {
        href: "/admin/rag-health",
        prefixes: &["/admin/rag-health"],
        icon: AdminNavIcon::None,
    },
    AdminNavItem {
        href: "/admin/feature-flags",
        prefixes: &["/admin/feature-flags"],
        icon: AdminNavIcon::None,
    },
    AdminNavItem {
        href: "/admin/system/workers",
        prefixes: &["/admin/system/workers"],
        icon: AdminNavIcon::None,
    },
    AdminNavItem {
        href: "/admin/system/degradation",
        prefixes: &["/admin/system/degradation"],
        icon: AdminNavIcon::None,
    },
    AdminNavItem {
        href: "/admin/audit-logs",
        prefixes: &["/admin/audit-logs"],
        icon: AdminNavIcon::None,
    },
];

fn nav_link_class(active: bool) -> &'static str {
    if active {
        "flex items-center px-3 py-2 text-sm font-medium rounded-md bg-muted text-foreground"
    } else {
        "flex items-center px-3 py-2 text-sm font-medium rounded-md text-muted-foreground hover:bg-muted/40 hover:text-foreground"
    }
}

fn admin_nav_label(locale: Locale, href: &str) -> &'static str {
    match href {
        "/admin" => choose(locale, "组织", "Organizations"),
        "/admin/users" => choose(locale, "用户", "Users"),
        "/admin/usage" => choose(locale, "用量", "Usage"),
        "/admin/billing" => choose(locale, "账单", "Billing"),
        "/admin/health" => choose(locale, "健康", "Health"),
        "/admin/rag-health" => choose(locale, "RAG 健康", "RAG Health"),
        "/admin/feature-flags" => choose(locale, "功能开关", "Feature Flags"),
        "/admin/system/workers" => choose(locale, "执行器", "Workers"),
        "/admin/system/degradation" => choose(locale, "降级", "Degradation"),
        "/admin/audit-logs" => choose(locale, "审计日志", "Audit Logs"),
        _ => "",
    }
}

fn feature_flag_status_label(locale: Locale, status: &str) -> String {
    match status {
        "pending" => choose(locale, "待处理", "Pending").to_string(),
        "approved" => choose(locale, "已批准", "Approved").to_string(),
        "rejected" => choose(locale, "已拒绝", "Rejected").to_string(),
        "executed" => choose(locale, "已执行", "Executed").to_string(),
        _ => status.to_string(),
    }
}

fn feature_flag_status_classes(status: &str) -> &'static str {
    match status {
        "pending" => "bg-amber-100 text-amber-800",
        "approved" => "bg-emerald-100 text-emerald-800",
        "rejected" => "bg-rose-100 text-rose-800",
        "executed" => "bg-sky-100 text-sky-800",
        _ => "bg-slate-100 text-slate-700",
    }
}

fn feature_flag_toggle_label(locale: Locale, enabled: bool) -> &'static str {
    if enabled {
        choose(locale, "开启", "on")
    } else {
        choose(locale, "关闭", "off")
    }
}

fn feature_flag_config_label(locale: Locale, ready: bool) -> &'static str {
    if ready {
        choose(locale, "就绪", "ready")
    } else {
        choose(locale, "缺失", "missing")
    }
}

fn humanize_identifier(value: &str) -> String {
    value
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn feature_flag_category_label(locale: Locale, category: &str) -> String {
    match category {
        "chat" => choose(locale, "对话", "Chat").to_string(),
        "retrieval" | "rag" => choose(locale, "检索", "Retrieval").to_string(),
        "ingestion" => choose(locale, "入库", "Ingestion").to_string(),
        "guard" | "guardrail" | "guardrails" | "safety" => {
            choose(locale, "安全护栏", "Guardrails").to_string()
        }
        "share" | "sharing" => choose(locale, "分享", "Sharing").to_string(),
        "admin" => choose(locale, "后台", "Admin").to_string(),
        "system" => choose(locale, "系统", "System").to_string(),
        "billing" => choose(locale, "账单", "Billing").to_string(),
        _ => humanize_identifier(category),
    }
}

fn feature_flag_source_label(locale: Locale, source: &str) -> String {
    match source {
        "seed" | "seeded" => choose(locale, "初始种子", "Seeded").to_string(),
        "env" | "environment" => choose(locale, "环境变量", "Environment").to_string(),
        "config" => choose(locale, "配置文件", "Config").to_string(),
        "db" | "database" | "postgres" | "postgresql" => {
            choose(locale, "数据库", "Database").to_string()
        }
        "request" | "change_request" => choose(locale, "变更申请", "Change Request").to_string(),
        "override" | "manual_override" | "emergency_override" => {
            choose(locale, "紧急覆盖", "Emergency Override").to_string()
        }
        "runtime" => choose(locale, "运行时", "Runtime").to_string(),
        _ => humanize_identifier(source),
    }
}

fn admin_user_role_label(locale: Locale, role: &str) -> String {
    match role {
        "owner" => choose(locale, "所有者", "Owner").to_string(),
        "admin" => choose(locale, "管理员", "Admin").to_string(),
        "member" => choose(locale, "成员", "Member").to_string(),
        "viewer" => choose(locale, "查看者", "Viewer").to_string(),
        "editor" => choose(locale, "编辑者", "Editor").to_string(),
        _ => role.to_string(),
    }
}

fn worker_runtime_label(locale: Locale, runtime: &str) -> String {
    match runtime {
        "inline" => choose(locale, "内联", "Inline").to_string(),
        "queue" | "queued" => choose(locale, "队列", "Queued").to_string(),
        "worker" => choose(locale, "执行器", "Worker").to_string(),
        _ => runtime.to_string(),
    }
}

fn audit_action_label(locale: Locale, action: &str) -> String {
    match action {
        "task_enqueued" => choose(locale, "任务已入队", "Task enqueued").to_string(),
        "task_started" => choose(locale, "任务开始执行", "Task started").to_string(),
        "task_completed" => choose(locale, "任务执行完成", "Task completed").to_string(),
        "task_failed" => choose(locale, "任务执行失败", "Task failed").to_string(),
        "state_transition" => choose(locale, "状态迁移", "State transition").to_string(),
        "input_guard_block" => {
            choose(locale, "输入被 Guard 拦截", "Input guard blocked").to_string()
        }
        "output_guard_block" => {
            choose(locale, "输出被 Guard 拦截", "Output guard blocked").to_string()
        }
        "output_guard_redact" => {
            choose(locale, "输出被 Guard 脱敏", "Output guard redacted").to_string()
        }
        "output_guard_flag" => {
            choose(locale, "输出被 Guard 标记", "Output guard flagged").to_string()
        }
        _ => action.replace('_', " "),
    }
}

fn audit_resource_type_label(locale: Locale, resource_type: &str) -> String {
    match resource_type {
        "document" => choose(locale, "文档", "Document").to_string(),
        "document_ingestion_task" => {
            choose(locale, "文档入库任务", "Document ingestion task").to_string()
        }
        "chat" => choose(locale, "对话", "Chat").to_string(),
        _ => resource_type.replace('_', " "),
    }
}
