const SKELETON_CARD_COUNT = 6;

export function DashboardSkeleton() {
  return (
    <section aria-hidden="true" className="dashboard-grid dashboard-skeleton-grid">
      {Array.from({ length: SKELETON_CARD_COUNT }, (_, index) => (
        <div className="dashboard-skeleton-card" key={index}>
          <div className="dashboard-skeleton-icon" />
          <div className="dashboard-skeleton-copy">
            <div className="dashboard-skeleton-line dashboard-skeleton-line-title" />
            <div className="dashboard-skeleton-line dashboard-skeleton-line-meta" />
          </div>
        </div>
      ))}
    </section>
  );
}
