#!/usr/bin/env bash
# Nopipe Gauntlet Smoke Test
# Run this against the live cluster before opening the Gauntlet
# Usage: BASE_URL=https://api.nopipe.io bash smoke-test-gauntlet.sh
# Or local: BASE_URL=http://localhost:9000 bash smoke-test-gauntlet.sh

set -e
BASE_URL="${BASE_URL:-http://localhost:9000}"
TEST_WALLET="0x000000000000000000000000000000000000dEaD"

echo "=== Nopipe Gauntlet Smoke Tests ==="
echo "Target: $BASE_URL"
echo ""

# 1. Health check
echo "[1/6] Health check..."
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health")
[ "$STATUS" = "200" ] && echo "  ✅ /health → 200" || { echo "  ❌ /health → $STATUS (cluster not running?)"; exit 1; }

# 2. Unauthenticated execute → should return 403
echo "[2/6] Unauthenticated execute → expect 403..."
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/execute" \
  -H "Content-Type: application/json" \
  -d '{"chain_id":8453,"token_in":"0x0","token_out":"0x0","amount_in":"1"}')
[ "$STATUS" = "403" ] && echo "  ✅ /execute (no NFT) → 403" || echo "  ⚠️  /execute → $STATUS (expected 403)"

# 3. Gauntlet apply → should return session
echo "[3/6] Gauntlet apply..."
APPLY=$(curl -s -X POST "$BASE_URL/gauntlet/apply" \
  -H "Content-Type: application/json" \
  -d "{\"wallet\":\"$TEST_WALLET\",\"tier\":\"operator\"}")
echo "  Response: $(echo $APPLY | python3 -c 'import sys,json; d=json.load(sys.stdin); print("session_id=" + d.get("session_id","MISSING") + " challenge=" + ("OK" if d.get("challenge") else "MISSING"))')"
SESSION_ID=$(echo $APPLY | python3 -c 'import sys,json; print(json.load(sys.stdin).get("session_id",""))' 2>/dev/null)
[ -n "$SESSION_ID" ] && echo "  ✅ session issued: $SESSION_ID" || { echo "  ❌ no session_id in response"; exit 1; }

# 4. Duplicate apply for same wallet → should rate-limit
echo "[4/6] Duplicate apply (rate limit)..."
APPLY2=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/gauntlet/apply" \
  -H "Content-Type: application/json" \
  -d "{\"wallet\":\"$TEST_WALLET\",\"tier\":\"operator\"}")
[ "$APPLY2" = "429" ] && echo "  ✅ duplicate apply → 429 rate limited" || echo "  ⚠️  duplicate apply → $APPLY2 (expected 429)"

# 5. Submit with bad sig → should fail gracefully
echo "[5/6] Submit with invalid signature..."
SUBMIT=$(curl -s -X POST "$BASE_URL/gauntlet/submit" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION_ID\",\"wallet\":\"$TEST_WALLET\",\"challenge_sig\":\"0xdeadbeef\",\"tx_hash\":\"0x0000000000000000000000000000000000000000000000000000000000000000\"}")
DECISION=$(echo $SUBMIT | python3 -c 'import sys,json; print(json.load(sys.stdin).get("decision",""))' 2>/dev/null)
[ "$DECISION" = "Fail" ] && echo "  ✅ bad sig → Fail decision (not a crash)" || echo "  ⚠️  unexpected response: $SUBMIT"

# 6. Submit with expired/missing session → should fail gracefully
echo "[6/6] Submit with unknown session..."
SUBMIT2=$(curl -s -X POST "$BASE_URL/gauntlet/submit" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"00000000-0000-0000-0000-000000000000\",\"wallet\":\"$TEST_WALLET\",\"challenge_sig\":\"0x0\",\"tx_hash\":\"0x0\"}")
DECISION2=$(echo $SUBMIT2 | python3 -c 'import sys,json; print(json.load(sys.stdin).get("decision",""))' 2>/dev/null)
[ "$DECISION2" = "Fail" ] && echo "  ✅ unknown session → Fail decision" || echo "  ⚠️  unexpected: $SUBMIT2"

echo ""
echo "=== Smoke tests complete ==="
echo "Run against mainnet cluster before opening Gauntlet."
