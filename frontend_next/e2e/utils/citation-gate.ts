/**
 * Citation assertion tier for journey specs that depend on external search.
 *
 * Set `E2E_TIER=nightly` or `E2E_TIER=staging` to require citations (hard gate).
 * Default PR journey runs use soft gates when Brave variability is acceptable.
 */
export function isHardCitationGate(): boolean {
  const tier = process.env.E2E_TIER?.toLowerCase();
  return tier === "nightly" || tier === "staging";
}
