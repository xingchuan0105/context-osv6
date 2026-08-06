import type { UiMessageDescriptor } from "./types";

export const workspaceMessages = {
  workspaceDistribute: {
    zh: "分享",
    en: "Share",
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
  workspaceChatComposerLabel: {
    zh: "工作区对话输入框",
    en: "Workspace chat composer",
  },
  workspaceChatComposerPlaceholder: {
    zh: "输入问题，可开启知识库 / 网络搜索…",
    en: "Ask anything — toggle RAG / Search below…",
  },
  workspaceChatHeroTitle: {
    zh: "有什么想问的？",
    en: "What would you like to ask?",
  },
  workspaceChatHeroSubtitle: {
    zh: "直接提问，或开启知识库、网络搜索获取更深入的回答。",
    en: "Ask directly, or toggle RAG / web search for deeper answers.",
  },
  workspaceChatLoadError: {
    zh: "加载工作区对话记录失败。",
    en: "Failed to load workspace transcript.",
  },
  workspaceChatModeChat: {
    zh: "聊天",
    en: "Chat",
  },
  workspaceChatCapabilityLabel: {
    zh: "能力标签",
    en: "Capabilities",
  },
  workspaceChatCapRag: {
    zh: "知识库",
    en: "RAG",
  },
  workspaceChatCapRagNeedsSources: {
    zh: "先在右侧选择要检索的文档，再使用知识库检索",
    en: "Select documents in the right rail to enable RAG retrieval",
  },
  workspaceChatCapSearch: {
    zh: "网络搜索",
    en: "Search",
  },
  workspaceChatRegionLabel: {
    zh: "工作区对话",
    en: "Workspace chat",
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
  workspaceLanguageChinese: {
    zh: "中文",
    en: "Chinese",
  },
  workspaceLanguageEnglish: {
    zh: "English",
    en: "English",
  },
  workspaceNewThread: {
    zh: "新建会话",
    en: "New session",
  },
  workspaceChatBackToBottom: {
    zh: "回到底部",
    en: "Back to bottom",
  },
  workspaceEmptyStateModeHint: {
    zh: "当前：{mode} · 可开启知识库 / 网络搜索",
    en: "Active: {mode} · toggle RAG / Search below",
  },
  workspaceNoSessionsMatch: {
    zh: "暂无会话。",
    en: "No sessions yet.",
  },
  workspaceRenameSessionAction: {
    zh: "重命名",
    en: "Rename",
  },
  workspaceDeleteSessionAction: {
    zh: "删除",
    en: "Delete",
  },
  workspaceDeleteSessionDialogTitle: {
    zh: "删除会话",
    en: "Delete session",
  },
  workspaceDeleteSessionDialogBody: {
    zh: "确定删除会话「{title}」吗？此操作无法撤销。",
    en: "Delete \"{title}\"? This cannot be undone.",
  },
  workspaceUntitledSession: {
    zh: "新对话",
    en: "Untitled",
  },
  workspaceChatCodeCopy: {
    zh: "复制",
    en: "Copy",
  },
  workspaceChatCodeCopied: {
    zh: "已复制",
    en: "Copied",
  },
  workspaceChatActionCopied: {
    zh: "已复制",
    en: "Copied",
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
  workspaceSend: {
    zh: "发送",
    en: "Send",
  },
  workspaceChatStop: {
    zh: "停止",
    en: "Stop",
  },
  workspaceChatComposerResize: {
    zh: "调整输入框高度",
    en: "Resize composer",
  },
  workspaceSessionTitleField: {
    zh: "工作区标题",
    en: "Workspace title",
  },
  workspaceThreadTitleField: {
    zh: "会话标题",
    en: "Session title",
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
  workspaceRenameSessionFailed: {
    zh: "重命名会话失败，请重试。",
    en: "Failed to rename the session. Please try again.",
  },
  workspaceDeleteSessionFailed: {
    zh: "删除会话失败，请重试。",
    en: "Failed to delete the session. Please try again.",
  },
  workspaceResizeHistoryRailLabel: {
    zh: "调整历史栏宽度",
    en: "Resize history rail",
  },
  workspaceResizeRightRailLabel: {
    zh: "调整右侧栏宽度",
    en: "Resize right rail",
  },
  workspaceStreamError: {
    zh: "发送工作区对话失败。",
    en: "Failed to stream workspace chat.",
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
  "workspaceRightRail.viewerSummaryHeading": {
    zh: "文档摘要",
    en: "Document summary",
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
  "workspaceCitation.openSource": {
    zh: "跳转来源",
    en: "Open source",
  },
  "workspaceCitation.error": {
    zh: "加载引用片段失败。",
    en: "Failed to load citation chunk.",
  },
  "workspaceRightRail.selectNoteToEdit": {
    zh: "选择一条笔记后可在此直接编辑。",
    en: "Select a note to edit it in place.",
  },
  "workspaceProgressHeadingRag": {
    zh: "知识库检索中",
    en: "Knowledge Retrieval",
  },
  "workspaceProgressHeadingSearch": {
    zh: "网络搜索中",
    en: "Web Search",
  },
  "workspaceProgressToggleExpand": {
    zh: "展开过程",
    en: "Expand progress",
  },
  "workspaceProgressToggleCollapse": {
    zh: "收起过程",
    en: "Collapse progress",
  },
  "workspaceProgressStepExpand": {
    zh: "展开步骤：{title}",
    en: "Expand step: {title}",
  },
  "workspaceProgressStepCollapse": {
    zh: "收起步骤：{title}",
    en: "Collapse step: {title}",
  },
  "workspaceProgressThinking": {
    zh: "正在思考",
    en: "Thinking",
  },
  "workspaceProgressCompletedRag": {
    zh: "知识库检索",
    en: "Knowledge retrieval",
  },
  "workspaceProgressCompletedSearch": {
    zh: "网络搜索",
    en: "Web search",
  },
  "workspaceProgressCompletedThinking": {
    zh: "思考完成",
    en: "Thought process",
  },
  "workspaceProgressCountQueries": {
    zh: "查询",
    en: "queries",
  },
  "workspaceProgressCountResults": {
    zh: "结果",
    en: "results",
  },
  "workspaceProgressCountSources": {
    zh: "来源",
    en: "sources",
  },
  "workspaceProgressCountChunks": {
    zh: "片段",
    en: "chunks",
  },
  "workspaceProgressCountDocuments": {
    zh: "文档",
    en: "documents",
  },
  // Progress WorkFact keys (backend title = stable key; frontend localizes)
  "progress.understand": {
    zh: "正在理解问题",
    en: "Understanding your question",
  },
  "progress.reason_preview": {
    zh: "正在思考",
    en: "Thinking",
  },
  "progress.compose_answer": {
    zh: "正在整理回答",
    en: "Writing the answer",
  },
  "progress.delegate_rag": {
    zh: "正在检索工作区文档",
    en: "Retrieving workspace documents",
  },
  "progress.delegate_search": {
    zh: "正在检索网页",
    en: "Retrieving web sources",
  },
  "progress.memory": {
    zh: "正在回忆相关上下文",
    en: "Recalling relevant context",
  },
  "progress.retrieve_semantic.running": {
    zh: "正在语义检索",
    en: "Running semantic search",
  },
  "progress.retrieve_semantic.done": {
    zh: "完成语义检索",
    en: "Semantic search complete",
  },
  "progress.retrieve_semantic.empty": {
    zh: "未找到相关内容（语义检索）",
    en: "No relevant results (semantic search)",
  },
  "progress.retrieve_keyword.running": {
    zh: "正在关键词检索",
    en: "Running keyword search",
  },
  "progress.retrieve_keyword.done": {
    zh: "完成关键词检索",
    en: "Keyword search complete",
  },
  "progress.retrieve_keyword.empty": {
    zh: "未找到相关内容（关键词检索）",
    en: "No relevant results (keyword search)",
  },
  "progress.retrieve_graph.running": {
    zh: "正在关系检索",
    en: "Running relation search",
  },
  "progress.retrieve_graph.done": {
    zh: "完成关系检索",
    en: "Relation search complete",
  },
  "progress.retrieve_graph.empty": {
    zh: "未找到相关内容（关系检索）",
    en: "No relevant results (relation search)",
  },
  "progress.retrieve_doc.running": {
    zh: "正在阅读文档",
    en: "Reading documents",
  },
  "progress.retrieve_doc.done": {
    zh: "完成文档阅读",
    en: "Document read complete",
  },
  "progress.retrieve_doc.empty": {
    zh: "未找到相关文档内容",
    en: "No relevant document content",
  },
  "progress.search_web.running": {
    zh: "正在网页搜索",
    en: "Searching the web",
  },
  "progress.search_web.done": {
    zh: "完成网页搜索",
    en: "Web search complete",
  },
  "progress.search_web.empty": {
    zh: "未找到相关网页",
    en: "No relevant web results",
  },
  "progress.fetch_url.running": {
    zh: "正在读取网页",
    en: "Fetching page",
  },
  "progress.fetch_url.done": {
    zh: "完成读取网页",
    en: "Page fetch complete",
  },
  "progress.fetch_url.empty": {
    zh: "未能读取网页",
    en: "Could not fetch page",
  },
  "progress.write_research": {
    zh: "正在收集写作素材",
    en: "Gathering writing material",
  },
  "progress.write_outline": {
    zh: "正在规划文章大纲",
    en: "Planning article outline",
  },
  "progress.write_draft": {
    zh: "正在起草正文",
    en: "Drafting the article",
  },
  "progress.write_draft_section": {
    zh: "正在起草第 {section} 节",
    en: "Drafting section {section}",
  },
  "progress.write_refine": {
    zh: "正在润色修订",
    en: "Refining the draft",
  },
  "progress.write_refine_round": {
    zh: "正在润色第 {section} 轮",
    en: "Refine round {section}",
  },
  "progress.write_validate": {
    zh: "正在校验文稿",
    en: "Validating the draft",
  },
  "progress.fallback_unavailable": {
    zh: "补充检索不可用",
    en: "Fallback retrieval unavailable",
  },
  "progress.detail.query": {
    zh: "「{query}」",
    en: "“{query}”",
  },
  "progress.detail.queryWithHits": {
    zh: "「{query}」· 命中 {n} 条",
    en: "“{query}” · {n} hits",
  },
  "progress.detail.hitsOnly": {
    zh: "命中 {n} 条",
    en: "{n} hits",
  },
  "progress.detail.emptyQuery": {
    zh: "检索式「{query}」",
    en: "Query “{query}”",
  },
  "progress.reasonPreview": {
    zh: "思考摘要",
    en: "Reasoning summary",
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
