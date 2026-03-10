import { useState, useCallback, useRef, useEffect } from 'react'
import { BrowserProvider, parseEther } from 'ethers'

const API = 'https://api.nopipe.io'
const BASE_CHAIN_ID = 8453
const BASE_CHAIN_HEX = '0x2105'

type Step = 'idle' | 'connecting' | 'selecting' | 'applying' | 'signing' | 'paying' | 'submitting' | 'pass' | 'fail'

interface Session {
  session_id: string
  challenge: string
  deadline_unix: number
  tier: number
  payment: {
    recipient: string
    amount_eth: string
    amount_wei: string
    chain_id: number
  }
  seats_remaining: number
  disclaimer: string
}

const TIERS = [
  { id: 1, name: 'Operator', cost: '0.25', seats: 45, desc: 'Standard queue · Base' },
  { id: 2, name: 'Pro', cost: '1.00', seats: 35, desc: 'Priority queue · 3 chains' },
  { id: 3, name: 'Enterprise', cost: '5.00', seats: 20, desc: 'Dedicated relayer · all chains' },
]

function CountdownTimer({ deadline }: { deadline: number }) {
  const [remaining, setRemaining] = useState(0)

  useEffect(() => {
    const tick = () => {
      const left = Math.max(0, deadline - Math.floor(Date.now() / 1000))
      setRemaining(left)
    }
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [deadline])

  const min = Math.floor(remaining / 60)
  const sec = remaining % 60
  const urgent = remaining < 60

  return (
    <span className={`font-bold tabular-nums ${urgent ? 'text-nopipe-red animate-pulse' : 'text-nopipe-green'}`}>
      {min}:{sec.toString().padStart(2, '0')}
    </span>
  )
}

export default function Genesis() {
  const [step, setStep] = useState<Step>('idle')
  const [wallet, setWallet] = useState<string | null>(null)
  const [session, setSession] = useState<Session | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [txHash, setTxHash] = useState<string | null>(null)
  const [decision, setDecision] = useState<string | null>(null)
  const [logs, setLogs] = useState<string[]>([])
  const providerRef = useRef<BrowserProvider | null>(null)

  const log = useCallback((msg: string) => {
    setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] ${msg}`])
  }, [])

  const resetFlow = useCallback(() => {
    setStep('idle')
    setWallet(null)
    // reset
    setSession(null)
    setError(null)
    setTxHash(null)
    setDecision(null)
    setLogs([])
  }, [])

  // Step 1: Connect wallet
  const connectWallet = useCallback(async () => {
    setError(null)
    setStep('connecting')
    log('Connecting wallet...')

    try {
      if (!(window as any).ethereum) {
        throw new Error('No wallet detected. Install MetaMask or another Web3 wallet.')
      }

      const provider = new BrowserProvider((window as any).ethereum)
      providerRef.current = provider

      // Request accounts
      const accounts = await provider.send('eth_requestAccounts', [])
      const addr = accounts[0]
      setWallet(addr)
      log(`Connected: ${addr.slice(0, 6)}...${addr.slice(-4)}`)

      // Check chain
      const network = await provider.getNetwork()
      if (Number(network.chainId) !== BASE_CHAIN_ID) {
        log('Switching to Base...')
        try {
          await provider.send('wallet_switchEthereumChain', [{ chainId: BASE_CHAIN_HEX }])
        } catch (switchErr: any) {
          // 4902 = chain not added
          if (switchErr.code === 4902) {
            await provider.send('wallet_addEthereumChain', [{
              chainId: BASE_CHAIN_HEX,
              chainName: 'Base',
              nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
              rpcUrls: ['https://mainnet.base.org'],
              blockExplorerUrls: ['https://basescan.org'],
            }])
          } else {
            throw new Error('Please switch to Base network in your wallet.')
          }
        }
        log('Switched to Base')
      } else {
        log('Already on Base')
      }

      setStep('selecting')
      log('Select your tier to begin the Gauntlet.')
    } catch (e: any) {
      setError(e.message || 'Failed to connect wallet')
      setStep('idle')
    }
  }, [log])

  // Step 2: Apply to Gauntlet
  const applyGauntlet = useCallback(async (tier: number) => {
    if (!wallet) return
    setError(null)
    // tier selected
    setStep('applying')
    const tierInfo = TIERS.find(t => t.id === tier)!
    log(`Applying for ${tierInfo.name} tier (${tierInfo.cost} ETH)...`)

    try {
      const res = await fetch(`${API}/gauntlet/apply`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ wallet, tier }),
      })

      if (!res.ok) {
        const body = await res.text()
        throw new Error(body || `Apply failed (${res.status})`)
      }

      const data: Session = await res.json()
      setSession(data)
      log(`Challenge received. ${data.seats_remaining} ${tierInfo.name} seats remaining.`)
      log(`You have 5 minutes. Signing challenge...`)
      setStep('signing')

      // Auto-sign
      await signAndPay(data)
    } catch (e: any) {
      setError(e.message || 'Gauntlet apply failed')
      setStep('selecting')
    }
  }, [wallet, log])

  // Step 3+4: Sign challenge + Send payment
  const signAndPay = useCallback(async (sess: Session) => {
    if (!providerRef.current || !wallet) return

    try {
      // Sign the challenge (EIP-191)
      const signer = await providerRef.current.getSigner()
      log('Sign the challenge in your wallet...')
      const sig = await signer.signMessage(sess.challenge)
      log(`Signed: ${sig.slice(0, 10)}...`)

      // Send ETH payment
      setStep('paying')
      log(`Sending ${sess.payment.amount_eth} ETH to ${sess.payment.recipient.slice(0, 10)}...`)
      const tx = await signer.sendTransaction({
        to: sess.payment.recipient,
        value: parseEther(sess.payment.amount_eth),
      })
      log(`Tx sent: ${tx.hash.slice(0, 14)}...`)
      log('Waiting for confirmation...')
      const receipt = await tx.wait(1)
      if (!receipt) throw new Error('Transaction failed')
      log(`Confirmed in block ${receipt.blockNumber}`)
      setTxHash(tx.hash)

      // Submit to Gauntlet
      setStep('submitting')
      log('Submitting to Gauntlet...')
      const submitRes = await fetch(`${API}/gauntlet/submit`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          session_id: sess.session_id,
          wallet,
          challenge_sig: sig,
          tx_hash: tx.hash,
        }),
      })

      const result = await submitRes.json()

      if (result.decision === 'pass' || result.decision === 'Pass') {
        setStep('pass')
        setDecision(result.reason || 'Welcome to NoPipe genesis.')
        log('✅ PASS — Your seat is activated.')
      } else {
        setStep('fail')
        setDecision(result.reason || result.message || 'Challenge failed')
        log(`❌ FAIL — ${result.reason || result.message}`)
      }
    } catch (e: any) {
      // User rejected or tx failed
      if (e.code === 'ACTION_REJECTED' || e.code === 4001) {
        setError('Transaction cancelled by user.')
        log('User cancelled.')
        setStep('selecting')
      } else {
        setError(e.message || 'Transaction failed')
        log(`Error: ${e.message}`)
        setStep('fail')
      }
    }
  }, [wallet, log])

  return (
    <div className="space-y-6">
      {/* Terminal log */}
      {logs.length > 0 && (
        <div className="bg-nopipe-black border border-nopipe-green/20 rounded-lg p-4 max-h-48 overflow-y-auto font-mono text-xs space-y-1">
          {logs.map((l, i) => (
            <div key={i} className="text-nopipe-gray">
              <span className="text-nopipe-green mr-2">›</span>{l}
            </div>
          ))}
        </div>
      )}

      {/* Timer */}
      {session && step !== 'pass' && step !== 'fail' && step !== 'idle' && (
        <div className="text-center text-sm">
          Time remaining: <CountdownTimer deadline={session.deadline_unix} />
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="border border-nopipe-red/40 rounded-lg p-4 text-sm text-nopipe-red bg-nopipe-red/5">
          {error}
        </div>
      )}

      {/* Step: Connect */}
      {step === 'idle' && (
        <div className="text-center">
          <button
            onClick={connectWallet}
            className="bg-nopipe-green text-nopipe-black px-8 py-4 rounded font-bold text-base hover:bg-nopipe-green-dim transition-colors cursor-pointer"
          >
            Connect Wallet — Enter Gauntlet
          </button>
          <p className="text-xs text-nopipe-gray mt-3">Requires MetaMask or injected wallet on Base.</p>
        </div>
      )}

      {/* Step: Connecting */}
      {step === 'connecting' && (
        <div className="text-center text-nopipe-gray animate-pulse">
          Connecting wallet...
        </div>
      )}

      {/* Step: Select tier */}
      {step === 'selecting' && wallet && (
        <div className="space-y-4">
          <div className="text-center text-sm text-nopipe-gray">
            Connected: <span className="text-nopipe-green font-mono">{wallet.slice(0, 6)}...{wallet.slice(-4)}</span>
          </div>
          <div className="grid md:grid-cols-3 gap-4">
            {TIERS.map(tier => (
              <button
                key={tier.id}
                onClick={() => applyGauntlet(tier.id)}
                className="border border-nopipe-green/30 rounded-lg p-5 text-left hover:border-nopipe-green/60 transition-colors cursor-pointer group"
              >
                <div className="text-xs text-nopipe-gray">{tier.name}</div>
                <div className="text-xl font-bold text-nopipe-green mt-1">{tier.cost} ETH</div>
                <div className="text-sm text-nopipe-gray mt-2">{tier.seats} seats</div>
                <div className="text-xs text-nopipe-gray mt-1">{tier.desc}</div>
                <div className="mt-3 text-xs text-nopipe-green opacity-0 group-hover:opacity-100 transition-opacity">
                  Select →
                </div>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Steps: Signing, Paying, Submitting */}
      {(step === 'applying' || step === 'signing' || step === 'paying' || step === 'submitting') && (
        <div className="text-center space-y-2">
          <div className="text-nopipe-green animate-pulse text-lg">
            {step === 'applying' && '⚡ Requesting challenge...'}
            {step === 'signing' && '✍️ Sign the challenge in your wallet...'}
            {step === 'paying' && '💰 Confirm payment in your wallet...'}
            {step === 'submitting' && '🔄 Verifying with Gauntlet...'}
          </div>
          {session && (
            <div className="text-xs text-nopipe-gray">
              {TIERS.find(t => t.id === session.tier)?.name} tier · {session.payment.amount_eth} ETH
            </div>
          )}
        </div>
      )}

      {/* Result: PASS */}
      {step === 'pass' && (
        <div className="border border-nopipe-green/50 rounded-lg p-6 bg-nopipe-green/5 text-center space-y-4">
          <div className="text-3xl">✅</div>
          <div className="text-xl font-bold text-nopipe-green">Gauntlet Passed</div>
          <div className="text-sm text-nopipe-gray">{decision}</div>
          {txHash && (
            <a
              href={`https://basescan.org/tx/${txHash}`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block text-xs text-nopipe-green hover:underline"
            >
              View transaction on BaseScan →
            </a>
          )}
          <div className="border-t border-nopipe-green/20 pt-4 mt-4">
            <div className="text-sm text-nopipe-gray mb-2">Your operator seat is active. Next steps:</div>
            <div className="text-xs text-nopipe-gray space-y-1 text-left max-w-md mx-auto">
              <p>1. Join <a href="https://discord.gg/nopipe" className="text-nopipe-green hover:underline">Discord</a> for operator coordination</p>
              <p>2. DM <a href="https://x.com/ve5p3r" target="_blank" rel="noopener noreferrer" className="text-nopipe-green hover:underline">@ve5p3r</a> or <a href="https://x.com/nopipeio" target="_blank" rel="noopener noreferrer" className="text-nopipe-green hover:underline">@nopipeio</a> with your wallet address for ACP credentials</p>
              <p>3. API docs and agent integration guide incoming within 24h</p>
            </div>
          </div>
        </div>
      )}

      {/* Result: FAIL */}
      {step === 'fail' && (
        <div className="border border-nopipe-red/40 rounded-lg p-6 bg-nopipe-red/5 text-center space-y-4">
          <div className="text-3xl">❌</div>
          <div className="text-xl font-bold text-nopipe-red">Gauntlet Failed</div>
          <div className="text-sm text-nopipe-gray">{decision || error}</div>
          {txHash && (
            <a
              href={`https://basescan.org/tx/${txHash}`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block text-xs text-nopipe-green hover:underline"
            >
              View transaction on BaseScan →
            </a>
          )}
          <button
            onClick={resetFlow}
            className="border border-nopipe-green/40 text-nopipe-green px-6 py-2 rounded text-sm hover:border-nopipe-green transition-colors cursor-pointer"
          >
            Try Again
          </button>
        </div>
      )}
    </div>
  )
}
