#!/bin/sh
set -eu

API_BASE_URL="${API_BASE_URL:-}"
ESCAPED_API_BASE_URL=$(printf '%s' "$API_BASE_URL" | sed 's/\\/\\\\/g; s/"/\\"/g')

cat > /usr/share/nginx/html/config.js <<EOF
window.__FUMEN_CONFIG__ = {
  apiBaseUrl: "${ESCAPED_API_BASE_URL}"
};
EOF

exec nginx -g 'daemon off;'
