/// 现行 route family 清单。导航权威仍是 Next `nav-config.ts`；
/// 本表做 parity，并记录 render / auth / noindex。未挂载的 family 不表示页面已实现。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Ssr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRequirement {
    Required,
    Optional,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteFamily {
    pub id: &'static str,
    pub href: &'static str,
    pub render_mode: RenderMode,
    pub auth: AuthRequirement,
    pub noindex: bool,
}

pub const ROUTE_FAMILIES: &[RouteFamily] = &[
    RouteFamily {
        id: "chat",
        href: "/chat",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Required,
        noindex: true,
    },
    RouteFamily {
        id: "dashboard",
        href: "/dashboard",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Required,
        noindex: true,
    },
    RouteFamily {
        id: "share-traffic",
        href: "/dashboard/analytics",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Required,
        noindex: true,
    },
    RouteFamily {
        id: "settings",
        href: "/settings",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Required,
        noindex: true,
    },
    RouteFamily {
        id: "providers",
        href: "/settings?tab=providers",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Required,
        noindex: true,
    },
    RouteFamily {
        id: "billing",
        href: "/settings?tab=billing",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Required,
        noindex: true,
    },
    RouteFamily {
        id: "pricing",
        href: "/pricing",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
    RouteFamily {
        id: "topup",
        href: "/pricing#topup",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
    RouteFamily {
        id: "desktop",
        href: "/desktop",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
    RouteFamily {
        id: "help",
        href: "/help",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Optional,
        noindex: true,
    },
    RouteFamily {
        id: "api-access",
        href: "/help/api-access",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
    RouteFamily {
        id: "legal",
        href: "/legal",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
    RouteFamily {
        id: "legal-terms",
        href: "/legal/terms",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
    RouteFamily {
        id: "legal-privacy",
        href: "/legal/privacy",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
    RouteFamily {
        id: "legal-licenses",
        href: "/legal/licenses",
        render_mode: RenderMode::Ssr,
        auth: AuthRequirement::Public,
        noindex: false,
    },
];

pub fn route_family(id: &str) -> Option<&'static RouteFamily> {
    ROUTE_FAMILIES.iter().find(|family| family.id == id)
}

/// E3.5 已挂载的管理后台路由（admin 不属导航权威，见 PRODUCT_IA；仅登记路由解析）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSection {
    Overview,
    Accounts,
    AccountDetail,
    Users,
    Usage,
    Billing,
    Health,
    RagHealth,
    Workers,
    Degradation,
    Broadcast,
    AuditLogs,
    FeatureFlags,
}

/// Phase 0, E3.1, E3.2, E3.3, E3.4 & E3.5 已挂载路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppRoute {
    Chat { session_id: Option<String> },
    Dashboard { workspace_id: Option<String> },
    DashboardAnalytics,
    WorkspaceAnalyze { workspace_id: String },
    WorkspaceShare { workspace_id: String },
    WorkspaceShareLogs { workspace_id: String },
    WorkspaceShareAnalytics { workspace_id: String },
    SharedKb { token: String },
    SharedUser { user_id: String },
    Invite { workspace_id: String, member_id: String },
    Admin { section: AdminSection },
    Help,
    HelpWrite,
    Pricing,
    UpgradePaywall,
    UpgradeSuccess,
    DesktopBuy,
    Login,
    Register,
    ResetPassword,
    ResetPasswordVerify,
    ResetPasswordConfirm,
    Settings,
    SettingsUsage,
    NotFound,
}

impl AppRoute {
    pub fn parse(path: &str) -> Self {
        let trimmed = path.trim_matches('/');
        let segments: Vec<&str> = if trimmed.is_empty() {
            vec![]
        } else {
            trimmed.split('/').collect()
        };

        match segments.as_slice() {
            ["chat"] => Self::Chat { session_id: None },
            ["chat", sid] => Self::Chat {
                session_id: Some(sid.to_string()),
            },
            ["dashboard"] => Self::Dashboard { workspace_id: None },
            ["dashboard", "analytics"] => Self::DashboardAnalytics,
            ["dashboard", wid, "analyze"] => Self::WorkspaceAnalyze {
                workspace_id: wid.to_string(),
            },
            ["dashboard", wid, "share", "access-logs"] => Self::WorkspaceShareLogs {
                workspace_id: wid.to_string(),
            },
            ["dashboard", wid, "share", "analytics"] => Self::WorkspaceShareAnalytics {
                workspace_id: wid.to_string(),
            },
            ["dashboard", wid, "share"] => Self::WorkspaceShare {
                workspace_id: wid.to_string(),
            },
            ["dashboard", wid] => Self::Dashboard {
                workspace_id: Some(wid.to_string()),
            },
            ["shared", "kb", tok] => Self::SharedKb {
                token: tok.to_string(),
            },
            ["shared", "u", uid] => Self::SharedUser {
                user_id: uid.to_string(),
            },
            ["invite", wid, mid] => Self::Invite {
                workspace_id: wid.to_string(),
                member_id: mid.to_string(),
            },
            ["admin"] => Self::Admin {
                section: AdminSection::Overview,
            },
            ["admin", "accounts"] => Self::Admin {
                section: AdminSection::Accounts,
            },
            ["admin", "accounts", _] => Self::Admin {
                section: AdminSection::AccountDetail,
            },
            ["admin", "users"] => Self::Admin {
                section: AdminSection::Users,
            },
            ["admin", "usage"] => Self::Admin {
                section: AdminSection::Usage,
            },
            ["admin", "billing"] => Self::Admin {
                section: AdminSection::Billing,
            },
            ["admin", "health"] => Self::Admin {
                section: AdminSection::Health,
            },
            ["admin", "rag-health"] => Self::Admin {
                section: AdminSection::RagHealth,
            },
            ["admin", "system", "workers"] => Self::Admin {
                section: AdminSection::Workers,
            },
            ["admin", "system", "degradation"] => Self::Admin {
                section: AdminSection::Degradation,
            },
            ["admin", "broadcast"] => Self::Admin {
                section: AdminSection::Broadcast,
            },
            ["admin", "audit-logs"] => Self::Admin {
                section: AdminSection::AuditLogs,
            },
            ["admin", "feature-flags"] => Self::Admin {
                section: AdminSection::FeatureFlags,
            },
            ["help"] => Self::Help,
            ["help", "write"] => Self::HelpWrite,
            ["pricing"] => Self::Pricing,
            ["upgrade", "paywall"] => Self::UpgradePaywall,
            ["upgrade", "success"] => Self::UpgradeSuccess,
            ["desktop", "buy"] => Self::DesktopBuy,
            ["login"] => Self::Login,
            ["register"] => Self::Register,
            ["reset-password"] => Self::ResetPassword,
            ["reset-password", "verify"] => Self::ResetPasswordVerify,
            ["reset-password", "confirm"] => Self::ResetPasswordConfirm,
            ["settings"] => Self::Settings,
            ["settings", "usage"] => Self::SettingsUsage,
            _ => Self::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AdminSection, AppRoute};

    #[test]
    fn admin_and_help_routes_are_canonical() {
        assert_eq!(
            AppRoute::parse("/admin"),
            AppRoute::Admin {
                section: AdminSection::Overview
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/accounts"),
            AppRoute::Admin {
                section: AdminSection::Accounts
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/accounts/ow-1"),
            AppRoute::Admin {
                section: AdminSection::AccountDetail
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/users"),
            AppRoute::Admin {
                section: AdminSection::Users
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/usage"),
            AppRoute::Admin {
                section: AdminSection::Usage
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/billing"),
            AppRoute::Admin {
                section: AdminSection::Billing
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/health"),
            AppRoute::Admin {
                section: AdminSection::Health
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/rag-health"),
            AppRoute::Admin {
                section: AdminSection::RagHealth
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/system/workers"),
            AppRoute::Admin {
                section: AdminSection::Workers
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/system/degradation"),
            AppRoute::Admin {
                section: AdminSection::Degradation
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/broadcast"),
            AppRoute::Admin {
                section: AdminSection::Broadcast
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/audit-logs"),
            AppRoute::Admin {
                section: AdminSection::AuditLogs
            }
        );
        assert_eq!(
            AppRoute::parse("/admin/feature-flags"),
            AppRoute::Admin {
                section: AdminSection::FeatureFlags
            }
        );
        assert_eq!(AppRoute::parse("/help"), AppRoute::Help);
        assert_eq!(AppRoute::parse("/help/write"), AppRoute::HelpWrite);
        assert_eq!(AppRoute::parse("/admin/nope"), AppRoute::NotFound);
    }

    #[test]
    fn chat_routes_are_canonical_and_root_is_not_chat() {
        assert_eq!(AppRoute::parse("/"), AppRoute::NotFound);
        assert_eq!(
            AppRoute::parse("/chat"),
            AppRoute::Chat { session_id: None }
        );
        assert_eq!(
            AppRoute::parse("/chat/session-1"),
            AppRoute::Chat {
                session_id: Some("session-1".to_string())
            }
        );
        assert_eq!(
            AppRoute::parse("/dashboard"),
            AppRoute::Dashboard { workspace_id: None }
        );
        assert_eq!(
            AppRoute::parse("/dashboard/analytics"),
            AppRoute::DashboardAnalytics
        );
        assert_eq!(
            AppRoute::parse("/dashboard/ws-1"),
            AppRoute::Dashboard {
                workspace_id: Some("ws-1".to_string())
            }
        );
        assert_eq!(
            AppRoute::parse("/dashboard/ws-1/analyze"),
            AppRoute::WorkspaceAnalyze {
                workspace_id: "ws-1".to_string()
            }
        );
        assert_eq!(
            AppRoute::parse("/dashboard/ws-1/share"),
            AppRoute::WorkspaceShare {
                workspace_id: "ws-1".to_string()
            }
        );
        assert_eq!(
            AppRoute::parse("/shared/kb/tok-1"),
            AppRoute::SharedKb {
                token: "tok-1".to_string()
            }
        );
        assert_eq!(
            AppRoute::parse("/shared/u/u-1"),
            AppRoute::SharedUser {
                user_id: "u-1".to_string()
            }
        );
        assert_eq!(
            AppRoute::parse("/invite/ws-1/mem-1"),
            AppRoute::Invite {
                workspace_id: "ws-1".to_string(),
                member_id: "mem-1".to_string()
            }
        );
        assert_eq!(AppRoute::parse("/pricing"), AppRoute::Pricing);
        assert_eq!(
            AppRoute::parse("/upgrade/paywall"),
            AppRoute::UpgradePaywall
        );
        assert_eq!(
            AppRoute::parse("/upgrade/success"),
            AppRoute::UpgradeSuccess
        );
        assert_eq!(
            AppRoute::parse("/desktop/buy"),
            AppRoute::DesktopBuy
        );
        assert_eq!(AppRoute::parse("/login"), AppRoute::Login);
        assert_eq!(AppRoute::parse("/register"), AppRoute::Register);
        assert_eq!(AppRoute::parse("/reset-password"), AppRoute::ResetPassword);
        assert_eq!(
            AppRoute::parse("/reset-password/verify"),
            AppRoute::ResetPasswordVerify
        );
        assert_eq!(
            AppRoute::parse("/reset-password/confirm"),
            AppRoute::ResetPasswordConfirm
        );
        assert_eq!(AppRoute::parse("/settings"), AppRoute::Settings);
        assert_eq!(AppRoute::parse("/settings/usage"), AppRoute::SettingsUsage);
    }
}
