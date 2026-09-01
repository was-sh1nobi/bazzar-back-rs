#!/usr/bin/env bash
set -euo pipefail

# Generate a cryptographically secure random JWT_SECRET for HS256
# - 64 bytes = 512 bits, base64-encoded (~86 chars url-safe without padding)
# - Meets requirement: >= 32 bytes (src/auth.rs warns if < 32)
# - Works with openssl or /dev/urandom + base64

BYTES=64

if command -v openssl >/dev/null 2>&1; then
  SECRET=$(openssl rand -base64 "$BYTES" | tr -d '\n' | tr '+/' '-_' | tr -d '=')
else
  # fallback: /dev/urandom + base64 (coreutils)
  SECRET=$(head -c "$BYTES" /dev/urandom | base64 | tr -d '\n' | tr '+/' '-_' | tr -d '=')
fi

# ensure length >= 43 (64 bytes base64url without padding is 86 chars)
if [ ${#SECRET} -lt 43 ]; then
  echo "error: generated secret too short" >&2
  exit 1
fi

# Output formats
echo "JWT_SECRET=$SECRET"
echo ""
echo "# Add to .env:"
echo "echo 'JWT_SECRET=$SECRET' >> .env"
echo ""
echo "# Or export for current shell:"
echo "export JWT_SECRET='$SECRET'"
