import { expect } from "chai";
import { ethers } from "hardhat";
import { time } from "@nomicfoundation/hardhat-network-helpers";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/signers";
import { OperatorNFT } from "../typechain-types";

describe("OperatorNFT", function () {
  let nft: OperatorNFT;
  let owner: HardhatEthersSigner;
  let user1: HardhatEthersSigner;
  let user2: HardhatEthersSigner;

  const TIER_FREE          = 1;
  const TIER_PRO           = 2;
  const TIER_INSTITUTIONAL = 3;
  const SOULBOUND_DAYS     = 180;

  before(async function () {
    [owner, user1, user2] = await ethers.getSigners();
    const NFT = await ethers.getContractFactory("OperatorNFT");
    nft = await NFT.deploy();
  });

  // ── Constants ──────────────────────────────────────────────────────────
  it("has correct name and symbol", async function () {
    expect(await nft.name()).to.equal("Nopipe Operator");
    expect(await nft.symbol()).to.equal("NPOP");
  });

  it("has correct supply caps", async function () {
    expect(await nft.MAX_SUPPLY()).to.equal(500n);
    expect(await nft.MAX_PRO()).to.equal(400n);
    expect(await nft.MAX_INSTITUTIONAL()).to.equal(100n);
  });

  // ── Minting ────────────────────────────────────────────────────────────
  it("owner can mint Pro NFT", async function () {
    await nft.mint(user1.address, TIER_PRO, false);
    expect(await nft.totalSupply()).to.equal(1n);
    expect(await nft.proSupply()).to.equal(1n);
    expect(await nft.tokenTier(1)).to.equal(TIER_PRO);
    expect(await nft.ownerOf(1)).to.equal(user1.address);
  });

  it("owner can mint Institutional NFT (soulbound)", async function () {
    await nft.mint(user2.address, TIER_INSTITUTIONAL, true);
    expect(await nft.institutionalSupply()).to.equal(1n);
    expect(await nft.tokenTier(2)).to.equal(TIER_INSTITUTIONAL);
  });

  it("non-owner cannot mint", async function () {
    await expect(
      nft.connect(user1).mint(user1.address, TIER_PRO, false)
    ).to.be.revertedWithCustomError(nft, "OwnableUnauthorizedAccount");
  });

  it("reverts on invalid tier", async function () {
    await expect(nft.mint(user1.address, 0, false)).to.be.revertedWith("Invalid tier");
    await expect(nft.mint(user1.address, 4, false)).to.be.revertedWith("Invalid tier");
  });

  // ── hasAccess ─────────────────────────────────────────────────────────
  it("hasAccess: tier 1 (Free) is always true", async function () {
    const nobody = ethers.Wallet.createRandom().address;
    expect(await nft.hasAccess(nobody, TIER_FREE)).to.equal(true);
  });

  it("hasAccess: Pro holder passes Pro check", async function () {
    expect(await nft.hasAccess(user1.address, TIER_PRO)).to.equal(true);
  });

  it("hasAccess: Pro holder fails Institutional check", async function () {
    expect(await nft.hasAccess(user1.address, TIER_INSTITUTIONAL)).to.equal(false);
  });

  it("hasAccess: Institutional holder passes Pro check", async function () {
    expect(await nft.hasAccess(user2.address, TIER_PRO)).to.equal(true);
  });

  it("hasAccess: wallet with no NFT fails Pro check", async function () {
    const nobody = ethers.Wallet.createRandom().address;
    expect(await nft.hasAccess(nobody, TIER_PRO)).to.equal(false);
  });

  // ── Soulbound ─────────────────────────────────────────────────────────
  it("soulbound token cannot be transferred during lockup", async function () {
    // token 2 is soulbound, minted to user2
    expect(await nft.isSoulbound(2)).to.equal(true);
    await expect(
      nft.connect(user2).transferFrom(user2.address, user1.address, 2)
    ).to.be.revertedWith("Soulbound: transfer locked");
  });

  it("non-soulbound token transfers freely", async function () {
    await nft.connect(user1).transferFrom(user1.address, user2.address, 1);
    expect(await nft.ownerOf(1)).to.equal(user2.address);
    // Transfer back
    await nft.connect(user2).transferFrom(user2.address, user1.address, 1);
  });

  it("soulbound token can transfer after lockup expires", async function () {
    // Advance time past 180 days
    await time.increase(SOULBOUND_DAYS * 24 * 60 * 60 + 1);

    expect(await nft.isSoulbound(2)).to.equal(false);
    await nft.connect(user2).transferFrom(user2.address, user1.address, 2);
    expect(await nft.ownerOf(2)).to.equal(user1.address);
  });

  // ── tokensOfOwner ──────────────────────────────────────────────────────
  it("tokensOfOwner returns correct token list", async function () {
    const tokens = await nft.tokensOfOwner(user1.address);
    // user1 should have tokenId 1 and 2 (2 was just transferred above)
    expect(tokens.length).to.equal(2);
  });
});
