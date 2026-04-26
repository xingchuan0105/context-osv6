"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { acceptInvite, declineInvite } from "../../lib/share/client";
import { getWorkspace } from "../../lib/workspace/client";

type InviteSurfaceProps = {
  memberId: string;
  workspaceId: string;
};

type InviteDecision = "accepted" | "declined" | null;

export function InviteSurface({ memberId, workspaceId }: InviteSurfaceProps) {
  const auth = useAuth();
  const router = useRouter();
  const [loading, setLoading] = useState(true);
  const [actionLoading, setActionLoading] = useState(false);
  const [error, setError] = useState("");
  const [workspaceTitle, setWorkspaceTitle] = useState("");
  const [decision, setDecision] = useState<InviteDecision>(null);

  const nextPath = useMemo(
    () => encodeURIComponent(`/invite/${workspaceId}/${memberId}`),
    [memberId, workspaceId],
  );

  useEffect(() => {
    let cancelled = false;

    async function loadInvite() {
      if (!auth.initialized) {
        return;
      }

      if (!workspaceId.trim() || !memberId.trim()) {
        setError("邀请链接无效。");
        setLoading(false);
        return;
      }

      if (!auth.token) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");

      try {
        const response = await getWorkspace(auth.token, workspaceId);

        if (!cancelled) {
          setWorkspaceTitle(response.workspace.title || response.workspace.name || workspaceId);
        }
      } catch {
        if (!cancelled) {
          setWorkspaceTitle(workspaceId);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadInvite();

    return () => {
      cancelled = true;
    };
  }, [auth.initialized, auth.token, memberId, workspaceId]);

  async function handleAccept() {
    if (!auth.token) {
      router.push(`/login?next=${nextPath}`);
      return;
    }

    setActionLoading(true);
    setError("");

    try {
      await acceptInvite(auth.token, workspaceId, memberId);
      setDecision("accepted");
    } catch (acceptError) {
      setError(acceptError instanceof Error ? acceptError.message : "接受邀请失败。");
    } finally {
      setActionLoading(false);
    }
  }

  async function handleDecline() {
    if (!auth.token) {
      router.push(`/login?next=${nextPath}`);
      return;
    }

    setActionLoading(true);
    setError("");

    try {
      await declineInvite(auth.token, workspaceId, memberId);
      setDecision("declined");
    } catch (declineError) {
      setError(declineError instanceof Error ? declineError.message : "拒绝邀请失败。");
    } finally {
      setActionLoading(false);
    }
  }

  return (
    <main className="app-auth-shell">
      <section className="app-surface-card" style={{ display: "grid", gap: "1rem", maxWidth: "34rem" }}>
        {loading ? (
          <p style={{ margin: 0 }}>正在加载邀请...</p>
        ) : error ? (
          <>
            <h1 className="app-page-title" style={{ fontSize: "1.8rem", marginBottom: 0 }}>
              邀请异常
            </h1>
            <p className="app-notice-banner">{error}</p>
          </>
        ) : decision === "accepted" ? (
          <>
            <h1 className="app-page-title" style={{ fontSize: "1.8rem", marginBottom: 0 }}>
              已接受邀请
            </h1>
            <p className="app-page-subtitle">你现在可以访问 {workspaceTitle || "这个 Workspace"}。</p>
            <div className="app-button-row">
              <Link className="app-button-primary" href={`/dashboard/${workspaceId}`}>
                打开 Workspace
              </Link>
            </div>
          </>
        ) : decision === "declined" ? (
          <>
            <h1 className="app-page-title" style={{ fontSize: "1.8rem", marginBottom: 0 }}>
              已拒绝邀请
            </h1>
            <p className="app-page-subtitle">你已拒绝加入 {workspaceTitle || "这个 Workspace"}。</p>
            <div className="app-button-row">
              <Link className="app-button-secondary" href="/">
                返回首页
              </Link>
            </div>
          </>
        ) : (
          <>
            <h1 className="app-page-title" style={{ fontSize: "1.8rem", marginBottom: 0 }}>
              Workspace 邀请
            </h1>
            <p className="app-page-subtitle">
              {workspaceTitle
                ? `你被邀请加入 ${workspaceTitle}。`
                : "你被邀请加入一个 Workspace。登录后可接受或拒绝邀请。"}
            </p>

            {!auth.token ? (
              <div className="app-inline-surface" style={{ display: "grid", gap: "0.75rem" }}>
                <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
                  需要先登录或注册，才能处理这条邀请。
                </p>
                <div className="app-button-row">
                  <Link className="app-button-primary" href={`/login?next=${nextPath}`}>
                    登录后继续
                  </Link>
                  <Link className="app-button-secondary" href={`/register?next=${nextPath}`}>
                    注册后继续
                  </Link>
                </div>
              </div>
            ) : (
              <div className="app-button-row">
                <button className="app-button-primary" disabled={actionLoading} type="button" onClick={() => void handleAccept()}>
                  {actionLoading ? "处理中..." : "接受邀请"}
                </button>
                <button className="app-button-secondary" disabled={actionLoading} type="button" onClick={() => void handleDecline()}>
                  拒绝邀请
                </button>
              </div>
            )}
          </>
        )}
      </section>
    </main>
  );
}
