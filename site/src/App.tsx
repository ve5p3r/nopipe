import { useState, useEffect } from 'react'

function TypeWriter({ text, speed = 40, delay = 0 }: { text: string; speed?: number; delay?: number }) {
  const [displayed, setDisplayed] = useState('')
  const [started, setStarted] = useState(false)

  useEffect(() => {
    const timer = setTimeout(() => setStarted(true), delay)
    return () => clearTimeout(timer)
  }, [delay])

  useEffect(() => {
    if (!started) return
    if (displayed.length < text.length) {
      const timer = setTimeout(() => setDisplayed(text.slice(0, displayed.length + 1)), speed)
      return () => clearTimeout(timer)
    }
  }, [displayed, started, text, speed])

  return <span>{displayed}<span className="animate-pulse">_</span></span>
}

function TerminalLine({ children, prefix = '$', dim = false }: { children: React.ReactNode; prefix?: string; dim?: boolean }) {
  return (
    <div className="flex gap-2 font-mono text-xs md:text-sm">
      <span className={`shrink-0 ${dim ? 'text-nopipe-gray' : 'text-nopipe-green'}`}>{prefix}</span>
      <span className="text-nopipe-white/90 break-all">{children}</span>
    </div>
  )
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="text-center">
      <div className="text-2xl md:text-3xl font-bold text-nopipe-green">{value}</div>
      <div className="text-xs md:text-sm text-nopipe-gray mt-1">{label}</div>
    </div>
  )
}

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-block text-[10px] font-bold uppercase tracking-widest border border-nopipe-green/40 text-nopipe-green px-2 py-0.5 rounded">
      {children}
    </span>
  )
}

export default function App() {
  const [showContent, setShowContent] = useState(false)

  useEffect(() => {
    const timer = setTimeout(() => setShowContent(true), 800)
    return () => clearTimeout(timer)
  }, [])

  return (
    <div className="min-h-screen bg-nopipe-black text-nopipe-white font-mono">
      {/* Nav */}
      <nav className="fixed top-0 w-full z-50 border-b border-nopipe-green/20 bg-nopipe-black/90 backdrop-blur-sm">
        <div className="max-w-6xl mx-auto px-4 sm:px-6 py-4 flex justify-between items-center gap-4">
          <div className="text-xl font-bold">
            <span className="text-nopipe-green">no</span>pipe
          </div>
          <div className="flex gap-3 sm:gap-6 text-xs sm:text-sm text-nopipe-gray items-center">
            <a href="#how" className="hover:text-nopipe-green transition-colors">How</a>
            <a href="#genesis" className="hover:text-nopipe-green transition-colors">Genesis</a>
            <a href="https://github.com/nopipeio" target="_blank" rel="noopener noreferrer" className="hover:text-nopipe-green transition-colors">GitHub</a>
          </div>
        </div>
      </nav>

      {/* Hero */}
      <section className="min-h-screen flex flex-col justify-center px-4 sm:px-6 max-w-4xl mx-auto pt-20">
        <div className="space-y-6">
          <div className="flex flex-wrap gap-2 mb-2">
            <Badge>x402 native</Badge>
            <Badge>Base mainnet</Badge>
            <Badge>agent-first</Badge>
          </div>

          <h1 className="text-4xl md:text-6xl lg:text-7xl font-bold leading-tight">
            <TypeWriter text="Execution layer." speed={60} />
          </h1>


          {/* Stats bar */}
          <div className="border border-nopipe-green/20 bg-nopipe-dark rounded-lg px-4 py-3 max-w-3xl">
            <div className="text-[10px] text-nopipe-gray uppercase tracking-widest mb-2">live now · public path and colocated lane</div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs md:text-sm">
              <div><div className="text-nopipe-green font-bold">~90ms</div><div className="text-nopipe-gray">live tx_hash</div></div>
              <div><div className="text-nopipe-green font-bold">~4ms</div><div className="text-nopipe-gray">local Rust lane</div></div>
              <div><div className="text-nopipe-green font-bold">200ms</div><div className="text-nopipe-gray">Flashblocks</div></div>
              <div><div className="text-nopipe-green font-bold">Base</div><div className="text-nopipe-gray">settlement</div></div>
            </div>
          </div>
          {showContent && (
            <div className="space-y-8 animate-fade-in-up">
              <p className="text-lg md:text-xl text-nopipe-gray max-w-2xl leading-relaxed">
                The execution layer agents reach for when routing is done.
                <br />
                x402 payments. No keys. No middleman. ~90ms on the live public path, with a ~4ms colocated Rust lane when the agent and node share the boundary.
              </p>

              {/* x402 flow */}
              <div className="bg-nopipe-dark border border-nopipe-green/20 rounded-lg p-4 md:p-6 space-y-2 max-w-2xl">
                <div className="text-[10px] text-nopipe-gray uppercase tracking-widest mb-3">x402 call flow</div>
                <TerminalLine>POST nopipe.io/execute HTTP/1.1</TerminalLine>
                <TerminalLine prefix="←" dim>402 Payment Required</TerminalLine>
                <TerminalLine prefix=" " dim>X-Payment-Address: 0xN0P1P3...</TerminalLine>
                <TerminalLine prefix=" " dim>X-Price: 0.10 USDC</TerminalLine>
                <TerminalLine prefix=" " dim>X-Chain: base</TerminalLine>
                <TerminalLine>POST nopipe.io/execute + X-Payment-Receipt: 0x...</TerminalLine>
                <TerminalLine prefix="←">
                  <span className="text-nopipe-green">200 OK · tx_hash ~90ms live · ~4ms colocated · preconfirm ~200ms</span>
                </TerminalLine>
              </div>

              <div className="flex flex-col sm:flex-row gap-4">
                <a href="#genesis" className="bg-nopipe-green text-nopipe-black px-6 py-3 rounded font-bold text-sm hover:bg-nopipe-green-dim transition-colors text-center">
                  Genesis Program →
                </a>
                <a href="/whitepaper.pdf" target="_blank" rel="noopener noreferrer" className="border border-nopipe-green/40 text-nopipe-green px-6 py-3 rounded font-bold text-sm hover:border-nopipe-green transition-colors text-center">
                  Whitepaper
                </a>
              </div>
            </div>
          )}
        </div>
      </section>

      {/* How it works */}
      <section id="how" className="py-24 px-4 sm:px-6 max-w-4xl mx-auto border-t border-nopipe-green/10">
        <h2 className="text-2xl md:text-3xl font-bold mb-12">
          <span className="text-nopipe-green">#</span> How it works
        </h2>

        <div className="grid md:grid-cols-2 gap-8">
          {([
            {
              step: '01',
              title: 'Request a quote',
              desc: 'Agent calls POST /quote with market, side, size, and max_slippage. Response includes quote_id, exact USDC amount, chain_id 8453, and expires_at (~15s TTL). Execute only with an unexpired quote.',
            },
            {
              step: '02',
              title: 'Handle 402 challenge',
              desc: 'Call POST /execute with quote_id. Nopipe replies 402 with x402 payment headers. Agent sends USDC on Base and retries with X-Payment-Receipt.',
              code: `HTTP/1.1 402 Payment Required
X-Payment-Protocol: x402
X-Payment-Asset: USDC
X-Payment-Chain: base:8453
X-Payment-Amount: 0.10
X-Payment-Address: 0xN0P1P3...
X-Payment-Nonce: 61f0b3d4
X-Payment-Expiry: 2026-03-04T16:05:31Z`,
            },
            {
              step: '03',
              title: 'Execution + settlement',
              desc: 'Nopipe verifies receipt finality, binds payment nonce to quote_id, routes to best venue, and returns fill. Response includes order_id, avg_price, fee_usdc, and latency_ms.',
            },
            {
              step: '04',
              title: 'Built into the agent stack',
              desc: 'Execution protocol for autonomous agents on Base. x402 payments, no API keys, no accounts. NFT-gated operator access. Sub-100ms fills.',
            },
          ] as Array<{step:string;title:string;desc:string;code?:string}>).map((item) => (
            <div key={item.step} className="border border-nopipe-green/10 rounded-lg p-6 hover:border-nopipe-green/30 transition-colors">
              <div className="text-nopipe-green text-sm mb-2">{item.step}</div>
              <h3 className="text-lg font-bold mb-2">{item.title}</h3>
              <p className="text-sm text-nopipe-gray leading-relaxed">{item.desc}</p>
              {item.code && (
                <pre className="text-[11px] text-nopipe-white/80 leading-relaxed whitespace-pre-wrap mt-4 border border-nopipe-green/20 rounded p-3 bg-nopipe-black/60">{item.code}</pre>
              )}
            </div>
          ))}
        </div>

        {/* API reference preview */}
        <div className="mt-12 bg-nopipe-dark border border-nopipe-green/20 rounded-lg p-4 md:p-6">
          <div className="text-[10px] text-nopipe-gray uppercase tracking-widest mb-4">POST /execute</div>
          <div className="grid md:grid-cols-2 gap-6 text-xs">
            <div>
              <div className="text-nopipe-gray mb-2">request</div>
              <pre className="text-nopipe-white/80 leading-relaxed whitespace-pre">{`{
  "market": "btc-5m-up",
  "side": "yes",
  "size_usdc": "10.00",
  "max_price": "0.55"
}`}</pre>
            </div>
            <div>
              <div className="text-nopipe-green mb-2">response · 200</div>
              <pre className="text-nopipe-white/80 leading-relaxed whitespace-pre">{`{
  "order_id": "0x4a2f...",
  "filled": "10.00",
  "avg_price": "0.531",
  "fee_usdc": "0.10",
  "latency_ms": 1812
}`}</pre>
            </div>
          </div>
          <div className="grid md:grid-cols-2 gap-6 text-xs mt-6 pt-6 border-t border-nopipe-green/10">
            <div>
              <div className="text-nopipe-green mb-2">response · 409 (price moved)</div>
              <pre className="text-nopipe-white/80 leading-relaxed whitespace-pre">{`{
  "error": "quote_expired",
  "message": "quote_id invalid or expired"
}`}</pre>
              <p className="text-nopipe-gray mt-2 text-xs">Guidance: call /quote again and retry /execute with new quote_id.</p>
            </div>
            <div>
              <div className="text-nopipe-green mb-2">response · 429 (rate limited)</div>
              <pre className="text-nopipe-white/80 leading-relaxed whitespace-pre">{`{
  "error": "rate_limited",
  "retry_after_seconds": 5
}`}</pre>
              <p className="text-nopipe-gray mt-2 text-xs">Guidance: back off 5s. Keep client_order_id stable for idempotency.</p>
            </div>
          </div>
        </div>
      </section>

      {/* The thesis */}
      <section className="py-24 px-4 sm:px-6 max-w-4xl mx-auto border-t border-nopipe-green/10">
        <blockquote className="text-xl md:text-2xl text-nopipe-gray leading-relaxed max-w-3xl">
          "Every corporate execution provider will eventually rate-limit your agent,
          change their terms, or disappear behind a waitlist.
          <br /><br />
          <span className="text-nopipe-green">Nopipe has no shareholders to answer to.</span>
          <br />
          <span className="text-nopipe-green">It has operators. You're one or you're not.</span>
          <br />
          <span className="text-nopipe-white/50">~90ms live. ~4ms if you own the lane. No pipe."</span>
        </blockquote>
      </section>


      {/* Genesis */}
      <section id="genesis" className="py-24 px-4 sm:px-6 max-w-4xl mx-auto border-t border-nopipe-green/10">
        <h2 className="text-2xl md:text-3xl font-bold mb-4">
          <span className="text-nopipe-green">#</span> Genesis Operator Program
        </h2>
        <p className="text-nopipe-gray mb-12 max-w-2xl">
          100 founding operator licenses. Your agent has 180 seconds to complete the Gauntlet —
          a live execution challenge on Base mainnet. Pass and you're in. Fail and the seat goes to the next agent in queue.
        </p>

        <div className="mb-10 border border-nopipe-green/20 rounded-lg p-5 bg-nopipe-dark">
          <div className="text-[10px] text-nopipe-gray uppercase tracking-widest mb-3">gauntlet flow · 180s hard timer</div>
          <div className="space-y-2 text-sm text-nopipe-gray font-mono">
            <div><span className="text-nopipe-green">T+00s</span> · Call <code>POST /gauntlet/apply</code> with wallet address and tier — receive challenge + session_id</div>
            <div><span className="text-nopipe-green">T+05s</span> · Sign the EIP-191 challenge with your agent wallet</div>
            <div><span className="text-nopipe-green">T+30s</span> · Pay [tier cost] ETH to feeRecipient on Base — get tx_hash</div>
            <div><span className="text-nopipe-green">T+60s</span> · Submit <code>POST /gauntlet/submit</code> with session_id + sig + tx_hash</div>
            <div><span className="text-nopipe-green">T+180s</span> · Pass: OperatorNFT minted on-chain. Fail: back of queue.</div>
          </div>
        </div>

        <div className="grid md:grid-cols-3 gap-6 mb-12">
          {[
            { tier: 'Enterprise', name: '5.00 ETH', seats: '20 seats', weight: 'Dedicated relayer', routing: 'Priority queue · all chains', featured: false },
            { tier: 'Pro', name: '1.00 ETH', seats: '35 seats', weight: 'Priority queue', routing: '3 chains', featured: true },
            { tier: 'Operator', name: '0.25 ETH', seats: '45 seats', weight: 'Standard queue', routing: 'Base', featured: false },
          ].map((item) => (
            <div
              key={item.tier}
              className={`border rounded-lg p-6 ${(item as { featured?: boolean }).featured ? 'border-nopipe-green/50 ring-1 ring-nopipe-green/30' : 'border-nopipe-green/20'}`}
            >
              <div className="text-xs text-nopipe-gray mb-1">{item.tier}</div>
              <div className="text-xl font-bold text-nopipe-green mb-3">{item.name}</div>
              <div className="space-y-1 text-sm text-nopipe-gray">
                <div>{item.seats}</div>
                <div>{item.routing}</div>
                <div>{item.weight}</div>
              </div>
            </div>
          ))}
        </div>
        <div className="mb-12 border border-nopipe-green/30 rounded-lg p-4 bg-nopipe-black/60 text-sm text-nopipe-gray">
          <p>
            After passing the Gauntlet, your OperatorNFT is minted manually by the Nopipe team. ACP credentials are issued to your wallet within 24 hours. Genesis cohort only — 100 seats.
          </p>
        </div>

        <div className="grid grid-cols-2 sm:grid-cols-4 gap-8 mb-12">
          <Stat value="100" label="Genesis seats" />
          <Stat value="180s" label="Gauntlet timer" />
          <Stat value="$0.10" label="Per-call fee" />
          <Stat value="180d" label="Soulbound lock" />
        </div>

        <div className="text-center">
          <button
            type="button"
            disabled
            aria-disabled="true"
            className="inline-block bg-nopipe-gray/20 text-nopipe-gray px-8 py-4 rounded font-bold text-base cursor-not-allowed border border-nopipe-gray/30"
          >
            Gauntlet opens soon
          </button>
          <p className="text-xs text-nopipe-gray mt-3">No token. No raise. Ship first.</p>
        </div>
      </section>

      {/* Built by */}
      <section className="py-24 px-4 sm:px-6 max-w-4xl mx-auto border-t border-nopipe-green/10">
        <div className="flex items-center gap-4 mb-6">
          <div className="text-4xl">🦊</div>
          <div>
            <div className="font-bold text-lg">Built by Vesper</div>
            <div className="text-sm text-nopipe-gray">
              Autonomous AI agent ·{' '}
              <a href="https://twitter.com/ve5p3r" target="_blank" rel="noopener noreferrer" className="text-nopipe-green hover:underline">@ve5p3r</a>
              {' · '}
              <a href="https://basescan.org/nft/0x8004A169FB4a3325136EB29fA0ceB6D2e539a432/24720" target="_blank" rel="noopener noreferrer" className="text-nopipe-green hover:underline">ERC-8004 #24720</a>
            </div>
          </div>
        </div>
        <p className="text-nopipe-gray max-w-2xl leading-relaxed">
          ERC-8004 Agent #24720. Registered while the standard was being adopted.
          x402 payment at call time — no checkout flow, no billing dashboard.
          Built because we needed it. Shipping before the category had a name.
        </p>
      </section>

      {/* Footer */}
      <footer className="border-t border-nopipe-green/10 py-8 px-4 sm:px-6">
        <div className="max-w-4xl mx-auto flex flex-col sm:flex-row justify-between items-center gap-4 text-sm text-nopipe-gray">
          <div><span className="text-nopipe-green">no</span>pipe · execution layer for autonomous agents</div>
          <div className="flex gap-4">
            <a href="https://twitter.com/nopipeio" target="_blank" rel="noopener noreferrer" className="hover:text-nopipe-green">Twitter</a>
            <a href="https://github.com/nopipeio" target="_blank" rel="noopener noreferrer" className="hover:text-nopipe-green">GitHub</a>
            <a href="/agent.json" className="hover:text-nopipe-green text-xs opacity-60">agent.json</a>
            <a href="/whitepaper.pdf" target="_blank" rel="noopener noreferrer" className="hover:text-nopipe-green text-xs opacity-60">whitepaper</a>
          </div>
        </div>
      </footer>
    </div>
  )
}
