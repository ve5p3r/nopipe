#!/usr/bin/env bash
HOST="${HOST:-localhost:9000}"
URL="http://$HOST/rpc"
PASS=0; FAIL=0

rpc() {
  local method="$1" params="$2"
  curl -sf -X POST "$URL" -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":[$params],\"id\":1}"
}

check() {
  local name="$1" result="$2"
  if echo "$result" | grep -q '"result"'; then
    echo "PASS: $name"; ((PASS++))
  elif echo "$result" | grep -q '"error"'; then
    local code=$(echo "$result" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['error']['code'])" 2>/dev/null)
    if [[ "$code" == "-32001" || "$code" == "-32002" || "$code" == "-32003" ]]; then
      echo "PASS: $name (expected auth gate: $code)"; ((PASS++))
    else
      echo "FAIL: $name -> $result"; ((FAIL++))
    fi
  else
    echo "FAIL: $name -> no response"; ((FAIL++))
  fi
}

echo "Smoke testing $URL"
check "swap_quote" "$(rpc swap_quote '{"token_in":"0x4200000000000000000000000000000000000006","token_out":"0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913","amount_in":"1000000000000000000","router":"0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43"}')"
check "agent_register auth gate" "$(rpc agent_register '{"wallet":"0x0000000000000000000000000000000000000001","nonce":"n1","sig":"0x00"}')"
check "swap_execute auth gate" "$(rpc swap_execute '{"wallet":"0x0000000000000000000000000000000000000001","token_in":"0x4200000000000000000000000000000000000006","token_out":"0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913","amount_in":"1000000","router":"0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43","slippage_bps":50,"nonce":"n2","sig":"0x00"}')"

result=$(rpc "bad_method" '{}')
if echo "$result" | grep -q "\-32601"; then
  echo "PASS: unknown method -> -32601"; ((PASS++))
else
  echo "FAIL: unknown method -> $result"; ((FAIL++))
fi

echo "Results: $PASS passed, $FAIL failed"
[[ $FAIL -eq 0 ]]
