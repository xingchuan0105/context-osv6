import type { Metadata } from "next";

import { SharedWorkspaceSurface } from "../../../../components/share/shared-workspace-surface";

type SharedWorkspacePageProps = {
  params: Promise<{
    token: string;
  }>;
};

/** ADR-0010 §9: shared KB pages must not enter search indexes (noindex, not robots Disallow alone). */
export const metadata: Metadata = {
  robots: {
    index: false,
    follow: false,
    googleBot: { index: false, follow: false },
  },
  other: {
    referrer: "no-referrer",
  },
};

export function generateStaticParams() {
  return [{ token: "_placeholder" }];
}

export default async function SharedWorkspacePage({ params }: SharedWorkspacePageProps) {
  const { token } = await params;
  return <SharedWorkspaceSurface shareToken={token} />;
}
