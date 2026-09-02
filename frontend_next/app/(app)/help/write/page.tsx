import type { Metadata } from "next";

import HelpWriteClient from "./help-write-client";

/** /help/write 长文属应用内帮助（app 壳页）；与 /help 同索引边界（Phase E E1.2-B）。 */
export const metadata: Metadata = {
  robots: { index: false, follow: true },
};

export default function HelpWritePage() {
  return <HelpWriteClient />;
}
