# Dashboard & Workspace Wiring Plan

> Scope: `dashboard` + `workspace` confirmed product decisions on 2026-04-15
> Product terminology: use `Workspace` in UI copy; `Notebook` remains a code/API compatibility term
> Purpose: define button destinations, state ownership, and API wiring before implementation

## 1. Wiring Principles

- Do not add fake backend capabilities just to satisfy UI copy.
- Prefer existing routes and SDK calls over new parallel transport helpers.
- Keep visual shell decisions in PRD; keep concrete button wiring in this document.
- Treat `dashboard` search as local quick-open, not as an AI search feature.
- Treat `workspace analyze` as a share analytics page only.
- Persist UI-only preferences locally unless a real account preference field already exists.

## 2. Existing Data and State Sources

### 2.1 SDK calls already available

- Workspaces
  - `list_notebooks()`
  - `create_notebook()`
  - `get_notebook()`
  - `update_notebook()`
  - `delete_notebook()`
  - `get_notebook_analysis()`
- Threads
  - `create_chat_session()`
  - `update_chat_session()`
  - `delete_chat_session()`
  - existing message load + SSE chat stream in workspace runtime
- Sources
  - `list_sources()`
  - `create_document_upload()`
  - file upload + `complete_upload()`
  - `add_url_source()`
  - existing source preview and citation focus runtime
- Notes
  - `list_notebook_notes()`
  - `create_notebook_note()`
  - `update_notebook_note()`
  - `delete_notebook_note()`
  - `promote_notebook_note()`
- Share
  - `get_share_settings()`
  - `update_share_settings()`
  - `create_share_with_options()`
  - `revoke_share()`
  - `list_members()`
  - `invite_member()`
  - `remove_member()`
  - `get_share_analytics()`
  - `get_access_logs()`
- API access
  - `list_api_keys()`
  - `create_api_key()`
  - `delete_api_key()`
- Account and preferences
  - `me()`
  - `logout()`
  - `get_user_preferences()`
  - `update_user_preferences()`
  - `get_subscription()`
- Appearance and locale
  - existing `use_ui_prefs_state()` with persisted `theme` and `locale`

### 2.2 Current persistence choices

- Favorites
  - source of truth: account preferences
  - current field: `preferences.dashboard.favorite_notebook_ids`
- Theme / locale
  - source of truth: local UI preference state in frontend
  - existing persistence: browser local storage + document root attributes
- Last chat mode
  - recommended source of truth: frontend local persistence only
  - reason: this is UI behavior, not business data
- Search modal query
  - ephemeral local state only

## 3. Route Targets

### 3.1 Product routes

- `/dashboard`
  - workspace home
- `/dashboard/:workspace_id`
  - main workspace
- `/dashboard/:workspace_id/analyze`
  - share analytics page
- `/dashboard/:workspace_id/share`
  - share center
- `/dashboard/:workspace_id/api-access`
  - API access
- `/settings?tab=appearance`
  - theme and language fallback destination
- `/settings?tab=profile`
  - profile fallback destination

### 3.2 Preview/live equivalents

- `/preview/live/dashboard`
- `/preview/live/workspace/:workspace_id`
- `/preview/live/workspace/:workspace_id/analyze`
- `/preview/live/workspace/:workspace_id/share`
- `/preview/live/workspace/:workspace_id/api-access`

Note:
- code and routes still use `notebook_id` in many places
- implementation can keep the existing path params short-term, but UI copy and PRD language should present them as `workspace_id`

## 4. Dashboard Wiring Matrix

| Control | User-facing function | Destination / state owner | SDK / state call |
|---|---|---|---|
| `Context-OS` logo | brand only | no-op | none |
| `设置` | open appearance settings | route | `/settings?tab=appearance` |
| avatar / account entry | open profile settings | route | `/settings?tab=profile` |
| `全部` tab | show all workspaces | local filter state | `active_tab = All` |
| `我的 Workspace` tab | show owned workspaces | local filter state | `active_tab = Mine` |
| `我的收藏` tab | show favorited workspaces only | local filter state | `active_tab = Favorites` |
| search trigger | open quick-open modal | local overlay state | `search_open = true` |
| search input | keyword filter workspaces | local derived state from loaded workspaces | filter `list_notebooks()` result by title/description |
| search result row | enter workspace | router navigation | `/dashboard/:workspace_id` |
| grid toggle | switch to card view | local UI state | `view_mode = Card` |
| list toggle | switch to list view | local UI state | `view_mode = List` |
| sort trigger | open sort menu | local UI state | `sort_menu_open = true/false` |
| sort `最近` | sort by recency | local UI state | `sort_by = Recent` |
| sort `标题` | sort by title | local UI state | `sort_by = Title` |
| `新建` | create new workspace | modal + API + navigate | `create_notebook()` then navigate |
| empty-state CTA `创建第一个 Workspace` | same as primary create | modal + API + navigate | same as `新建` |
| workspace card/row click | enter workspace | router navigation | `/dashboard/:workspace_id` |
| row/card menu `加入收藏` / `取消收藏` | update favorites | optimistic local state + account sync | `get_user_preferences()` + `update_user_preferences()` |
| row/card menu `重命名` | rename workspace | inline/prompt state + API | `update_notebook()` |
| row/card menu `删除` | delete workspace | confirm + API + refresh list | `delete_notebook()` |

### 4.1 Dashboard search modal definition

First milestone behavior:

- opens as a centered lightweight overlay
- one input field only
- one result list only
- no filter chips
- no pagination controls
- no AI answer area
- `Esc` closes
- click backdrop closes
- `Enter` on highlighted result enters the workspace

Recommended data flow:

1. use the already loaded dashboard dataset from `list_notebooks()`
2. derive a filtered result list in memory
3. if dashboard data is stale, refresh `list_notebooks()` before opening modal only when needed

## 5. Dashboard Empty State

### 5.1 Trigger conditions

Show the empty state when the active scope has zero visible workspaces:

- `全部`: total workspace count is zero
- `我的 Workspace`: user-owned workspace count is zero
- `我的收藏`: favorite workspace count is zero

### 5.2 CTA behavior

- label: `创建第一个 Workspace`
- behavior: exact same create flow as the main `新建` action
- no dedicated backend endpoint
- no branch-specific logic

## 6. Workspace Wiring Matrix

| Control | User-facing function | Destination / state owner | SDK / state call |
|---|---|---|---|
| workspace title | inline rename current workspace | local edit state + API | `get_notebook()` + `update_notebook()` |
| `New Workspace` | create workspace and enter it | modal + API + navigate | `create_notebook()` then navigate |
| `Analyze` | open workspace share analytics page | router navigation | `/dashboard/:workspace_id/analyze` |
| `Share` | open share center | router navigation | `/dashboard/:workspace_id/share` |
| `API` | open API access | router navigation | `/dashboard/:workspace_id/api-access` |
| gear trigger | open menu | local menu state | `gear_menu_open = true/false` |
| gear item `主题配色` | switch light/dark theme | UI prefs state | `use_ui_prefs_state().set_theme` |
| gear item `语言设置` | switch `zh-CN` / `en` | UI prefs state | `use_ui_prefs_state().set_locale` |
| avatar trigger | open account menu | local menu state | `avatar_menu_open = true/false` |
| avatar item `账号信息` | open profile page | route | `/settings?tab=profile` |
| avatar item `用户标识` | show current plan badge | derived account state | `get_subscription()` fallback `Free` |
| avatar item `退出登录` | sign out current user | auth state + route | `logout()` + auth clear + navigate `/login` |
| `New Thread` | create a new thread in current workspace | local pending state + API | `create_chat_session()` |
| thread search input | filter thread list | local filter state | client-side filter over loaded sessions |
| thread row click | open thread transcript | workspace runtime state | existing message/session load flow |
| thread menu `置顶/取消置顶` | prioritize thread | optimistic local state + API | `update_chat_session(pinned)` |
| thread menu `重命名` | rename thread | local prompt/edit state + API | `update_chat_session(title)` |
| thread menu `删除` | delete thread | confirm + API + refresh | `delete_chat_session()` |
| `New Source` | open add-source modal | local modal state | source modal open |
| source modal `上传文件` | create upload and ingest file | API workflow | `create_document_upload()` + upload + `complete_upload()` |
| source modal `链接` | ingest URL | API workflow | `add_url_source()` |
| `Select all` | select all usable sources for RAG scope | local workspace state | set `selected_source_ids` |
| source checkbox | include/exclude source from scope | local workspace state | update `selected_source_ids` |
| source row | open source detail | local workspace state + preview load | existing preview/citation runtime |
| `New Note` | create note | local pending state + API | `create_notebook_note()` |
| note card | open note editor | local workspace state | set active note |
| note editor save | persist note changes | debounced local state + API | `update_notebook_note()` |
| note editor `Promote to Source` | turn note into source | API + source refresh | `promote_notebook_note()` |
| note editor `Delete` | remove note | confirm + API | `delete_notebook_note()` |
| note editor `Export Markdown` | download note markdown | client-only export | local blob download |
| mode switcher | change answer mode | local persisted UI state | set `agent_mode` |
| send button | submit current prompt | chat runtime | existing SSE chat request |
| composer `Enter` | submit prompt | keyboard binding | same as send button |
| composer `Shift+Enter` | insert newline | keyboard binding | prevent submit |
| assistant message `Copy` | copy answer text | client-only action | clipboard helper |
| user message `Edit` | restore previous prompt to composer | local state | set input text |
| assistant message `Add to Note` | append answer to note | local note state + API | create/update note workflow |
| assistant message `Regenerate` | rerun last user turn | chat runtime | existing SSE chat request |
| citation pill | open evidence and focus source | workspace state | existing citation focus runtime |

## 7. Workspace Default Naming Mechanism

### 7.1 Product rule

If the user creates a workspace without entering a name, the frontend must send a deterministic localized default title.

### 7.2 Proposed naming algorithm

- locale `zh-CN`
  - `未命名 Workspace YYYY-MM-DD`
- locale `en`
  - `Untitled Workspace YYYY-MM-DD`
- duplicate handling on the same day
  - append `·2`, `·3`, ... after checking the current list result from `list_notebooks()`

### 7.3 Wiring point

- apply before `create_notebook()` request is sent
- after creation, allow immediate inline rename in the workspace top bar

## 8. Workspace Analyze Wiring

### 8.1 Scope

Analyze is intentionally narrow.

It only shows workspace share analytics. It does not show:

- token usage
- LLM cost views
- global search
- global product analytics

### 8.2 Data sources

- `get_share_settings(workspace_id)`
- `get_share_analytics(workspace_id)`
- `get_access_logs(workspace_id)`

### 8.3 Page blocks

1. Share status
- access level
- allow download
- expires at
- whether sharing is enabled

2. KPI cards
- total views
- total unique visitors

3. Trend chart
- `views_by_day`

4. Recent access logs
- accessed_at
- visitor_id
- action

### 8.4 Empty state

Trigger when sharing is effectively disabled.

- title: explain that share analytics are available after sharing is enabled
- primary CTA: `前往 Share`
- CTA destination: `/dashboard/:workspace_id/share`

## 9. Theme, Language, and Mode Persistence

### 9.1 Theme and language

- use existing `use_ui_prefs_state()`
- keep persistence in local browser storage
- gear menu becomes a direct access point to these controls
- full settings page remains the fallback destination for deeper explanation

### 9.2 Last chat mode

Recommended implementation:

- persist `last_agent_mode_by_workspace_id` in frontend local storage
- initialize each workspace composer from:
  1. stored workspace-specific last mode
  2. otherwise `RAG`
- update stored value only when the user explicitly changes mode

Reason:

- satisfies product behavior
- avoids backend contract change
- keeps mode memory a UI preference instead of business state

## 10. Account Badge Rule

The avatar menu includes a user badge line.

Data source priority:

1. `get_subscription()`
   - `plan_id` maps to product-facing badge such as `Free` / `VIP`
2. fallback when unavailable
   - show `Free`

This avoids inventing a new auth field just for the menu badge.

## 11. Recommended Execution Order

1. Dashboard scope and wording cleanup
- switch visible copy from `Notebook` to `Workspace`
- add `我的收藏`
- keep favorites synced through account preferences

2. Dashboard search modal
- replace route jump with in-place quick-open overlay
- keep dataset local to dashboard load result

3. Dashboard create and empty-state convergence
- ensure primary `新建` and empty-state CTA reuse the same code path

4. Workspace top-bar destinations
- wire `Share`, `Analyze`, `API`
- wire gear and avatar menus

5. Workspace input behavior
- add keyboard behavior
- add `RAG` default and last-mode persistence

6. Workspace analyze page
- ship share analytics only
- keep empty state routed into share center

7. Default naming pass
- apply localized untitled naming before create request

## 12. Non-goals for This Wiring Slice

- no new backend telemetry for workspace token accounting
- no standalone `/dashboard/search` product page
- no AI-powered search result explanation in dashboard search modal
- no multi-feature mega menu inside gear or avatar controls
