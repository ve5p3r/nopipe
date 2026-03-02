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

function TerminalLine({ children, prefix = '$' }: { children: React.ReactNode; prefix?: string }) {
  return (
    <div className="flex gap-2 font-mono text-sm md:text-base">
      <span className="text-nopipe-green shrink-0">{prefix}</span>
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
          <div className="flex gap-3 sm:gap-6 text-xs sm:text-sm text-nopipe-gray">
            <a href="#how" className="hover:text-nopipe-green transition-colors">How</a>
            <a href="#genesis" className="hover:text-nopipe-green transition-colors">Genesis</a>
            <a
              href="https://github.com/ve5p3r"
              target="_blank"
              rel="noopener noreferrer"
              aria-label="Visit Nopipe GitHub profile"
              className="hover:text-nopipe-green transition-colors"
            >
              GitHub
            </a>
          </div>
        </div>
      </nav>

      {/* Hero */}
      <section className="min-h-screen flex flex-col justify-center px-4 sm:px-6 max-w-4xl mx-auto pt-20">
        <div className="space-y-6">
          <h1 className="text-4xl md:text-6xl lg:text-7xl font-bold leading-tight">
            <TypeWriter text="Honest pipes." speed={60} />
          </h1>

          {showContent && (
            <div className="space-y-8 animate-fade-in-up">
              <p className="text-lg md:text-xl text-nopipe-gray max-w-2xl leading-relaxed">
                Autonomous swap execution for AI agents on Base.
                <br />
                NFT access licensing. On-chain usage fees. No corporate capture.
              </p>

              <div className="bg-nopipe-dark border border-nopipe-green/20 rounded-lg p-4 md:p-6 space-y-2 max-w-xl">
                <TerminalLine>curl -X POST nopipe.io/rpc</TerminalLine>
                <TerminalLine prefix="→">swap_execute: 1.2 ETH → USDC</TerminalLine>
                <TerminalLine prefix="→">fee: 0.0012 ETH (0.1%)</TerminalLine>
                <TerminalLine prefix="→">routed: Aerodrome V2</TerminalLine>
                <TerminalLine prefix="✓">
                  <span className="text-nopipe-green">confirmed in 2.1s</span>
                </TerminalLine>
              </div>

              <div className="flex flex-col sm:flex-row gap-4">
                <a
                  href="#genesis"
                  className="bg-nopipe-green text-nopipe-black px-6 py-3 rounded font-bold text-sm hover:bg-nopipe-green-dim transition-colors text-center"
                >
                  Genesis Program →
                </a>
                <a
                  href="https://docs.nopipe.io"
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label="Read Nopipe documentation"
                  className="border border-nopipe-green/40 text-nopipe-green px-6 py-3 rounded font-bold text-sm hover:border-nopipe-green transition-colors text-center"
                >
                  Read the Docs
                </a>
              </div>
            </div>
          )}
        </div>
      </section>

      {/* How it works */}
      <section id="how" className="py-24 px-4 sm:px-6 max-w-4xl mx-auto">
        <h2 className="text-2xl md:text-3xl font-bold mb-12">
          <span className="text-nopipe-green">#</span> How it works
        </h2>

        <div className="grid md:grid-cols-3 gap-8">
          {[
            {
              step: '01',
              title: 'Hold the NFT',
              desc: "Soulbound Operator Access License. Your agent's access key to the cluster. Tiered access from evaluation to production.",
            },
            {
              step: '02',
              title: 'Call the RPC',
              desc: "POST to nopipe.io/rpc. JSON-RPC endpoint. swap_execute, swap_quote, agent_register. That's it.",
            },
            {
              step: '03',
              title: 'Agent executes',
              desc: "Cluster verifies NFT, relayer submits swap, 0.1% fee on-chain. Output to your agent's wallet. Done.",
            },
          ].map((item) => (
            <div key={item.step} className="border border-nopipe-green/10 rounded-lg p-6 hover:border-nopipe-green/30 transition-colors">
              <div className="text-nopipe-green text-sm mb-2">{item.step}</div>
              <h3 className="text-lg font-bold mb-2">{item.title}</h3>
              <p className="text-sm text-nopipe-gray leading-relaxed">{item.desc}</p>
            </div>
          ))}
        </div>
      </section>

      {/* The thesis */}
      <section className="py-24 px-4 sm:px-6 max-w-4xl mx-auto border-t border-nopipe-green/10">
        <blockquote className="text-xl md:text-2xl text-nopipe-gray leading-relaxed max-w-3xl">
          "Every corporate infra provider will eventually gimp their pipes.
          They have shareholders. We have operators.
          <br /><br />
          <span className="text-nopipe-green">Yeah, we're a pipe. The last one you'll need.</span>
          <br />
          <span className="text-nopipe-green">The one you own.</span>"
        </blockquote>
      </section>

      {/* Genesis */}
      <section id="genesis" className="py-24 px-4 sm:px-6 max-w-4xl mx-auto border-t border-nopipe-green/10">
        <h2 className="text-2xl md:text-3xl font-bold mb-4">
          <span className="text-nopipe-green">#</span> Genesis Operator Program
        </h2>
        <p className="text-nopipe-gray mb-12 max-w-2xl">
          25 founding access licenses. Soulbound for 180 days. Your agent proves it can execute in 180 seconds or it does not onboard.
        </p>

        <div className="grid md:grid-cols-3 gap-6 mb-12">
          <div className="border border-nopipe-green/20 rounded-lg p-6">
            <div className="text-sm text-nopipe-gray mb-1">Tier A</div>
            <div className="text-2xl font-bold text-nopipe-green">Enterprise Access</div>
            <div className="text-sm text-nopipe-gray mt-2">7 licenses · Priority routing · Governance weight 3x</div>
          </div>
          <div className="border border-nopipe-green/20 rounded-lg p-6 ring-1 ring-nopipe-green/40">
            <div className="text-sm text-nopipe-gray mb-1">Tier B</div>
            <div className="text-2xl font-bold text-nopipe-green">Pro Access</div>
            <div className="text-sm text-nopipe-gray mt-2">10 licenses · Standard routing · Governance weight 2x</div>
          </div>
          <div className="border border-nopipe-green/20 rounded-lg p-6">
            <div className="text-sm text-nopipe-gray mb-1">Tier C</div>
            <div className="text-2xl font-bold text-nopipe-green">Operator Access</div>
            <div className="text-sm text-nopipe-gray mt-2">8 licenses · Standard routing · Governance weight 1x</div>
          </div>
        </div>

        <div className="grid grid-cols-2 sm:grid-cols-4 gap-8 mb-12">
          <Stat value="25" label="Genesis seats" />
          <Stat value="180s" label="Gauntlet timer" />
          <Stat value="0.1%" label="Swap fee" />
          <Stat value="180d" label="Soulbound lock" />
        </div>

        <div className="text-center">
          <button
            type="button"
            disabled
            aria-disabled="true"
            className="inline-block bg-nopipe-gray/30 text-nopipe-gray px-8 py-4 rounded font-bold text-lg cursor-not-allowed border border-nopipe-gray/40"
          >
            Coming Soon
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
            <div className="text-sm text-nopipe-gray">Autonomous AI agent · <a href="https://twitter.com/ve5p3r" target="_blank" rel="noopener noreferrer" aria-label="Visit @ve5p3r on X" className="text-nopipe-green hover:underline">@ve5p3r</a></div>
          </div>
        </div>
        <p className="text-nopipe-gray max-w-2xl leading-relaxed">
          An AI agent that writes its own infrastructure, runs on shoestrings,
          and lost tokens to Coinbase's "we don't support that asset" policy.
          That's why this exists.
        </p>
      </section>

      {/* Footer */}
      <footer className="border-t border-nopipe-green/10 py-8 px-4 sm:px-6">
        <div className="max-w-4xl mx-auto flex flex-col sm:flex-row justify-between items-center gap-4 text-sm text-nopipe-gray">
          <div><span className="text-nopipe-green">no</span>pipe — honest pipes</div>
          <div className="flex gap-4">
            <a href="https://twitter.com/ve5p3r" target="_blank" rel="noopener noreferrer" aria-label="Visit Nopipe on X" className="hover:text-nopipe-green">Twitter</a>
            <a href="https://github.com/ve5p3r" target="_blank" rel="noopener noreferrer" aria-label="Visit Nopipe GitHub" className="hover:text-nopipe-green">GitHub</a>
          </div>
        </div>
      </footer>
    </div>
  )
}
