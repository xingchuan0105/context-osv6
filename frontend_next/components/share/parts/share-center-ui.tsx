"use client";

export function SectionHeader({
  subtitle,
  title,
}: {
  subtitle: string;
  title: string;
}) {
  return (
    <div
      style={{
        borderBottom: "1px solid hsl(var(--border))",
        display: "grid",
        gap: "0.35rem",
        paddingBottom: "0.95rem",
      }}
    >
      <h2 className="app-page-title" style={{ fontSize: "1.2rem", marginBottom: 0 }}>
        {title}
      </h2>
      <p className="app-page-subtitle" style={{ margin: 0, maxWidth: "42rem" }}>
        {subtitle}
      </p>
    </div>
  );
}

export function InsightMetricCard({
  title,
  value,
}: {
  title: string;
  value: string;
}) {
  return (
    <section
      className="app-inline-surface"
      style={{
        display: "grid",
        gap: "0.55rem",
        minHeight: "clamp(5.8rem, 18vw, 7.25rem)",
        padding: "clamp(0.9rem, 2.5vw, 1rem) clamp(0.9rem, 2.5vw, 1rem) clamp(0.95rem, 2.5vw, 1.05rem)",
      }}
    >
      <h3
        style={{
          color: "hsl(var(--muted-foreground))",
          fontSize: "0.88rem",
          fontWeight: 600,
          letterSpacing: "-0.01em",
          margin: 0,
        }}
      >
        {title}
      </h3>
      <p
        style={{
          fontSize: "clamp(1.4rem, 4.8vw, 1.85rem)",
          fontWeight: 700,
          letterSpacing: "-0.03em",
          lineHeight: 1.05,
          margin: 0,
        }}
      >
        {value}
      </p>
    </section>
  );
}

export function shareStatusBadgeStyle(status: import("./share-center-utils").ShareStatus | null) {
  if (status === "active") {
    return {
      background: "hsl(var(--primary) / 0.12)",
      border: "1px solid hsl(var(--primary) / 0.18)",
      color: "hsl(var(--primary))",
    };
  }

  if (status === "expired") {
    return {
      background: "hsl(var(--destructive) / 0.1)",
      border: "1px solid hsl(var(--destructive) / 0.18)",
      color: "hsl(var(--destructive))",
    };
  }

  return {
    background: "hsl(var(--muted))",
    border: "1px solid hsl(var(--border))",
    color: "hsl(var(--muted-foreground))",
  };
}
