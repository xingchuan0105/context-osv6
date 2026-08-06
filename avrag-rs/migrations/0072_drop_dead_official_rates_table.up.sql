-- Rates are loaded from code + PLATFORM_OFFICIAL_RATES_JSON (not this unused table).
-- Drop dead schema introduced in 0069 (repo rule: no unused paths).
DROP TABLE IF EXISTS platform_official_rates;
