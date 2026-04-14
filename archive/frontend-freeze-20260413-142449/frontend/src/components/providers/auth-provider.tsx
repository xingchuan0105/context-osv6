"use client";

import { useEffect, useState } from "react";
import { authApi, getCachedAuthUser } from "@/lib/api/client";
import { useAppStore } from "@/stores/useAppStore";

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [loading, setLoading] = useState(true);
  const { setUser } = useAppStore();

  useEffect(() => {
    const cachedUser = getCachedAuthUser();
    if (cachedUser) {
      setUser(cachedUser);
    }

    const restoreSession = async () => {
      const response = await authApi.me();
      if (response.success && response.data?.user) {
        setUser(response.data.user);
      }
      setLoading(false);
    };

    void restoreSession();
  }, [setUser]);

  if (loading) {
    return (
      <div className="h-screen flex items-center justify-center bg-background">
        <div className="text-muted-foreground">加载中...</div>
      </div>
    );
  }

  return <>{children}</>;
}
