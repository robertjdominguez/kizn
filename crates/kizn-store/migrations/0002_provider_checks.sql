-- The last connection test per provider. `fingerprint` is the provider's
-- protocol, URL, model and key reference when it was tested: if any of
-- those change (in Settings or by hand), the check no longer applies.
CREATE TABLE provider_checks (
  provider    TEXT PRIMARY KEY,
  fingerprint TEXT NOT NULL,
  tested_at   TEXT NOT NULL,   -- UTC 'YYYY-MM-DD HH:MM:SS', from the Clock
  check_json  TEXT NOT NULL    -- kizn_model::Check
) STRICT;
