#!/bin/bash
# Attempt to demote bootstrap role 'avrag' by running initdb-style local peer
# auth as OS postgres user inside the container. Alpine postgres image runs
# everything as uid 70 (postgres); docker exec defaults to the image USER.
# If we can launch a NEW backend as OS user postgres (who owns PGDATA), peer
# auth will map us to a role named 'postgres' — which does not exist. Instead:
# single-user mode is the documented escape hatch.
set -euo pipefail
PGCMD=(docker exec avrag-postgres)

# Stop accepting app connections during the swap; workers retry on their own.
echo "NOTE: single-user demotion requires brief DB downtime. Proceeding."

# 1) graceful stop of app containers is done by caller.

# 2) stop postgres cleanly, then run single-user mode as OS postgres user.
${PGCMD[*]} pg_ctl -D /var/lib/postgresql/data -m fast stop || true
sleep 2
printf "ALTER ROLE avrag NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;\nALTER ROLE avrag NOINHERIT;\nselect 'demoted: ' || 'super=' || rolsuper::text from pg_roles where rolname='avrag';\n" \
  | ${PGCMD[*]} su postgres -c "postgres --single -D /var/lib/postgresql/data avrag_rs" 2>&1 | tail -4