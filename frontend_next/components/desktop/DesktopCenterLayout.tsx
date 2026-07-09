import type { ReactNode } from "react";

import styles from "./desktop.module.css";

export function DesktopCenterLayout({ children }: { children: ReactNode }) {
  return (
    <div className={styles.shell}>
      <div className={styles.center}>{children}</div>
    </div>
  );
}
