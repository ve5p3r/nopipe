import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

const AERODROME_V2  = "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43";
const UNISWAP_V2   = "0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24";
const UNISWAP_V3   = "0x2626664c2603336E57B271c5C0b26F421741e481";

async function main() {
  const [deployer] = await ethers.getSigners();
  const feeRecipient = process.env.FEE_RECIPIENT || deployer.address;

  console.log("Deployer:", deployer.address);
  console.log("Fee recipient:", feeRecipient);
  console.log("Balance:", ethers.formatEther(await ethers.provider.getBalance(deployer.address)), "ETH");

  // 1. SwapExecutor
  console.log("\nDeploying SwapExecutor...");
  const SwapExecutor = await ethers.getContractFactory("SwapExecutor");
  const swapExecutor = await SwapExecutor.deploy(
    feeRecipient,
    [AERODROME_V2, UNISWAP_V2, UNISWAP_V3]
  );
  await swapExecutor.waitForDeployment();
  const swapExecutorAddr = await swapExecutor.getAddress();
  console.log("SwapExecutor:", swapExecutorAddr, "| tx:", swapExecutor.deploymentTransaction()?.hash);

  // 2. SubscriptionKeeper
  console.log("\nDeploying SubscriptionKeeper...");
  const SubscriptionKeeper = await ethers.getContractFactory("SubscriptionKeeper");
  const keeper = await SubscriptionKeeper.deploy(swapExecutorAddr, feeRecipient);
  await keeper.waitForDeployment();
  const keeperAddr = await keeper.getAddress();
  console.log("SubscriptionKeeper:", keeperAddr, "| tx:", keeper.deploymentTransaction()?.hash);

  // 3. OperatorNFT
  console.log("\nDeploying OperatorNFT...");
  const OperatorNFT = await ethers.getContractFactory("OperatorNFT");
  const nft = await OperatorNFT.deploy(
    "Polyclaw Operator License",
    "PCLW",
    "https://polyclaw.xyz/metadata/"
  );
  await nft.waitForDeployment();
  const nftAddr = await nft.getAddress();
  console.log("OperatorNFT:", nftAddr, "| tx:", nft.deploymentTransaction()?.hash);

  // Save deployment
  const deployment = {
    network: "base-sepolia",
    chainId: 84532,
    deployedAt: new Date().toISOString(),
    deployer: deployer.address,
    contracts: {
      SwapExecutor: {
        address: swapExecutorAddr,
        txHash: swapExecutor.deploymentTransaction()?.hash,
        args: { feeRecipient, routers: [AERODROME_V2, UNISWAP_V2, UNISWAP_V3] }
      },
      SubscriptionKeeper: {
        address: keeperAddr,
        txHash: keeper.deploymentTransaction()?.hash,
        args: { swapExecutor: swapExecutorAddr, feeRecipient }
      },
      OperatorNFT: {
        address: nftAddr,
        txHash: nft.deploymentTransaction()?.hash,
        args: { name: "Polyclaw Operator License", symbol: "PCLW" }
      }
    }
  };

  const outPath = path.join(__dirname, "../deployments/84532.json");
  fs.writeFileSync(outPath, JSON.stringify(deployment, null, 2));
  console.log("\nDeployment saved to deployments/84532.json");
  console.log("\n=== ADDRESSES ===");
  console.log("SWAP_EXECUTOR=" + swapExecutorAddr);
  console.log("SUBSCRIPTION_KEEPER=" + keeperAddr);
  console.log("OPERATOR_NFT=" + nftAddr);
}

main().catch((e) => { console.error(e); process.exit(1); });
