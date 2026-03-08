import { ethers } from "hardhat";
import { expect } from "chai";

describe("Security red-team", () => {

  // ──────────────────────────────────────────────────────────────────────────
  // OperatorNFT
  // ──────────────────────────────────────────────────────────────────────────
  describe("OperatorNFT", () => {
    async function deploy() {
      const [owner, alice, attacker] = await ethers.getSigners();
      const F = await ethers.getContractFactory("OperatorNFT");
      const nft = await F.deploy();
      return { nft, owner, alice, attacker };
    }

    it("highestTier is 0 after burning last Pro NFT", async () => {
      const { nft, alice } = await deploy();
      await nft.mint(alice.address, 2, false);
      expect(await nft.highestTier(alice.address)).to.equal(2);
      const tokens = await nft.tokensOfOwner(alice.address);
      await nft.connect(alice).burn(tokens[0]);
      expect(await nft.highestTier(alice.address)).to.equal(0);
    });

    it("highestTier falls back to lower tier after burning highest", async () => {
      const { nft, alice } = await deploy();
      await nft.mint(alice.address, 1, false); // Free → tokenId 1
      await nft.mint(alice.address, 2, false); // Pro  → tokenId 2
      const tokens = await nft.tokensOfOwner(alice.address);
      // find Pro token by checking tierOf
      let proToken = tokens[0];
      for (const t of tokens) {
        if ((await nft.tokenTier(t)) === 2n) { proToken = t; break; }
      }
      await nft.connect(alice).burn(proToken);
      expect(await nft.highestTier(alice.address)).to.equal(1); // drops to Free
    });

    it("totalSupply decrements on burn", async () => {
      const { nft, alice } = await deploy();
      await nft.mint(alice.address, 1, false);
      const before = await nft.totalSupply();
      const tokens = await nft.tokensOfOwner(alice.address);
      await nft.connect(alice).burn(tokens[0]);
      expect(await nft.totalSupply()).to.equal(before - 1n);
    });

    it("non-owner cannot burn another wallet's soulbound token", async () => {
      const { nft, alice, attacker } = await deploy();
      await nft.mint(alice.address, 1, true); // soulbound to alice
      const tokens = await nft.tokensOfOwner(alice.address);
      // attacker is a distinct third address, not alice
      await expect(nft.connect(attacker).burn(tokens[0])).to.be.reverted;
    });
  });

  // ──────────────────────────────────────────────────────────────────────────
  // SwapExecutor
  // ──────────────────────────────────────────────────────────────────────────
  describe("SwapExecutor", () => {
    it("reverts construction with zero feeRecipient", async () => {
      const F = await ethers.getContractFactory("SwapExecutor");
      await expect(F.deploy(ethers.ZeroAddress)).to.be.revertedWith("Zero fee recipient");
    });

    it("setFeeRecipient reverts on zero address", async () => {
      const [owner] = await ethers.getSigners();
      const F = await ethers.getContractFactory("SwapExecutor");
      const executor = await F.deploy(owner.address);
      await expect(executor.setFeeRecipient(ethers.ZeroAddress)).to.be.revertedWith("Zero address");
    });
  });

  // ──────────────────────────────────────────────────────────────────────────
  // SubscriptionKeeper
  // ──────────────────────────────────────────────────────────────────────────
  describe("SubscriptionKeeper", () => {
    async function deploy() {
      const [owner, alice] = await ethers.getSigners();
      const MockERC20F = await ethers.getContractFactory("MockERC20");
      const stable = await MockERC20F.deploy("USDC", "USDC", 6);
      const F = await ethers.getContractFactory("SubscriptionKeeper");
      const keeper = await F.deploy(await stable.getAddress(), owner.address);
      return { keeper, stable, owner, alice };
    }

    it("reverts construction with zero feeRecipient", async () => {
      const [, alice] = await ethers.getSigners();
      const MockERC20F = await ethers.getContractFactory("MockERC20");
      const stable = await MockERC20F.deploy("USDC", "USDC", 6);
      const F = await ethers.getContractFactory("SubscriptionKeeper");
      await expect(F.deploy(await stable.getAddress(), ethers.ZeroAddress))
        .to.be.revertedWith("Zero address");
    });

    it("reverts construction with zero stable", async () => {
      const [owner] = await ethers.getSigners();
      const F = await ethers.getContractFactory("SubscriptionKeeper");
      await expect(F.deploy(ethers.ZeroAddress, owner.address))
        .to.be.revertedWith("Zero address");
    });

    it("does NOT accept ETH (receive removed)", async () => {
      const { keeper, owner } = await deploy();
      await expect(
        owner.sendTransaction({ to: await keeper.getAddress(), value: ethers.parseEther("0.1") })
      ).to.be.reverted;
    });

    it("authorizeBudget sets caller budget only", async () => {
      const { keeper, alice, owner } = await deploy();
      await keeper.connect(alice).authorizeBudget(100n, 5n);
      const budget = await keeper.getBudget(alice.address);
      expect(budget.maxPerCycle).to.equal(100n);
      expect(budget.cyclesRemaining).to.equal(5n);
      const ownerBudget = await keeper.getBudget(owner.address);
      expect(ownerBudget.maxPerCycle).to.equal(0n);
    });
  });
});
