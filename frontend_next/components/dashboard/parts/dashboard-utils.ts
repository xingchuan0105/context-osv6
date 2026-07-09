import {
  type DashboardWorkspace,
} from "../../../lib/dashboard/client";
import {
  buildDashboardWorkspaceListState,
  type DashboardLocale,
  type DashboardWorkspaceInput,
} from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";

export type DashboardViewMode = "list" | "card";
export type DashboardWorkspaceView = ReturnType<typeof buildDashboardWorkspaceListState>[number];
export type DashboardWorkspaceItem = DashboardWorkspaceView & { documentCount: number };

export function mapWorkspace(workspace: DashboardWorkspace): DashboardWorkspaceInput {
  return {
    id: workspace.workspace_id,
    title: workspace.title,
    name: workspace.name,
    description: workspace.description,
    createdAt: workspace.created_at,
    updatedAt: workspace.updated_at,
    ownerId: workspace.owner_id,
    statusSummary: workspace.status_summary ?? {},
  };
}

export function formatWorkspaceTitle(locale: DashboardLocale, workspace: { title: string }) {
  const trimmedTitle = workspace.title.trim();
  return trimmedTitle.length > 0 ? trimmedTitle : formatUiMessage(locale, "dashboardUntitledWorkspace");
}

export function getWorkspaceGlyph(title: string) {
  const normalized = title.toLowerCase();

  if (normalized.includes("interview")) {
    return "\u{1F3A4}";
  }

  if (normalized.includes("power") || normalized.includes("market")) {
    return "\u26A1";
  }

  if (normalized.includes("prospectus") || normalized.includes("analysis") || normalized.includes("research")) {
    return "\u{1F50E}";
  }

  if (normalized.includes("logic") || normalized.includes("network") || normalized.includes("rag")) {
    return "\u{1F916}";
  }

  if (normalized.includes("framework") || normalized.includes("flow")) {
    return "\u{1F504}";
  }

  return "\u{1F4C4}";
}

export function getWorkspaceTone(title: string) {
  const tones = ["rose", "sage", "lavender", "amber"] as const;
  const normalized = title.trim().toLowerCase();
  let hash = 0;

  for (const char of normalized) {
    hash = (hash * 31 + char.charCodeAt(0)) % 9973;
  }

  return tones[hash % tones.length];
}

export function formatWorkspaceSourceCount(locale: DashboardLocale, documentCount: number) {
  if (locale === "zh-CN") {
    return `${documentCount} \u4e2a\u6765\u6e90`;
  }

  return `${documentCount} source${documentCount === 1 ? "" : "s"}`;
}