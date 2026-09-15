#!/usr/bin/env bash
set -euo pipefail

APP_NAME="bazzar"
APP_DIR="/var/www/bazzar"
SRC_DIR="/tmp/bazzar-src"

UNIT_DIR="$HOME/.config/systemd/user"
UNIT_FILE="$UNIT_DIR/${APP_NAME}.service"

echo "== [1/6] preparing application directory"

mkdir -p "$APP_DIR"
mkdir -p "$UNIT_DIR"

echo "== [2/6] preparing environment"

if [ ! -f "$APP_DIR/.env" ]; then
    if [ ! -f "$SRC_DIR/.env.example" ]; then
        echo "ERROR: .env.example not found"
        exit 1
    fi

    cp "$SRC_DIR/.env.example" "$APP_DIR/.env"

    echo "::warning:: No .env found."
    echo "::warning:: Copied .env.example to $APP_DIR/.env"
    echo "::warning:: You MUST configure production environment variables."
fi

# Generate a fresh JWT_SECRET on every deployment.
# This intentionally invalidates all previously issued JWTs.
BYTES=64

if command -v openssl >/dev/null 2>&1; then
    JWT_SECRET_NEW=$(
        openssl rand -base64 "$BYTES" |
        tr -d '\n' |
        tr '+/' '-_' |
        tr -d '='
    )
else
    JWT_SECRET_NEW=$(
        head -c "$BYTES" /dev/urandom |
        base64 |
        tr -d '\n' |
        tr '+/' '-_' |
        tr -d '='
    )
fi

if [ "${#JWT_SECRET_NEW}" -lt 43 ]; then
    echo "ERROR: generated JWT_SECRET too short" >&2
    exit 1
fi

if grep -q '^JWT_SECRET=' "$APP_DIR/.env"; then
    sed -i "s|^JWT_SECRET=.*|JWT_SECRET=${JWT_SECRET_NEW}|" "$APP_DIR/.env"
else
    echo "JWT_SECRET=${JWT_SECRET_NEW}" >> "$APP_DIR/.env"
fi

unset JWT_SECRET_NEW

chmod 600 "$APP_DIR/.env"

echo "Generated fresh JWT_SECRET for this deploy."
echo "All previously issued tokens are intentionally invalidated."

echo "== [3/6] updating application files"

# Stop existing service before replacing the running binary.
if systemctl --user is-active --quiet "$APP_NAME.service" 2>/dev/null; then
    echo "Stopping existing $APP_NAME.service..."
    systemctl --user stop "$APP_NAME.service" || true
fi

# Copy release artifacts.
cp -r "$SRC_DIR/." "$APP_DIR/"

chmod +x "$APP_DIR/bazzar-back"
chmod +x "$APP_DIR/sea-orm-cli"

echo "== [4/6] running database migrations"

cd "$APP_DIR"

set -a
source "$APP_DIR/.env"
set +a

if [ ! -x "$APP_DIR/sea-orm-cli" ]; then
    echo "ERROR: sea-orm-cli not found"
    exit 1
fi

if [ ! -d "$APP_DIR/migrations" ]; then
    echo "ERROR: migrations directory not found"
    exit 1
fi

"$APP_DIR/sea-orm-cli" migrate up

echo "Database migrations completed successfully."

echo "== [5/6] configuring systemd user service"

if [ ! -f "$SRC_DIR/deploy/bazzar.service" ]; then
    echo "ERROR: $SRC_DIR/deploy/bazzar.service not found"
    exit 1
fi

cp "$SRC_DIR/deploy/bazzar.service" "$UNIT_FILE"

# Keep user systemd manager alive after SSH logout/reboot.
loginctl enable-linger "$(id -un)" >/dev/null 2>&1 || true

systemctl --user daemon-reload
systemctl --user enable "$APP_NAME.service"

echo "Starting $APP_NAME.service..."
systemctl --user restart "$APP_NAME.service"

echo "== [6/6] verifying deployment"

if ! systemctl --user is-active --quiet "$APP_NAME.service"; then
    echo "ERROR: $APP_NAME.service failed to start."
    echo
    echo "---- systemd status ----"
    systemctl --user status "$APP_NAME.service" --no-pager || true
    echo
    echo "---- journal ----"
    journalctl --user \
        -u "$APP_NAME.service" \
        -n 50 \
        --no-pager || true

    exit 1
fi

echo
echo "========================================"
echo " Deployment successful"
echo "========================================"
echo "Application : $APP_NAME"
echo "Directory   : $APP_DIR"
echo "Binary      : $APP_DIR/bazzar-back"
echo "Migration   : $APP_DIR/sea-orm-cli"
echo "Service     : $APP_NAME.service"
echo "Status      : $(systemctl --user is-active "$APP_NAME.service")"
echo "========================================"

rm -rf "$SRC_DIR"