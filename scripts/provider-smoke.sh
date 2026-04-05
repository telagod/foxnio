#!/bin/bash
# FoxNIO Provider Smoke Test
# 需要: 1个用户 + 1个FoxNIO API Key + 真实 provider 密钥已配置到 backend
#
# 用法:
#   ./scripts/provider-smoke.sh <FOXNIO_API_KEY> [BASE_URL]
#
# 示例:
#   ./scripts/provider-smoke.sh sk-fox_abc123
#   ./scripts/provider-smoke.sh sk-fox_abc123 http://localhost:8080

set -euo pipefail

API_KEY="${1:-}"
BASE_URL="${2:-http://localhost:8080}"

if [ -z "$API_KEY" ]; then
    echo "Usage: $0 <FOXNIO_API_KEY> [BASE_URL]"
    exit 1
fi

PASS=0
FAIL=0
TOTAL=0
RESULTS_FILE="/tmp/provider_smoke_results.md"

red='\033[0;31m'
green='\033[0;32m'
yellow='\033[1;33m'
nc='\033[0m'

check() {
    local name="$1"
    local method="$2"
    local url="$3"
    local expect="$4"
    local data="${5:-}"

    TOTAL=$((TOTAL + 1))
    local args=(-s -o /tmp/smoke_body -w "%{http_code}" -X "$method" -m 30)
    args+=(-H "Authorization: Bearer $API_KEY")

    if [ -n "$data" ]; then
        args+=(-H "Content-Type: application/json" -d "$data")
    fi

    local code
    code=$(curl "${args[@]}" "$url" 2>/dev/null) || code="000"

    local match=false
    IFS='|' read -ra codes <<< "$expect"
    for c in "${codes[@]}"; do
        [ "$code" = "$c" ] && match=true && break
    done

    local body
    body=$(cat /tmp/smoke_body 2>/dev/null | head -c 200)

    if $match; then
        echo -e "  ${green}PASS${nc}  [$code] $name"
        echo "- PASS [$code] $name" >> "$RESULTS_FILE"
        PASS=$((PASS + 1))
    else
        echo -e "  ${red}FAIL${nc}  [$code] $name (expected $expect)"
        echo "  $body"
        echo "- FAIL [$code] $name (expected $expect): $body" >> "$RESULTS_FILE"
        FAIL=$((FAIL + 1))
    fi
}

echo "# Provider Smoke Results - $(date -Iseconds)" > "$RESULTS_FILE"

echo "========================================"
echo " FoxNIO Provider Smoke Test"
echo " Target: $BASE_URL"
echo " Key: ${API_KEY:0:12}..."
echo "========================================"
echo ""

# --- OpenAI ---
echo "[OpenAI - chat/completions]"
check "POST chat/completions (gpt-4)" POST "$BASE_URL/v1/chat/completions" "200" \
    '{"model":"gpt-4","messages":[{"role":"user","content":"Say hi in 3 words"}],"max_tokens":20}'

check "POST chat/completions (gpt-3.5-turbo)" POST "$BASE_URL/v1/chat/completions" "200" \
    '{"model":"gpt-3.5-turbo","messages":[{"role":"user","content":"Say hi"}],"max_tokens":10}'

check "POST chat/completions stream" POST "$BASE_URL/v1/chat/completions" "200" \
    '{"model":"gpt-4","messages":[{"role":"user","content":"Hi"}],"max_tokens":10,"stream":true}'
echo ""

echo "[OpenAI - responses]"
check "POST responses" POST "$BASE_URL/v1/responses" "200|404|501" \
    '{"model":"gpt-4","input":"Say hello"}'
echo ""

echo "[OpenAI - images]"
check "POST images/generations" POST "$BASE_URL/v1/images/generations" "200|402|501" \
    '{"model":"dall-e-3","prompt":"A red fox","n":1,"size":"1024x1024"}'
echo ""

echo "[OpenAI - prompt enhance]"
check "POST prompt enhance" POST "$BASE_URL/v1/chat/completions" "200" \
    '{"model":"gpt-4","messages":[{"role":"system","content":"Enhance this prompt"},{"role":"user","content":"draw a fox"}],"max_tokens":50}'
echo ""

# --- Gemini ---
echo "[Gemini - generateContent]"
check "POST generateContent" POST "$BASE_URL/v1/gemini/models/gemini-pro:generateContent" "200|502|404" \
    '{"contents":[{"parts":[{"text":"Say hi in 3 words"}]}]}'
echo ""

echo "[Gemini - streamGenerateContent]"
check "POST streamGenerateContent" POST "$BASE_URL/v1/gemini/models/gemini-pro:streamGenerateContent?alt=sse" "200|502|404" \
    '{"contents":[{"parts":[{"text":"Hi"}]}]}'
echo ""

echo "[Gemini - countTokens]"
check "POST countTokens" POST "$BASE_URL/v1/gemini/models/gemini-pro:countTokens" "200|502|404" \
    '{"contents":[{"parts":[{"text":"Count these tokens please"}]}]}'
echo ""

echo "[Gemini - embedContent]"
check "POST embedContent" POST "$BASE_URL/v1/gemini/models/embedding-001:embedContent" "200|502|404" \
    '{"content":{"parts":[{"text":"Embed this text"}]}}'
echo ""

# --- Summary ---
echo "========================================"
if [ "$FAIL" -eq 0 ]; then
    echo -e " ${green}ALL PASSED${nc}: $PASS/$TOTAL"
else
    echo -e " ${yellow}RESULTS${nc}: ${green}$PASS passed${nc}, ${red}$FAIL failed${nc} / $TOTAL total"
fi
echo " Results saved to: $RESULTS_FILE"
echo "========================================"

rm -f /tmp/smoke_body
exit "$FAIL"
