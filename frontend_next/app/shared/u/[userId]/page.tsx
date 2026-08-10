import type { Metadata } from "next";

import { SharedOwnerProfileSurface } from "../../../../components/share/shared-owner-profile-surface";

type Props = {
  params: Promise<{
    userId: string;
  }>;
};

/** Public sharer profile — noindex. */
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
  return [{ userId: "_placeholder" }];
}

export default async function SharedOwnerProfilePage({ params }: Props) {
  const { userId } = await params;
  return <SharedOwnerProfileSurface userId={userId} />;
}
