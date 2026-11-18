#!/bin/sh
# T017 — one-command local e2e for the frontend suite.
#
# What it does:
#   1. Derives E2E_DATABASE_URL for the throwaway `kanpad_e2e` schema —
#      from the environment if set, else from ../backend-rs/.env (DB_HOST/
#      DB_PORT/DB_USERNAME/DB_PASSWORD; same algorithm as playwright.config.ts).
#   2. Creates `kanpad_e2e` if missing (utf8mb4 / utf8mb4_unicode_ci).
#   3. Applies migrations with the backend-rs binary (no sqlx-cli needed).
#   4. Runs Playwright (any extra args are passed through, e.g. `npm run test:e2e -- auth.spec.ts`).
#
# Prerequisites: a reachable MySQL/MariaDB (DB_HOST/DB_PORT from backend-rs/.env),
# the mysql client on PATH (to create the schema), and a built backend-rs
# (first `cargo run` compiles). Never touches the dev DB — the API under test
# always runs against kanpad_e2e with APP_ENV=test and throttling effectively
# unlimited (10000/min).
#
# Non-POSIX shells: run the steps above manually (see frontend/.env.example).
# CI does NOT use this script — the docker-e2e job manages its own container.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)   # frontend/scripts
frontend_dir=$(CDPATH= cd -- "$here/.." && pwd)     # frontend
root_dir=$(CDPATH= cd -- "$frontend_dir/.." && pwd) # repo root
db_env="$root_dir/backend-rs/.env"

# --- 1. derive E2E_DATABASE_URL -------------------------------------------
db_url=${E2E_DATABASE_URL:-}
if [ -z "$db_url" ]; then
  if [ ! -f "$db_env" ]; then
    echo "e2e: cannot derive DATABASE_URL — backend-rs/.env not found." >&2
    echo "     export E2E_DATABASE_URL='mysql://user:pass@host:port/kanpad_e2e?charset=utf8mb4&collation=utf8mb4_unicode_ci' and retry." >&2
    exit 1
  fi
  # shellcheck disable=SC1090
  db_host=$(grep -E '^DB_HOST=' "$db_env" | tail -1 | cut -d= -f2- | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  db_port=$(grep -E '^DB_PORT=' "$db_env" | tail -1 | cut -d= -f2- | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  db_user=$(grep -E '^DB_USERNAME=' "$db_env" | tail -1 | cut -d= -f2- | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  db_pass=$(grep -E '^DB_PASSWORD=' "$db_env" | tail -1 | cut -d= -f2- | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  db_port=${db_port:-3306}
  if [ -z "${db_host:-}" ] || [ -z "${db_user:-}" ] || [ -z "${db_pass:-}" ]; then
    echo "e2e: backend-rs/.env must define DB_HOST, DB_USERNAME, DB_PASSWORD." >&2
    exit 1
  fi
  # strip one layer of surrounding quotes if any
  db_user=$(printf '%s' "$db_user" | sed -e 's/^"\(.*\)"$/\1/' -e "s/^'\(.*\)'\$/\1/")
  db_pass=$(printf '%s' "$db_pass" | sed -e 's/^"\(.*\)"$/\1/' -e "s/^'\(.*\)'\$/\1/")
  enc_user=$(node -e 'process.stdout.write(encodeURIComponent(process.argv[1]))' "$db_user")
  enc_pass=$(node -e 'process.stdout.write(encodeURIComponent(process.argv[1]))' "$db_pass")
  db_url="mysql://$enc_user:$enc_pass@$db_host:$db_port/kanpad_e2e?charset=utf8mb4&collation=utf8mb4_unicode_ci"
fi

# DB coordinates for the mysql client (parsed from the URL, raw values)
db_host_raw=$(node -e 'const u=new URL(process.argv[1]);process.stdout.write(u.hostname)' "$db_url")
db_port_raw=$(node -e 'const u=new URL(process.argv[1]);process.stdout.write(u.port||"3306")' "$db_url")
db_user_raw=$(node -e 'const u=new URL(process.argv[1]);process.stdout.write(decodeURIComponent(u.username))' "$db_url")
db_pass_raw=$(node -e 'const u=new URL(process.argv[1]);process.stdout.write(decodeURIComponent(u.password))' "$db_url")

# --- 2. create kanpad_e2e if missing --------------------------------------
if ! command -v mysql >/dev/null 2>&1; then
  echo "e2e: mysql client not found on PATH — create the schema manually:" >&2
  echo "     CREATE DATABASE IF NOT EXISTS kanpad_e2e CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;" >&2
  exit 1
fi
MYSQL_PWD="$db_pass_raw" mysql -h"$db_host_raw" -P"$db_port_raw" -u"$db_user_raw" \
  -e "CREATE DATABASE IF NOT EXISTS kanpad_e2e CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"

# --- 3. migrate ------------------------------------------------------------
( cd "$root_dir/backend-rs" && \
  APP_ENV=test \
  SESSION_SECRET=e2e-session-secret-not-for-production-0123456789abcdef \
  DATABASE_URL="$db_url" \
  cargo run --locked -- migrate run )

# --- 4. run playwright -------------------------------------------------------
cd "$frontend_dir"
E2E_DATABASE_URL="$db_url" npx playwright test "$@"
