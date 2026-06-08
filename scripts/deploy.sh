#!/usr/bin/env bash
# Manually deploy the API to the VPS: pull latest main, rebuild, restart.
# Usage:  ./scripts/deploy.sh        (uses the default host below)
#         VPS=root@1.2.3.4 ./scripts/deploy.sh
set -euo pipefail

VPS="${VPS:-root@185.197.250.205}"
APP="/opt/rewoven-api"

echo "-> deploying to $VPS:$APP"
ssh "$VPS" bash -lc "'
  set -e
  source \$HOME/.cargo/env 2>/dev/null || export PATH=\$HOME/.cargo/bin:\$PATH
  git config --global --add safe.directory $APP
  cd $APP
  git fetch origin
  git reset --hard origin/main
  cargo build --release           # if this fails, the running service is left untouched
  systemctl restart rewoven-api
  sleep 2
  systemctl is-active rewoven-api
'"
echo "-> done"
