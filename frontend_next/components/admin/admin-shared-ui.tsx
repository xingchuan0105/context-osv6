"use client";

export function AdminPageHeading({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <header style={{ display: "grid", gap: "0.35rem", marginBottom: "1rem" }}>
      <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 2.5vw, 2.4rem)", lineHeight: 1.05 }}>{title}</h1>
      <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>{subtitle}</p>
    </header>
  );
}

export function AdminMetricCard({
  label,
  value,
  tone = "primary",
  detail,
}: {
  label: string;
  value: string;
  tone?: "primary" | "success" | "warning" | "danger";
  detail?: string;
}) {
  const palette =
    tone === "success"
      ? { dot: "hsl(var(--success))", value: "hsl(var(--success))" }
      : tone === "warning"
        ? { dot: "hsl(var(--warning))", value: "hsl(var(--warning))" }
        : tone === "danger"
          ? { dot: "hsl(var(--destructive))", value: "hsl(var(--destructive))" }
          : { dot: "hsl(var(--info))", value: "hsl(var(--foreground))" };

  return (
    <section className="app-inline-surface" style={{ display: "grid", gap: "0.6rem" }}>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "0.45rem", fontSize: "0.78rem", color: "hsl(var(--muted-foreground))" }}>
        <span style={{ width: "0.6rem", height: "0.6rem", borderRadius: "999px", background: palette.dot }} />
        <span>{label}</span>
      </div>
      <strong style={{ fontSize: "1.5rem", color: palette.value }}>{value}</strong>
      {detail ? <span style={{ fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>{detail}</span> : null}
    </section>
  );
}

export function LoadingState({ copy }: { copy: string }) {
  return (
    <section className="app-inline-surface" style={{ textAlign: "center", color: "hsl(var(--muted-foreground))" }}>
      {copy}
    </section>
  );
}

export function EmptyState({ copy }: { copy: string }) {
  return (
    <section
      className="app-inline-surface"
      style={{
        textAlign: "center",
        borderStyle: "dashed",
        color: "hsl(var(--muted-foreground))",
      }}
    >
      {copy}
    </section>
  );
}

export function ErrorState({ message }: { message: string }) {
  return <p className="app-notice-banner">{message}</p>;
}
