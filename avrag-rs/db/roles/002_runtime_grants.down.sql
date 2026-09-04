-- Down: revoke runtime-role grants. Never deletes the role itself (may own
-- active connections); use operator tooling if a full teardown is needed.

REVOKE ALL ON ALL TABLES IN SCHEMA public FROM avrag_runtime;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM avrag_runtime;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA public FROM avrag_runtime;
REVOKE USAGE ON SCHEMA public FROM avrag_runtime;
REVOKE CONNECT ON DATABASE CURRENT_DATABASE() FROM avrag_runtime;
REVOKE CREATE ON SCHEMA public FROM avrag;

ALTER DEFAULT PRIVILEGES FOR ROLE avrag IN SCHEMA public
    REVOKE ALL ON TABLES FROM avrag_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE avrag IN SCHEMA public
    REVOKE ALL ON SEQUENCES FROM avrag_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE avrag IN SCHEMA public
    REVOKE ALL ON TYPES FROM avrag_runtime;