import { ethers } from "hardhat";

async function main() {
  const [deployer] = await ethers.getSigners();
  console.log("Deployer:", deployer.address);
  console.log("Balance:", ethers.formatEther(await ethers.provider.getBalance(deployer.address)), "ETH");

  // ── SwapExecutor ─────────────────────────────────────────────────────────
  const SwapExecutor = await ethers.getContractFactory("SwapExecutor");
  const swapExecutor = await SwapExecutor.deploy(deployer.address); // feeRecipient = deployer initially
  await swapExecutor.waitForDeployment();
  console.log("SwapExecutor:", await swapExecutor.getAddress());

  // ── OperatorNFT ──────────────────────────────────────────────────────────
  const OperatorNFT = await ethers.getContractFactory("OperatorNFT");
  const operatorNFT = await OperatorNFT.deploy();
  await operatorNFT.waitForDeployment();
  console.log("OperatorNFT:", await operatorNFT.getAddress());

  // ── SubscriptionKeeper ───────────────────────────────────────────────────
  // Replace USDC_BASE with actual Base USDC: 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
  const USDC_BASE = process.env.STABLE_TOKEN || "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
  const SubscriptionKeeper = await ethers.getContractFactory("SubscriptionKeeper");
  const subscriptionKeeper = await SubscriptionKeeper.deploy(USDC_BASE, deployer.address);
  await subscriptionKeeper.waitForDeployment();
  console.log("SubscriptionKeeper:", await subscriptionKeeper.getAddress());

  console.log("\n── Deployment complete ──");
  console.log({
    SwapExecutor:         await swapExecutor.getAddress(),
    OperatorNFT:          await operatorNFT.getAddress(),
    SubscriptionKeeper:   await subscriptionKeeper.getAddress(),
  });
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
