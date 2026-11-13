#!/usr/bin/env bash
# Verifies the auth behaviour of a running vipayment deployment.
#
#   ./scripts/test-auth.sh https://vipsdk.ttmi.my.id "$ADMIN_TOKEN"
#   ./scripts/test-auth.sh http://127.0.0.1:8089 "$ADMIN_TOKEN"
#
# Optional 3rd argument: base URL of an individual service (catalog or order,
# e.g. http://127.0.0.1:8081). When given, /internal/* is checked to require
# the x-internal-token header. On the public gateway /internal/* must be 404.
#
# Exits non-zero if any expectation fails, so it can be used in CI or a
# post-deploy check.
set -uo pipefail

BASE="${1:-http://127.0.0.1:8080}"
TOKEN="${2:-${ADMIN_TOKEN:-}}"
SERVICE="${3:-}"
BASE="${BASE%/}"
SERVICE="${SERVICE%/}"

fail=0

# expect <label> <expected-status> <curl args...>
expect() {
    local label="$1" want="$2"
    shift 2
    local got
    got=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "$@" 2>/dev/null)
    if [ "$got" = "$want" ]; then
        printf '  \033[32mPASS\033[0m %-46s %s\n' "$label" "$got"
    else
        printf '  \033[31mFAIL\033[0m %-46s got %s, want %s\n' "$label" "$got" "$want"
        fail=1
    fi
}

echo "Base URL: $BASE"
echo

echo "Public endpoints (no auth):"
expect "GET /api/v1/health" 200 "$BASE/api/v1/health"
expect "GET /api/v1/games" 200 "$BASE/api/v1/games"

echo
echo "Admin endpoints require a Bearer token:"
expect "GET /admin/orders  (no header)" 401 "$BASE/api/v1/admin/orders"
expect "GET /admin/orders  (wrong token)" 401 \
    -H "Authorization: Bearer definitely-not-the-token" "$BASE/api/v1/admin/orders"
expect "GET /admin/orders  (token without 'Bearer ')" 401 \
    -H "Authorization: ${TOKEN:-x}" "$BASE/api/v1/admin/orders"
expect "GET /admin/orders  (lowercase 'bearer')" 401 \
    -H "Authorization: bearer ${TOKEN:-x}" "$BASE/api/v1/admin/orders"

if [ -z "$TOKEN" ]; then
    echo
    echo "  SKIP  valid-token checks (pass ADMIN_TOKEN as the 2nd argument)"
else
    expect "GET /admin/orders  (valid token)" 200 \
        -H "Authorization: Bearer $TOKEN" "$BASE/api/v1/admin/orders"
    expect "GET /admin/profile (valid token)" 200 \
        -H "Authorization: Bearer $TOKEN" "$BASE/api/v1/admin/profile"
fi

echo
echo "Internal endpoints are not exposed on the public gateway:"
# The gateway has no /internal route, so this must 404 regardless of headers.
expect "POST /internal/orders/claim via gateway" 404 \
    -X POST "$BASE/internal/orders/claim"

if [ -n "$SERVICE" ]; then
    echo
    echo "Internal endpoints on a service require x-internal-token ($SERVICE):"
    expect "POST /internal/orders/claim (no header)" 401 \
        -X POST "$SERVICE/internal/orders/claim"
    expect "POST /internal/orders/claim (wrong header)" 401 \
        -X POST -H "x-internal-token: wrong-token" "$SERVICE/internal/orders/claim"
    expect "GET /internal/catalog/resolve (no header)" 401 \
        "$SERVICE/internal/catalog/resolve/x/y"
fi

echo
if [ "$fail" -ne 0 ]; then
    echo "RESULT: FAILED"
    exit 1
fi
echo "RESULT: OK"
