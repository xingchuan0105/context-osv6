-- Irreversible: org tenant removal. Restore from backup if needed.
DO $$
BEGIN
  RAISE EXCEPTION '0056_remove_org_tenant is irreversible; restore from backup';
END $$;
