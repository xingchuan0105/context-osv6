import type { ReactNode } from "react";

import { ChatShell } from "@/components/chat/chat-shell";

export default function ChatLayout({ children }: { children: ReactNode }) {
  return <ChatShell>{children}</ChatShell>;
}
