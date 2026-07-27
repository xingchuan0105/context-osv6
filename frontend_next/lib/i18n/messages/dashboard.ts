import type { UiMessageDescriptor } from "./types";

export const dashboardMessages = {
  dashboardActionDelete: {
    zh: "删除",
    en: "Delete",
  },
  dashboardActionFavorite: {
    zh: "收藏",
    en: "Favorite",
  },
  dashboardActionRename: {
    zh: "重命名",
    en: "Rename",
  },
  dashboardActionUnfavorite: {
    zh: "取消收藏",
    en: "Unfavorite",
  },
  dashboardBrandSubtitle: {
    zh: "工作区控制台",
    en: "Workspace Dashboard",
  },
  dashboardCloseSearch: {
    zh: "关闭搜索",
    en: "Close search",
  },
  dashboardCreateFirst: {
    zh: "创建第一个工作区",
    en: "Create the first workspace",
  },
  dashboardEmptyAllTitle: {
    zh: "还没有工作区",
    en: "No workspaces yet",
  },
  dashboardEmptyBody: {
    zh: "先创建一个工作区，然后再进入工作区。",
    en: "Create a workspace first, then jump into the workspace shell.",
  },
  dashboardEmptyDescription: {
    zh: "暂无描述",
    en: "No description",
  },
  dashboardEmptyFavoritesTitle: {
    zh: "还没有收藏的工作区",
    en: "No favorited workspaces yet",
  },
  dashboardHeadingAll: {
    zh: "全部工作区",
    en: "All workspaces",
  },
  dashboardHeadingCount: {
    zh: "{count} 个工作区",
    en: "{count} workspaces",
  },
  dashboardHeadingFavorites: {
    zh: "我的收藏",
    en: "Favorites",
  },
  dashboardHeadingMine: {
    zh: "我的工作区",
    en: "My workspaces",
  },
  dashboardListLabel: {
    zh: "工作区列表",
    en: "Workspace list",
  },
  dashboardLoadError: {
    zh: "加载工作区失败",
    en: "Failed to load workspaces.",
  },
  dashboardActionFailed: {
    zh: "操作失败，请稍后重试。",
    en: "Something went wrong. Please try again.",
  },
  dashboardActionRetry: {
    zh: "重试",
    en: "Retry",
  },
  dashboardDeleteDialogTitle: {
    zh: "删除工作区",
    en: "Delete workspace",
  },
  dashboardDeleteDialogBody: {
    zh: "确定删除 {title} 吗？此操作无法撤销。",
    en: "Delete {title}? This cannot be undone.",
  },
  dashboardRenameDialogTitle: {
    zh: "重命名工作区",
    en: "Rename workspace",
  },
  dashboardRenameSubmit: {
    zh: "保存",
    en: "Save",
  },
  dashboardLoginRequired: {
    zh: "请先登录",
    en: "Please sign in first.",
  },
  dashboardRoleMember: {
    zh: "成员",
    en: "Member",
  },
  dashboardRoleOwner: {
    zh: "所有者",
    en: "Owner",
  },
  dashboardSearchDialogLabel: {
    zh: "快速打开工作区",
    en: "Quick open workspace",
  },
  dashboardCreatedAtColumn: {
    zh: "创建时间",
    en: "Created",
  },
  dashboardRoleColumn: {
    zh: "角色",
    en: "Role",
  },
  dashboardAccountLink: {
    zh: "账户",
    en: "Account",
  },
  dashboardAppearanceLink: {
    zh: "外观",
    en: "Appearance",
  },
  dashboardProfileLink: {
    zh: "个人资料",
    en: "Profile",
  },
  dashboardBillingLink: {
    zh: "订阅与用量",
    en: "Subscription & Usage",
  },
  dashboardLogout: {
    zh: "退出登录",
    en: "Log out",
  },
  dashboardBackToWorkspaces: {
    zh: "返回工作台",
    en: "Back to workspace",
  },
  dashboardSearchEmptyIdle: {
    zh: "输入关键词搜索工作区",
    en: "Type to search workspaces",
  },
  dashboardSearchEmptyNoMatch: {
    zh: "没有匹配的工作区",
    en: "No matching workspaces",
  },
  dashboardSearchLabel: {
    zh: "搜索工作区",
    en: "Search workspaces",
  },
  dashboardSearchPlaceholder: {
    zh: "搜索工作区标题或描述",
    en: "Search workspace titles or descriptions",
  },
  dashboardSearchResultsLabel: {
    zh: "工作区搜索结果",
    en: "Workspace search results",
  },
  dashboardSearchSubtitle: {
    zh: "输入关键词，点击结果进入工作区",
    en: "Type a keyword and jump directly into the workspace.",
  },
  dashboardSearchTitle: {
    zh: "快速打开工作区",
    en: "Quick open workspace",
  },
  dashboardSortRecent: {
    zh: "创建时间",
    en: "Created",
  },
  dashboardSortTitle: {
    zh: "标题",
    en: "Title",
  },
  dashboardStatusFailed: {
    zh: "异常",
    en: "failed",
  },
  dashboardStatusProcessing: {
    zh: "处理中",
    en: "processing",
  },
  dashboardStatusReady: {
    zh: "就绪",
    en: "ready",
  },
  dashboardTabAll: {
    zh: "全部",
    en: "All",
  },
  dashboardTabFavorites: {
    zh: "我的收藏",
    en: "Favorites",
  },
  dashboardTabMine: {
    zh: "我的工作区",
    en: "My workspaces",
  },
  dashboardTabsLabel: {
    zh: "工作区标签",
    en: "Workspace tabs",
  },
  dashboardToolbarSearch: {
    zh: "搜索工作区",
    en: "Search workspaces",
  },
  dashboardViewCard: {
    zh: "卡片",
    en: "Cards",
  },
  dashboardViewGridLabel: {
    zh: "工作区卡片",
    en: "Workspace cards",
  },
  dashboardViewList: {
    zh: "列表",
    en: "List",
  },
  dashboardViewModeLabel: {
    zh: "工作区视图模式",
    en: "Workspace view mode",
  },
  dashboardWorkspaceNameField: {
    zh: "名称",
    en: "Name",
  },
  dashboardNewWorkspace: {
    zh: "新建工作区",
    en: "New workspace",
  },
  dashboardUntitledWorkspace: {
    zh: "未命名工作区",
    en: "Untitled workspace",
  },
} satisfies Record<string, UiMessageDescriptor>;
