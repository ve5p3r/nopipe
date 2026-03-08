import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

// Local validation deploy — deploys MockERC20 as USDC stand-in
// DO NOT use on mainnet

const AERODROME_V2  = "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43";
const UNISWAP_V2   = "0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24";
const UNISWAP_V3   = "0x2626664c2603336E57B271c5C0b26F421741e481";

async function main() {
  const [deployer] = await ethers.getSigners();
  const feeRecipient = process.env.FEE_RECIPIENT || deployer.address;

  console.log("Deployer:", deployer.address);
  console.log("Fee recipient:", feeRecipient);

  // Mock USDC (6 decimals) for local testing
  console.log("\nDeploying MockUSDC...");
  const MockERC20 = await ethers.getContractFactory("MockERC20");
  const usdc = await MockERC20.deploy("USD Coin", "USDC", 6);
  await usdc.waitForDeployment();
  const usdcAddr = await usdc.getAddress();
  console.log("MockUSDC:", usdcAddr);

  // SwapExecutor
  console.log("\nDeploying SwapExecutor...");
  const SwapExecutor = await ethers.getContractFactory("SwapExecutor");
  const swapExecutor = await SwapExecutor.deploy(feeRecipient);
  await swapExecutor.waitForDeployment();
  const swapExecutorAddr = await swapExecutor.getAddress();
  console.log("SwapExecutor:", swapExecutorAddr);

  // SubscriptionKeeper (USDC as stable)
  console.log("\nDeploying SubscriptionKeeper...");
  const SubscriptionKeeper = await ethers.getContractFactory("SubscriptionKeeper");
  const keeper = await SubscriptionKeeper.deploy(usdcAddr, feeRecipient);
  await keeper.waitForDeployment();
  const keeperAddr = await keeper.getAddress();
  console.log("SubscriptionKeeper:", keeperAddr);

  // OperatorNFT
  console.log("\nDeploying OperatorNFT...");
  const OperatorNFT = await ethers.getContractFactory("OperatorNFT");
  const nft = await OperatorNFT.deploy();
  await nft.waitForDeployment();
  const nftAddr = await nft.getAddress();
  console.log("OperatorNFT:", nftAddr);

  const deployment = {
    network: "localhost",
    chainId: 31337,
    deployedAt: new Date().toISOString(),
    deployer: deployer.address,
    contracts: {
      MockUSDC: { address: usdcAddr },
      SwapExecutor: { address: swapExecutorAddr },
      SubscriptionKeeper: { address: keeperAddr },
      OperatorNFT: { address: nftAddr },
    }
  };

  fs.mkdirSync(path.join(__dirname, "../deployments"), { recursive: true });
  const outPath = path.join(__dirname, "../deployments/31337.json");
  fs.writeFileSync(outPath, JSON.stringify(deployment, null, 2));
  console.log("\n=== ADDRESSES ===");
  console.log("MOCK_USDC=" + usdcAddr);
  console.log("SWAP_EXECUTOR=" + swapExecutorAddr);
  console.log("SUBSCRIPTION_KEEPER=" + keeperAddr);
  console.log("OPERATOR_NFT=" + nftAddr);
  console.log("\nDeployment saved to deployments/31337.json");
}

main().catch((e) => { console.error(e); process.exit(1); });
