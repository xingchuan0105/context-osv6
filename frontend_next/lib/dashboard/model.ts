import { formatUiMessage } from "../i18n/messages";

export type DashboardLocale = "en" | "zh-CN";

export type DashboardTab = "all" | "mine" | "favorites";

export type DashboardSortMode = "recent" | "title";

export type DashboardStatusSummary = Partial<
  Record<
    | "ready"
    | "completed"
    | "pending"
    | "enqueueing"
    | "queued"
    | "processing"
    | "indexing"
    | "failed"
    | "error",
    number
  >
>;

export interface DashboardWorkspaceInput {
  id: string;
  title: string;
  name: string;
  description: string;
  createdAt: string;
  updatedAt: string;
  ownerId: string;
  statusSummary: DashboardStatusSummary;
}

export interface DashboardWorkspaceViewModel {
  id: string;
  title: string;
  description: string;
  dateLabel: string;
  roleLabel: string;
  statusSummaryLabel: string;
  isFavorite: boolean;
  isOwner: boolean;
}

export interface DashboardWorkspaceListStateOptions {
  locale: DashboardLocale;
  currentUserId: string;
  favoriteIds: readonly string[];
  tab?: DashboardTab;
  sort?: DashboardSortMode;
  query?: string;
}

export function formatDashboardWorkspaceDisplayTitle(workspace: DashboardWorkspaceInput) {
  const title = workspace.title.trim();
  return title.length > 0 ? title : workspace.name;
}

export function formatDashboardWorkspaceDescriptionLabel(
  locale: DashboardLocale,
  description: string,
) {
  const trimmed = description.trim();
  return trimmed.length > 0
    ? trimmed
    : formatUiMessage(locale, "dashboardEmptyDescription");
}

export function formatDashboardWorkspaceDateLabel(
  locale: DashboardLocale,
  isoString: string,
) {
  const datePart = isoString.split("T")[0] ?? isoString;
  const parts = datePart.split("-");
  if (parts.length < 3) {
    return isoString;
  }

  const [year, month, day] = parts;
  if (locale === "zh-CN") {
    const normalizedMonth = month.replace(/^0+/, "") || "0";
    const normalizedDay = day.replace(/^0+/, "") || "0";
    return `${year}年${normalizedMonth}月${normalizedDay}日`;
  }

  return `${year}-${month}-${day}`;
}

export function formatDashboardWorkspaceRoleLabel(
  locale: DashboardLocale,
  isOwner: boolean,
) {
  return formatUiMessage(locale, isOwner ? "dashboardRoleOwner" : "dashboardRoleMember");
}

export function formatDashboardWorkspaceStatusSummary(
  locale: DashboardLocale,
  statusSummary: DashboardStatusSummary,
) {
  const sum = (keys: readonly (keyof DashboardStatusSummary)[]) =>
    keys.reduce((total, key) => total + (statusSummary[key] ?? 0), 0);

  const ready = sum(["ready", "completed"]);
  const processing = sum(["pending", "enqueueing", "queued", "processing", "indexing"]);
  const failed = sum(["failed", "error"]);

  const parts: string[] = [];
  if (ready > 0) {
    parts.push(`${ready} ${formatUiMessage(locale, "dashboardStatusReady")}`);
  }
  if (processing > 0) {
    parts.push(`${processing} ${formatUiMessage(locale, "dashboardStatusProcessing")}`);
  }
  if (failed > 0) {
    parts.push(`${failed} ${formatUiMessage(locale, "dashboardStatusFailed")}`);
  }

  return parts.join(" · ");
}

function sortDashboardWorkspaces(
  workspaces: DashboardWorkspaceInput[],
  sort: DashboardSortMode,
) {
  return workspaces.sort((left, right) => {
    if (sort === "title") {
      return (
        formatDashboardWorkspaceDisplayTitle(left)
          .toLowerCase()
          .localeCompare(formatDashboardWorkspaceDisplayTitle(right).toLowerCase()) ||
        right.createdAt.localeCompare(left.createdAt) ||
        left.id.localeCompare(right.id)
      );
    }

    return (
      right.createdAt.localeCompare(left.createdAt) ||
      formatDashboardWorkspaceDisplayTitle(left)
        .toLowerCase()
        .localeCompare(formatDashboardWorkspaceDisplayTitle(right).toLowerCase()) ||
      left.id.localeCompare(right.id)
    );
  });
}

function matchesDashboardWorkspaceQuery(
  workspace: DashboardWorkspaceInput,
  query: string,
) {
  if (query.trim().length === 0) {
    return true;
  }

  const needle = query.trim().toLowerCase();
  const title = formatDashboardWorkspaceDisplayTitle(workspace).toLowerCase();
  const description = workspace.description.toLowerCase();

  return title.includes(needle) || description.includes(needle);
}

export function buildDashboardWorkspaceListState(
  workspaces: readonly DashboardWorkspaceInput[],
  options: DashboardWorkspaceListStateOptions,
) {
  const tab = options.tab ?? "all";
  const sort = options.sort ?? "recent";
  const query = options.query ?? "";

  const filtered = workspaces.filter((workspace) => {
    if (tab === "mine" && workspace.ownerId !== options.currentUserId) {
      return false;
    }

    if (tab === "favorites" && !options.favoriteIds.includes(workspace.id)) {
      return false;
    }

    return matchesDashboardWorkspaceQuery(workspace, query);
  });

  const sorted = sortDashboardWorkspaces([...filtered], sort);

  return sorted.map((workspace) => {
    const title = formatDashboardWorkspaceDisplayTitle(workspace);
    const isOwner = workspace.ownerId === options.currentUserId;

    return {
      id: workspace.id,
      title,
      description: formatDashboardWorkspaceDescriptionLabel(options.locale, workspace.description),
      dateLabel: formatDashboardWorkspaceDateLabel(options.locale, workspace.createdAt),
      roleLabel: formatDashboardWorkspaceRoleLabel(options.locale, isOwner),
      statusSummaryLabel: formatDashboardWorkspaceStatusSummary(
        options.locale,
        workspace.statusSummary,
      ),
      isFavorite: options.favoriteIds.includes(workspace.id),
      isOwner,
    };
  });
}
