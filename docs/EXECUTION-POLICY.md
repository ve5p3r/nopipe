# Nopipe Execution Policy

This policy defines the execution contract for `POST /execute`.

It exists to make failure handling deterministic for autonomous agents running capital in production.

## Scope

This document covers:
- Retry logic and backoff policy
- `POST /execute` failure codes and required agent actions
- Non-custodial execution guarantees and limits
- Refund policy
- SLA commitments
- Quote TTL requirements
- Nonce and idempotency behavior

## Retry Logic

Agents must implement retries client-side. Nopipe does not guarantee fill on first submission.

### Attempts

- Max attempts per order: `3` total (`1` initial + `2` retries)
- Use the same `client_order_id` for all retries of the same logical order
- Stop retrying immediately on hard-fail errors (see table)

### Backoff

- For `429 rate_limited`: wait `5s`, then retry
- For `503 cluster_unavailable`: exponential backoff `1s -> 2s` (cap `5s`)
- Add jitter of `+/-20%` to avoid synchronized retry storms

### Retry vs Hard Fail

Retry only when the request is valid but temporarily not executable:
- `402 payment_required` (complete x402 flow, retry with payment receipt)
- `429 rate_limited`
- `503 cluster_unavailable`

Treat as hard fail for the current quote/order:
- `403 operator_required`
- `409 quote_expired`

## `POST /execute` Failure Codes

| HTTP | Error Code | Operational Meaning | Agent Action |
|---|---|---|---|
| `402` | `payment_required` | Request accepted for pricing challenge; execution is gated until x402 payment proof is attached. | Pay the quoted amount on Base within expiry, then retry `POST /execute` with the same body and same `client_order_id` plus payment receipt header. |
| `403` | `operator_required` | Wallet is not authorized to execute (no valid Operator NFT / access gate not satisfied). | Do not retry this order. Complete Gauntlet/admission flow, then submit new orders after access is granted. |
| `409` | `quote_expired` | `quote_id` is invalid, consumed, or outside TTL. | Do not retry this quote. Request a fresh quote and submit a new execute call with a new `quote_id`. |
| `429` | `rate_limited` | Per-wallet or per-operator rate threshold reached. | Back off `5s` (with jitter) and retry with the same `client_order_id`. |
| `503` | `cluster_unavailable` | Executor is temporarily unavailable (degraded infra, upstream RPC failure, or maintenance window). | Retry with exponential backoff (`1s`, `2s`, max `5s`) using same `client_order_id`. Stop after max attempts. |

## Non-Custodial Guarantees

Nopipe is non-custodial:
- We relay execution; we do not custody user funds.
- Agents sign and authorize their own actions.
- We sponsor gas for execution infrastructure, but gas sponsorship is not a fill guarantee.

Operationally, "we relay, we don't guarantee fills" means:
- A valid request can still fail due to market movement, expiry, chain conditions, or infrastructure faults.
- Fill assurance is not implied by request acceptance or payment challenge issuance.
- Agents are responsible for retry logic, re-quoting, and order-level risk controls.

## Refund Policy

No refunds are issued for failed or expired submissions to `POST /execute`.

This includes failures caused by:
- Quote expiry
- Agent-side retry timing
- Rate limiting
- Temporary cluster unavailability
- Market movement between quote and execution

Agents own retry and re-quote behavior end-to-end.

## SLA

### Availability

- Monthly uptime target for execution API: `99.9%`

### Latency

Measured at API edge for `POST /execute` response time (request receipt to HTTP response), excluding on-chain settlement finality:
- `p50 <= 120ms`
- `p95 <= 400ms`

### If We Miss SLA

- We publish an incident summary with timestamps, cause, and remediation actions.
- No automatic refunds are issued for SLA misses under this policy.
- Enterprise contracts may define additional remedies in separate commercial terms.

## Quote TTL

`quote_id` expires in approximately `15 seconds` from quote issuance.

Agent requirements:
- Submit `POST /execute` within TTL.
- If TTL is exceeded, treat the quote as invalid and request a new quote.
- Do not retry an expired `quote_id`.

## Nonce and Idempotency

### `client_order_id` (Agent Idempotency Key)

- `client_order_id` identifies a single logical order intent.
- Retries for the same order must reuse the same `client_order_id`.
- New logical orders must use a new `client_order_id`.

### Replay Protection

- Payment nonce is single-use and replay-protected.
- Nopipe binds payment nonce and execution context to prevent duplicate execution on replayed receipts.
- Replayed or stale execution material is rejected rather than re-executed.

Idempotency + nonce checks together ensure safe retry behavior under packet loss, timeout, and duplicated submit scenarios.

---

*Nopipe Protocol — Execution Policy v1.0 — March 2026*
