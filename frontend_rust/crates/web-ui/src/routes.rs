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

/// Phase 0, E3.1 & E3.2 已挂载路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppRoute {
    Chat { session_id: Option<String> },
    Dashboard { workspace_id: Option<String> },
    DashboardAnalytics,
    WorkspaceAnalyze { workspace_id: String },
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
            ["dashboard", wid] => Self::Dashboard {
                workspace_id: Some(wid.to_string()),
            },
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
    use super::AppRoute;

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
