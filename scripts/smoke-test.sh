#!/bin/bash
# FoxNIO 最小上线 Smoke Test
# 覆盖: health, models, auth, usage, admin dashboard

set -euo pipefail

BASE_URL="${1:-http://localhost:8080}"
PASS=0
FAIL=0
TOTAL=0

red='\033[0;31m'
green='\033[0;32m'
yellow='\033[1;33m'
nc='\033[0m'

check() {
    local name="$1"
    local method="$2"
    local url="$3"
    local expect_code="$4"
    local extra="${5:-}"

    TOTAL=$((TOTAL + 1))
    local args=(-s -o /tmp/smoke_body -w "%{http_code}" -X "$method")

    if [ -n "$extra" ]; then
        args+=(-H "Content-Type: application/json" -d "$extra")
    fi

    if [ -n "${TOKEN:-}" ]; then
        args+=(-H "Authorization: Bearer $TOKEN")
    fi

    local code
    code=$(curl "${args[@]}" "$url" 2>/dev/null) || code="000"

    # expect_code supports multiple codes separated by |
    local match=false
    IFS='|' read -ra codes <<< "$expect_code"
    for c in "${codes[@]}"; do
        if [ "$code" = "$c" ]; then
            match=true
            break
        fi
    done

    if $match; then
        echo -e "  ${green}PASS${nc}  [$code] $name"
        PASS=$((PASS + 1))
    else
        echo -e "  ${red}FAIL${nc}  [$code] $name (expected $expect_code)"
        cat /tmp/smoke_body 2>/dev/null | head -3
        echo ""
        FAIL=$((FAIL + 1))
    fi
}

echo "========================================"
echo " FoxNIO Smoke Test"
echo " Target: $BASE_URL"
echo "========================================"
echo ""

# --- 1. Health endpoints ---
echo "[Health]"
check "GET /health"           GET  "$BASE_URL/health"           200
check "GET /health/live"      GET  "$BASE_URL/health/live"      200
check "GET /health/ready"     GET  "$BASE_URL/health/ready"     200
check "GET /health/detailed"  GET  "$BASE_URL/health/detailed"  200
echo ""

# --- 2. Public API ---
echo "[Public API]"
check "GET /v1/models"        GET  "$BASE_URL/v1/models"        200
check "GET /metrics"          GET  "$BASE_URL/metrics"           200
echo ""

# --- 3. Auth ---
echo "[Auth]"
check "POST /auth/login (bad creds)" POST "$BASE_URL/api/v1/auth/login" 401 \
    '{"email":"smoke@test.invalid","password":"wrong"}'
check "POST /auth/register (validation)" POST "$BASE_URL/api/v1/auth/register" "400|422" \
    '{"email":"","password":""}'
echo ""

# --- 4. Protected endpoints (no token) ---
echo "[Protected - no token]"
check "GET /user/me (401)"    GET  "$BASE_URL/api/v1/user/me"   401
check "GET /user/usage (401)" GET  "$BASE_URL/api/v1/user/usage" 401
echo ""

# --- 5. Admin endpoints (no token) ---
echo "[Admin - no token]"
check "GET /admin/dashboard/stats (401)" GET "$BASE_URL/api/v1/admin/dashboard/stats" 401
echo ""

# --- Summary ---
echo "========================================"
if [ "$FAIL" -eq 0 ]; then
    echo -e " ${green}ALL PASSED${nc}: $PASS/$TOTAL"
else
    echo -e " ${red}FAILED${nc}: $FAIL/$TOTAL"
    echo -e " ${green}PASSED${nc}: $PASS/$TOTAL"
fi
echo "========================================"

rm -f /tmp/smoke_body
exit "$FAIL"
