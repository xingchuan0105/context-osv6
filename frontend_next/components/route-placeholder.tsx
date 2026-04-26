import { AppPageFrame } from "./page-frame";

type RoutePlaceholderProps = {
  title: string;
  subtitle: string;
  bullets?: string[];
};

export function RoutePlaceholder({
  title,
  subtitle,
  bullets = [],
}: RoutePlaceholderProps) {
  return (
    <AppPageFrame title={title} subtitle={subtitle}>
      <section className="app-surface-card">
        {bullets.length > 0 ? (
          <ul style={{ margin: 0, paddingLeft: "1.1rem" }}>
            {bullets.map((bullet) => (
              <li key={bullet} style={{ marginBottom: "0.4rem" }}>
                {bullet}
              </li>
            ))}
          </ul>
        ) : (
          <p style={{ margin: 0 }}>Placeholder route. Wiring and page-specific UI come next.</p>
        )}
      </section>
    </AppPageFrame>
  );
}
