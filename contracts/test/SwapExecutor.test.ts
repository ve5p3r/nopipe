import { expect } from "chai";
import { ethers } from "hardhat";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/signers";
import { SwapExecutor } from "../typechain-types";

// ── Minimal mock router ─────────────────────────────────────────────────────
// We test routing logic without forking mainnet.
// A mock router that just transfers amountIn of path[0] to `to`.

describe("SwapExecutor", function () {
  let executor: SwapExecutor;
  let owner: HardhatEthersSigner;
  let relayer: HardhatEthersSigner;
  let recipient: HardhatEthersSigner;
  let MockERC20: any;
  let MockRouter: any;
  let tokenIn: any;
  let tokenOut: any;
  let mockRouter: any;

  before(async function () {
    [owner, relayer, recipient] = await ethers.getSigners();

    // Deploy mock ERC20s
    const ERC20Factory = await ethers.getContractFactory("MockERC20");
    tokenIn = await ERC20Factory.deploy("TokenA", "TKA", 18);
    tokenOut = await ERC20Factory.deploy("TokenB", "TKB", 18);

    // Deploy mock router
    const RouterFactory = await ethers.getContractFactory("MockRouter");
    mockRouter = await RouterFactory.deploy();

    // Fund the router with tokenOut so it can pay out swaps
    await tokenOut.mint(await mockRouter.getAddress(), ethers.parseEther("1000000"));

    // Deploy SwapExecutor with relayer as fee recipient
    const Executor = await ethers.getContractFactory("SwapExecutor");
    executor = await Executor.deploy(await relayer.getAddress());

    // Allow mock router for local tests
    await executor.setRouterAllowed(await mockRouter.getAddress(), true);

    // Mint tokenIn to owner
    await tokenIn.mint(owner.address, ethers.parseEther("10000"));
    await tokenIn.approve(await executor.getAddress(), ethers.MaxUint256);
  });

  it("deploys with correct feeRecipient", async function () {
    expect(await executor.feeRecipient()).to.equal(await relayer.getAddress());
  });

  it("WPEG constant is Base WETH", async function () {
    expect(await executor.WPEG()).to.equal("0x4200000000000000000000000000000000000006");
  });

  it("AERODROME_ROUTER constant is correct", async function () {
    expect(await executor.AERODROME_ROUTER()).to.equal("0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43");
  });

  it("tradeFor routes output to recipient, not msg.sender", async function () {
    const amountIn = ethers.parseEther("100");
    const path = [await tokenIn.getAddress(), await tokenOut.getAddress()];
    const recipientAddr = await recipient.getAddress();

    const balBefore = await tokenOut.balanceOf(recipientAddr);

    await executor.tradeFor(amountIn, recipientAddr, await mockRouter.getAddress(), path, 5);

    const balAfter = await tokenOut.balanceOf(recipientAddr);
    expect(balAfter).to.be.gt(balBefore);
    // Caller (owner) does NOT receive tokenOut
    expect(await tokenOut.balanceOf(owner.address)).to.equal(0n);
  });

  it("trade() sends output to msg.sender (backwards compat)", async function () {
    const amountIn = ethers.parseEther("100");
    const path = [await tokenIn.getAddress(), await tokenOut.getAddress()];

    const balBefore = await tokenOut.balanceOf(owner.address);
    await executor.trade(amountIn, await mockRouter.getAddress(), path, 5);
    const balAfter = await tokenOut.balanceOf(owner.address);
    expect(balAfter).to.be.gt(balBefore);
  });

  it("owner can update feeRecipient", async function () {
    const newFee = await recipient.getAddress();
    await executor.setFeeRecipient(newFee);
    expect(await executor.feeRecipient()).to.equal(newFee);

    // Reset
    await executor.setFeeRecipient(await relayer.getAddress());
  });

  it("non-owner cannot set feeRecipient", async function () {
    await expect(
      executor.connect(relayer).setFeeRecipient(relayer.address)
    ).to.be.revertedWithCustomError(executor, "OwnableUnauthorizedAccount");
  });

  it("rejects zero recipient in tradeFor", async function () {
    const path = [await tokenIn.getAddress(), await tokenOut.getAddress()];
    await expect(
      executor.tradeFor(ethers.parseEther("1"), ethers.ZeroAddress, await mockRouter.getAddress(), path, 5)
    ).to.be.revertedWith("Zero recipient");
  });
});
