import { expect } from "chai";
import { ethers } from "hardhat";

describe("SubscriptionKeeper reentrancy", () => {
  async function deployFixture() {
    const [owner, agent, relayer] = await ethers.getSigners();

    const MockERC20 = await ethers.getContractFactory("MockERC20");
    const stable = await MockERC20.deploy("USDC", "USDC", 6);

    const Keeper = await ethers.getContractFactory("SubscriptionKeeper");
    const keeper = await Keeper.deploy(await stable.getAddress(), owner.address);

    const MockRouter = await ethers.getContractFactory("MockRouter");
    const router = await MockRouter.deploy();

    const Malicious = await ethers.getContractFactory("MockMaliciousERC20");
    const malicious = await Malicious.deploy("MAL", "MAL", 6);

    await stable.mint(await router.getAddress(), 10_000_000_000n);
    await malicious.mint(agent.address, 10_000_000_000n);

    const pool = relayer.address;
    await keeper.setPool(pool, 1, 100);
    await keeper.subscribe(
      agent.address,
      pool,
      await router.getAddress(),
      [await malicious.getAddress(), await stable.getAddress()],
      0
    );

    await keeper.connect(agent).authorizeBudget(1_000_000n, 2n);
    await malicious.connect(agent).approve(await keeper.getAddress(), ethers.MaxUint256);

    await ethers.provider.send("evm_increaseTime", [30 * 24 * 60 * 60 + 10]);
    await ethers.provider.send("evm_mine", []);

    return { keeper, stable, malicious, owner, agent, relayer, pool, router };
  }

  it("blocks reentrancy via malicious ERC20 callback on collectFor", async () => {
    const { keeper, malicious, agent, router } = await deployFixture();

    await keeper.setPool(await malicious.getAddress(), 1, 10);
    await keeper.subscribe(
      await malicious.getAddress(),
      await malicious.getAddress(),
      await router.getAddress(),
      [await malicious.getAddress(), await keeper.STABLE()],
      0
    );

    await malicious.configureHook(await keeper.getAddress(), 2, true);
    await malicious.armHook(true);

    const subBefore = await keeper.getSub(agent.address);
    const balBefore = await malicious.balanceOf(await keeper.getAddress());

    await expect(keeper.collectFor(agent.address))
      .to.emit(keeper, "SubRenewalFailed")
      .withArgs(agent.address, "Payment pull failed");

    const subAfter = await keeper.getSub(agent.address);
    const balAfter = await malicious.balanceOf(await keeper.getAddress());
    const tokenBudget = await keeper.getBudget(await malicious.getAddress());

    expect(subAfter.nextRenewal).to.equal(subBefore.nextRenewal);
    expect(subAfter.stop).to.equal(false);
    expect(balAfter).to.equal(balBefore);
    expect(tokenBudget.maxPerCycle).to.equal(0n);
  });

  it("cross-function reentrancy attempt (stopRenewal) during collectFor keeps state consistent", async () => {
    const { keeper, malicious, agent, router } = await deployFixture();

    await keeper.setPool(await malicious.getAddress(), 1, 10);
    await keeper.subscribe(
      await malicious.getAddress(),
      await malicious.getAddress(),
      await router.getAddress(),
      [await malicious.getAddress(), await keeper.STABLE()],
      0
    );

    const beforeBudget = await keeper.getBudget(agent.address);
    const subBefore = await keeper.getSub(agent.address);

    await malicious.configureHook(await keeper.getAddress(), 1, false);
    await malicious.armHook(true);

    await expect(keeper.collectFor(agent.address))
      .to.emit(keeper, "SubRenewed");

    const afterBudget = await keeper.getBudget(agent.address);
    const subAfter = await keeper.getSub(agent.address);

    expect(subAfter.stop).to.equal(false);
    expect(subAfter.nextRenewal).to.be.gt(subBefore.nextRenewal);
    expect(afterBudget.cyclesRemaining).to.equal(beforeBudget.cyclesRemaining - 1n);
  });
});
