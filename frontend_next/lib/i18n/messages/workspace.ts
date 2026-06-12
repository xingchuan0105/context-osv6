import type { UiMessageDescriptor } from "./types";

export const workspaceMessages = {
  workspaceAnalyze: {
    zh: "分析",
    en: "Analyze",
  },
  workspaceDistribute: {
    zh: "传播",
    en: "Distribute",
  },
  workspaceAnonymousUser: {
    zh: "未登录",
    en: "Not signed in",
  },
  workspaceApi: {
    zh: "API",
    en: "API",
  },
  workspaceChatComposerHint: {
    zh: "Enter 发送，Shift+Enter 换行。",
    en: "Press Enter to send and Shift+Enter for a newline.",
  },
  workspaceChatActionAddToNote: {
    zh: "加入笔记",
    en: "Add to note",
  },
  workspaceChatActionCopy: {
    zh: "复制",
    en: "Copy",
  },
  workspaceChatActionEdit: {
    zh: "编辑",
    en: "Edit",
  },
  workspaceChatActionRegenerate: {
    zh: "重新生成",
    en: "Regenerate",
  },
  workspaceChatActionThumbUp: {
    zh: "有用",
    en: "Helpful",
  },
  workspaceChatActionThumbDown: {
    zh: "无用",
    en: "Not helpful",
  },
  workspaceChatClearMode: {
    zh: "清除对话模式",
    en: "Clear chat mode",
  },
  workspaceChatComposerLabel: {
    zh: "工作区对话输入框",
    en: "Workspace chat composer",
  },
  workspaceChatComposerPlaceholder: {
    zh: "输入 / 选择模式，然后开始提问...",
    en: "Type / to choose a mode, then start typing...",
  },
  workspaceCitationsLabel: {
    zh: "引用",
    en: "Citations",
  },
  workspaceChatEyebrow: {
    zh: "工作区对话",
    en: "Workspace chat",
  },
  workspaceChatLoadError: {
    zh: "加载工作区对话记录失败。",
    en: "Failed to load workspace transcript.",
  },
  workspaceChatModeChat: {
    zh: "聊天模式",
    en: "chat",
  },
  workspaceChatModeLabel: {
    zh: "对话模式",
    en: "Chat mode",
  },
  workspaceChatModeRag: {
    zh: "知识库检索",
    en: "RAG",
  },
  workspaceChatModeSearch: {
    zh: "网络搜索",
    en: "web_search",
  },
  workspaceChatRegionLabel: {
    zh: "工作区对话",
    en: "Workspace chat",
  },
  workspaceChatSubtitle: {
    zh: "和当前工作区对话",
    en: "Chat with the workspace",
  },
  workspaceCreateAction: {
    zh: "创建工作区",
    en: "Create workspace",
  },
  workspaceCreateDialogLabel: {
    zh: "新建工作区",
    en: "Create workspace",
  },
  workspaceDegradeReasons: {
    zh: "降级原因：{reasons}",
    en: "Degrade reasons: {reasons}",
  },
  workspaceDescriptionField: {
    zh: "描述",
    en: "Description",
  },
  workspaceGuardIntervened: {
    zh: "Guardrail 已介入当前回答。",
    en: "Guardrails intervened in this answer.",
  },
  workspaceHistoryLabel: {
    zh: "工作区历史",
    en: "Workspace history",
  },
  workspaceHistorySearch: {
    zh: "搜索会话",
    en: "Search sessions",
  },
  workspaceSearchDialogLabel: {
    zh: "搜索会话",
    en: "Search sessions",
  },
  workspaceSearchTitle: {
    zh: "搜索会话",
    en: "Search sessions",
  },
  workspaceSearchSubtitle: {
    zh: "按关键词搜索会话标题、摘要和聊天正文。",
    en: "Search session titles, summaries, and chat content by keyword.",
  },
  workspaceSearchPlaceholder: {
    zh: "输入关键词搜索聊天正文",
    en: "Search chat content by keyword",
  },
  workspaceSearchEmptyIdle: {
    zh: "输入关键词后即可搜索会话和聊天正文。",
    en: "Type a keyword to search sessions and chat content.",
  },
  workspaceSearchEmptyNoMatch: {
    zh: "没有找到匹配的会话。",
    en: "No matching sessions found.",
  },
  workspaceSearchResultsLabel: {
    zh: "会话搜索结果",
    en: "Session search results",
  },
  workspaceSearchLoading: {
    zh: "正在加载会话内容…",
    en: "Loading session content...",
  },
  workspaceSearchLoadError: {
    zh: "部分会话内容加载失败，搜索结果可能不完整。",
    en: "Some session content could not be loaded, so results may be incomplete.",
  },
  workspaceHistorySubtitle: {
    zh: "查看会话、筛选结果和线程操作",
    en: "Sessions, filters, and thread actions",
  },
  workspaceHistoryTitle: {
    zh: "历史",
    en: "History",
  },
  workspaceLanguageChinese: {
    zh: "中文",
    en: "Chinese",
  },
  workspaceLanguageEnglish: {
    zh: "English",
    en: "English",
  },
  workspaceMenuLanguage: {
    zh: "语言",
    en: "Language",
  },
  workspaceMenuTheme: {
    zh: "主题",
    en: "Theme",
  },
  workspaceNameField: {
    zh: "工作区名称",
    en: "Workspace name",
  },
  workspaceNewThread: {
    zh: "新建会话",
    en: "New session",
  },
  workspaceNewWorkspace: {
    zh: "新建工作区",
    en: "New workspace",
  },
  workspaceNoMessages: {
    zh: "还没有消息。",
    en: "No messages yet.",
  },
  workspaceEmptyStateBody: {
    zh: "开始一个新对话，或围绕当前工作区资料提出问题。",
    en: "Start a new conversation or ask a question about this workspace.",
  },
  workspaceNoSessionsMatch: {
    zh: "暂无会话。",
    en: "No sessions yet.",
  },
  workspacePromptRenameSession: {
    zh: "重命名会话",
    en: "Rename session",
  },
  workspaceRecentSession: {
    zh: "最近",
    en: "Recent",
  },
  workspaceRightRailLabel: {
    zh: "工作区右侧栏",
    en: "Workspace right rail",
  },
  workspaceRenameSessionAction: {
    zh: "重命名",
    en: "Rename",
  },
  workspaceDeleteSessionAction: {
    zh: "删除",
    en: "Delete",
  },
  workspacePinSessionAction: {
    zh: "置顶",
    en: "Pin",
  },
  workspaceUnpinSessionAction: {
    zh: "取消置顶",
    en: "Unpin",
  },
  workspacePinnedSession: {
    zh: "已置顶",
    en: "Pinned",
  },
  workspaceLogout: {
    zh: "退出登录",
    en: "Log out",
  },
  workspaceOpenAccountMenu: {
    zh: "打开账号菜单",
    en: "Open account menu",
  },
  workspaceOpenSettingsMenu: {
    zh: "打开设置菜单",
    en: "Open settings menu",
  },
  workspaceSaveTitle: {
    zh: "保存标题",
    en: "Save workspace title",
  },
  workspaceSending: {
    zh: "发送中...",
    en: "Sending...",
  },
  workspaceSend: {
    zh: "发送",
    en: "Send",
  },
  workspaceSessionTitleField: {
    zh: "工作区标题",
    en: "Workspace title",
  },
  workspaceShare: {
    zh: "分享",
    en: "Share",
  },
  workspaceThemeDark: {
    zh: "深色",
    en: "Dark",
  },
  workspaceThemeLight: {
    zh: "浅色",
    en: "Light",
  },
  workspaceThemeSystem: {
    zh: "跟随系统",
    en: "System",
  },
  workspaceThreadTitleField: {
    zh: "会话标题",
    en: "Session title",
  },
  workspaceTitleEyebrow: {
    zh: "工作区",
    en: "Workspace",
  },
  workspaceToggleHistoryRail: {
    zh: "切换历史栏",
    en: "Toggle history rail",
  },
  workspaceToggleRightRail: {
    zh: "切换右侧栏",
    en: "Toggle right rail",
  },
  workspaceTranscriptLabel: {
    zh: "工作区记录",
    en: "Workspace transcript",
  },
  workspaceRenameSessionDialogLabel: {
    zh: "重命名会话",
    en: "Rename session",
  },
  workspaceRenameSessionSubmit: {
    zh: "保存会话",
    en: "Save session",
  },
  workspaceStreamError: {
    zh: "发送工作区对话失败。",
    en: "Failed to stream workspace chat.",
  },
  "workspaceShell.eyebrow": {
    zh: "工作区",
    en: "Workspace",
  },
  "workspaceShell.titleLabel": {
    zh: "工作区标题",
    en: "Workspace title",
  },
  "workspaceShell.titlePlaceholder": {
    zh: "输入工作区标题",
    en: "Enter a workspace title",
  },
  "workspaceShell.descriptionLabel": {
    zh: "工作区描述",
    en: "Workspace description",
  },
  "workspaceShell.descriptionPlaceholder": {
    zh: "补充当前工作区的目标或上下文",
    en: "Add a short goal or context for this workspace",
  },
  "workspaceShell.saveAction": {
    zh: "保存标题",
    en: "Save title",
  },
  "workspaceShell.analyzeAction": {
    zh: "分析",
    en: "Analyze",
  },
  "workspaceShell.shareAction": {
    zh: "分享",
    en: "Share",
  },
  "workspaceShell.apiAction": {
    zh: "API 访问",
    en: "API access",
  },
  "workspaceShell.newThreadAction": {
    zh: "新建会话",
    en: "New session",
  },
  "workspaceShell.newWorkspaceAction": {
    zh: "新建工作区",
    en: "New workspace",
  },
  "workspaceShell.createDialogTitle": {
    zh: "创建工作区",
    en: "Create workspace",
  },
  "workspaceShell.createDialogSubtitle": {
    zh: "先命名并补充描述，再进入新的工作区。",
    en: "Name the workspace and add a short description before entering it.",
  },
  "workspaceShell.historyRailToggle": {
    zh: "切换历史栏",
    en: "Toggle history rail",
  },
  "workspaceShell.rightRailToggle": {
    zh: "切换右侧栏",
    en: "Toggle right rail",
  },
  "workspaceShell.settingsMenuLabel": {
    zh: "工作区设置菜单",
    en: "Workspace settings menu",
  },
  "workspaceShell.accountMenuLabel": {
    zh: "账户菜单",
    en: "Account menu",
  },
  "workspaceShell.themeSectionTitle": {
    zh: "主题",
    en: "Theme",
  },
  "workspaceShell.languageSectionTitle": {
    zh: "语言",
    en: "Language",
  },
  "workspaceShell.emptyTitle": {
    zh: "还没有工作区内容",
    en: "No workspace content yet",
  },
  "workspaceShell.emptyBody": {
    zh: "先创建会话、上传资料，或从右侧栏开始整理上下文。",
    en: "Start with a thread, upload sources, or organize context from the right rail.",
  },
  "workspaceShell.loadError": {
    zh: "加载工作区失败。",
    en: "Failed to load the workspace.",
  },
  "workspaceRightRail.label": {
    zh: "工作区右侧栏",
    en: "Workspace right rail",
  },
  "workspaceRightRail.sourcesSectionTitle": {
    zh: "内容源",
    en: "Sources",
  },
  "workspaceRightRail.sourcesSectionSubtitle": {
    zh: "管理可检索资料、URL 导入和文档状态。",
    en: "Manage retrievable sources, URL imports, and document status.",
  },
  "workspaceRightRail.notesSectionTitle": {
    zh: "笔记",
    en: "Notes",
  },
  "workspaceRightRail.notesSectionSubtitle": {
    zh: "记录工作区结论、草稿和后续行动。",
    en: "Capture conclusions, drafts, and next actions for the workspace.",
  },
  "workspaceRightRail.viewerSectionTitle": {
    zh: "预览",
    en: "Preview",
  },
  "workspaceRightRail.viewerSectionSubtitle": {
    zh: "查看资料片段、引用内容和原始文本。",
    en: "Inspect source excerpts, citations, and raw content.",
  },
  "workspaceRightRail.sourceUrlLabel": {
    zh: "资料链接",
    en: "Source URLs",
  },
  "workspaceRightRail.sourceUrlPlaceholder": {
    zh: "每行输入一个 URL，支持直接粘贴多行链接",
    en: "Paste one URL per line",
  },
  "workspaceRightRail.addUrlAction": {
    zh: "添加链接",
    en: "Add URL",
  },
  "workspaceRightRail.refreshAction": {
    zh: "刷新",
    en: "Refresh",
  },
  "workspaceRightRail.reindexAction": {
    zh: "重建索引",
    en: "Reindex",
  },
  "workspaceRightRail.emptySourcesTitle": {
    zh: "还没有资料",
    en: "No sources yet",
  },
  "workspaceRightRail.emptySourcesBody": {
    zh: "上传文件或添加 URL 后，资料会出现在这里。",
    en: "Uploaded files and imported URLs will appear here.",
  },
  "workspaceRightRail.emptyNotesTitle": {
    zh: "还没有笔记",
    en: "No notes yet",
  },
  "workspaceRightRail.emptyNotesBody": {
    zh: "把关键结论、决策和行动项记录到这里。",
    en: "Capture key conclusions, decisions, and action items here.",
  },
  "workspaceRightRail.notesSavedBanner": {
    zh: "笔记已同步。",
    en: "Notes synced.",
  },
  "workspaceRightRail.notesSavingBanner": {
    zh: "正在同步笔记...",
    en: "Syncing notes...",
  },
  "workspaceRightRail.notesErrorBanner": {
    zh: "同步笔记失败。",
    en: "Failed to sync notes.",
  },
  "workspaceRightRail.viewerEmptyTitle": {
    zh: "选择资料以查看内容",
    en: "Select a source to preview",
  },
  "workspaceRightRail.viewerEmptyBody": {
    zh: "从资料列表或引用卡片中选中一个条目后，这里会显示预览。",
    en: "Choose a source from the list or a citation card to preview it here.",
  },
  "workspaceRightRail.viewerLoadMoreAction": {
    zh: "加载更多",
    en: "Load more",
  },
  "workspaceRightRail.sourcesError": {
    zh: "加载资料失败。",
    en: "Failed to load sources.",
  },
  "workspaceRightRail.notesError": {
    zh: "加载笔记失败。",
    en: "Failed to load notes.",
  },
  "workspaceRightRail.saveNoteError": {
    zh: "保存笔记失败。",
    en: "Failed to save note.",
  },
  "workspaceRightRail.promoteNoteError": {
    zh: "转换为来源失败。",
    en: "Failed to convert note to source.",
  },
  "workspaceRightRail.promoteNoteEmptyError": {
    zh: "笔记为空，无法转换为来源。",
    en: "Cannot convert an empty note into a source.",
  },
  "workspaceRightRail.viewerError": {
    zh: "加载资料预览失败。",
    en: "Failed to load source preview.",
  },
  "workspaceRightRail.loading": {
    zh: "加载中...",
    en: "Loading...",
  },
  "workspaceRightRail.updating": {
    zh: "更新中...",
    en: "Updating...",
  },
  "workspaceRightRail.totalCount": {
    zh: "共 {count} 项",
    en: "{count} total",
  },
  "workspaceRightRail.selectAllAction": {
    zh: "全选",
    en: "Select all",
  },
  "workspaceRightRail.clearSelectionAction": {
    zh: "清除选择",
    en: "Clear selection",
  },
  "workspaceRightRail.hidePreviewAction": {
    zh: "收起预览",
    en: "Hide preview",
  },
  "workspaceRightRail.sourceActionsLabel": {
    zh: "资料操作",
    en: "Source actions",
  },
  "workspaceRightRail.openSourceAction": {
    zh: "打开预览",
    en: "Open preview",
  },
  "workspaceRightRail.deleteSourceAction": {
    zh: "删除",
    en: "Delete",
  },
  "workspaceRightRail.sourcesListLabel": {
    zh: "资料列表",
    en: "Sources list",
  },
  "workspaceRightRail.notesListLabel": {
    zh: "笔记列表",
    en: "Notes list",
  },
  "workspaceRightRail.newNoteAction": {
    zh: "新建笔记",
    en: "New note",
  },
  "workspaceRightRail.saveNoteAction": {
    zh: "保存笔记",
    en: "Save note",
  },
  "workspaceRightRail.noteEditorToolbar": {
    zh: "笔记编辑工具栏",
    en: "Note editor toolbar",
  },
  "workspaceRightRail.newSourceAction": {
    zh: "添加内容源",
    en: "New Source",
  },
  "workspaceRightRail.addSourceTitle": {
    zh: "添加新资料",
    en: "Add New Source",
  },
  "workspaceRightRail.addSourceSubtitle": {
    zh: "选择文件、网页链接或粘贴内容来扩展当前工作区。",
    en: "Add a file, web link, or pasted content to this workspace.",
  },
  "workspaceRightRail.uploadFileTab": {
    zh: "上传文件",
    en: "Upload File",
  },
  "workspaceRightRail.webLinkTab": {
    zh: "网页链接",
    en: "Web Link",
  },
  "workspaceRightRail.pasteTextTab": {
    zh: "粘贴文本",
    en: "Paste Text",
  },
  "workspaceRightRail.uploadDropTitle": {
    zh: "拖拽文件到这里",
    en: "Drop files here",
  },
  "workspaceRightRail.uploadDropBody": {
    zh: "产品支持上传 {formats} 格式。",
    en: "Supported upload formats: {formats}.",
  },
  "workspaceRightRail.browseFilesAction": {
    zh: "浏览文件",
    en: "Browse Files",
  },
  "workspaceRightRail.addLinkAction": {
    zh: "添加链接",
    en: "Add Link",
  },
  "workspaceRightRail.pasteTitleLabel": {
    zh: "标题",
    en: "Title",
  },
  "workspaceRightRail.pasteContentLabel": {
    zh: "文本内容",
    en: "Text",
  },
  "workspaceRightRail.saveAsSourceAction": {
    zh: "保存为资料",
    en: "Save as Source",
  },
  "workspaceRightRail.untitledNote": {
    zh: "未命名笔记",
    en: "Untitled note",
  },
  "workspaceRightRail.emptyNotePreview": {
    zh: "还没有内容。",
    en: "No content yet.",
  },
  "workspaceRightRail.promotedNoteBadge": {
    zh: "已转换为来源",
    en: "Converted to source",
  },
  "workspaceRightRail.noteTitleLabel": {
    zh: "标题",
    en: "Title",
  },
  "workspaceRightRail.noteContentLabel": {
    zh: "内容",
    en: "Content",
  },
  "workspaceRightRail.idleState": {
    zh: "空闲",
    en: "Idle",
  },
  "workspaceRightRail.loadingSourcePreview": {
    zh: "正在加载资料预览...",
    en: "Loading source preview...",
  },
  "workspaceRightRail.viewerSectionLabel": {
    zh: "资料预览",
    en: "Source viewer",
  },
  "workspaceRightRail.closeViewerAction": {
    zh: "关闭",
    en: "Close",
  },
  "workspaceRightRail.citationFallbackTitle": {
    zh: "引用",
    en: "Citation",
  },
  "workspaceRightRail.viewerScore": {
    zh: "分数 {score}",
    en: "Score {score}",
  },
  "workspaceRightRail.viewerLocation": {
    zh: "第 {page} 页 · 游标 {cursor}",
    en: "Page {page} · Cursor {cursor}",
  },
  "workspaceRightRail.viewerPage": {
    zh: "第 {page} 页",
    en: "Page {page}",
  },
  "workspaceCitation.dialogLabel": {
    zh: "引用片段",
    en: "Citation chunk",
  },
  "workspaceCitation.chunkTitle": {
    zh: "Chunk 内容",
    en: "Chunk content",
  },
  "workspaceCitation.loading": {
    zh: "正在加载引用片段...",
    en: "Loading citation chunk...",
  },
  "workspaceCitation.empty": {
    zh: "当前引用没有可展示的文本内容。",
    en: "This citation does not include displayable text.",
  },
  "workspaceCitation.error": {
    zh: "加载引用片段失败。",
    en: "Failed to load citation chunk.",
  },
  "workspaceRightRail.selectNoteToEdit": {
    zh: "选择一条笔记后可在此直接编辑。",
    en: "Select a note to edit it in place.",
  },
  "workspaceRightRail.promoteNoteAction": {
    zh: "转换为来源",
    en: "Convert to source",
  },
  "workspaceRightRail.deleteNoteAction": {
    zh: "删除笔记",
    en: "Delete note",
  },
  "workspaceRightRail.sessionActionsLabel": {
    zh: "{title} 操作",
    en: "{title} actions",
  },
  "workspaceRightRail.resizePanelsLabel": {
    zh: "调整右侧面板大小",
    en: "Resize right rail panels",
  },
  "workspaceRightRail.sourceStatus.processing": {
    zh: "处理中",
    en: "processing",
  },
  "workspaceRightRail.sourceStatus.pending": {
    zh: "等待中",
    en: "pending",
  },
  "workspaceRightRail.sourceStatus.enqueueing": {
    zh: "入队中",
    en: "enqueueing",
  },
  "workspaceRightRail.sourceStatus.queued": {
    zh: "排队中",
    en: "queued",
  },
  "workspaceRightRail.sourceStatus.indexing": {
    zh: "索引中",
    en: "indexing",
  },
  "workspaceRightRail.sourceStatus.completed": {
    zh: "已完成",
    en: "completed",
  },
  "workspaceRightRail.sourceStatus.ready": {
    zh: "就绪",
    en: "ready",
  },
  "workspaceRightRail.sourceStatus.failed": {
    zh: "失败",
    en: "failed",
  },
  "workspaceRightRail.sourceStatus.error": {
    zh: "异常",
    en: "error",
  },
} satisfies Record<string, UiMessageDescriptor>;
