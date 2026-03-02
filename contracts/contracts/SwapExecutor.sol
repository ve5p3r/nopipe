// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

interface IRouter {
    function getAmountsOut(uint256 amountIn, address[] calldata path)
        external
        view
        returns (uint256[] memory amounts);

    function swapExactTokensForTokensSupportingFeeOnTransferTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external;
}

/**
 * @title SwapExecutor
 * @notice Non-custodial swap execution layer for Nopipe on Base.
 *
 * - Router allowlist enforced at execution time
 * - 0.1% protocol fee charged on input before routing
 * - Slippage guard in basis points
 * - Pausable emergency stop
 */
contract SwapExecutor is Ownable, Pausable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // Base WETH
    address public constant WPEG = 0x4200000000000000000000000000000000000006;

    // Base router allowlist constants
    address public constant AERODROME_ROUTER = 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43;
    address public constant UNISWAP_V3_ROUTER = 0x2626664c2603336E57B271c5C0b26F421741e481;
    address public constant UNISWAP_V2_ROUTER = 0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24;

    uint256 public constant FEE_BPS = 10; // 0.1%

    mapping(address => bool) public allowedRouters;
    address public feeRecipient;

    event FeeRecipientUpdated(address indexed newRecipient);
    event RouterAllowlistUpdated(address indexed router, bool allowed);
    event SwapExecuted(
        address indexed caller,
        address indexed recipient,
        uint256 amountIn,
        uint256 feeCharged,
        uint256 amountOut
    );

    constructor(address _feeRecipient) Ownable(msg.sender) {
        require(_feeRecipient != address(0), "Zero fee recipient");
        feeRecipient = _feeRecipient;

        allowedRouters[AERODROME_ROUTER] = true;
        allowedRouters[UNISWAP_V3_ROUTER] = true;
        allowedRouters[UNISWAP_V2_ROUTER] = true;
    }

    /**
     * @notice Core cluster primitive: swap and route output to recipient.
     * @param amountIn Input token amount (must be > 0)
     * @param recipient Recipient of output token
     * @param router Approved DEX router
     * @param path Swap path
     * @param slippageBps Max tolerated slippage in bps (0-10000)
     */
    function tradeFor(
        uint256 amountIn,
        address recipient,
        address router,
        address[] calldata path,
        uint256 slippageBps
    ) public nonReentrant whenNotPaused returns (uint256 amountOut) {
        _validateTradeInput(amountIn, recipient, router, path, slippageBps);

        IERC20 inputToken = IERC20(path[0]);
        IERC20 outputToken = IERC20(path[path.length - 1]);

        inputToken.safeTransferFrom(msg.sender, address(this), amountIn);

        uint256 feeCharged = (amountIn * FEE_BPS) / 10_000;
        uint256 amountToSwap = amountIn - feeCharged;
        require(amountToSwap > 0, "Amount too small");

        if (feeCharged > 0) {
            inputToken.safeTransfer(feeRecipient, feeCharged);
        }

        uint256 expected = _quote(router, path, amountToSwap);
        uint256 minOut = (expected * (10_000 - slippageBps)) / 10_000;

        inputToken.forceApprove(router, amountToSwap);

        uint256 balanceBefore = outputToken.balanceOf(recipient);

        IRouter(router).swapExactTokensForTokensSupportingFeeOnTransferTokens(
            amountToSwap,
            minOut,
            path,
            recipient,
            block.timestamp
        );

        uint256 balanceAfter = outputToken.balanceOf(recipient);
        amountOut = balanceAfter - balanceBefore;

        require(amountOut >= minOut, "Slippage exceeded");

        emit SwapExecuted(msg.sender, recipient, amountIn, feeCharged, amountOut);
    }

    /**
     * @notice Backwards compatibility helper: output to msg.sender.
     */
    function trade(
        uint256 amountIn,
        address router,
        address[] calldata path,
        uint256 slippageBps
    ) external returns (uint256 amountOut) {
        return tradeFor(amountIn, msg.sender, router, path, slippageBps);
    }

    /**
     * @notice Quote expected output after protocol fee deduction.
     */
    function getQuote(
        address router,
        address[] calldata path,
        uint256 amountIn
    ) external view returns (uint256 amountOut) {
        require(allowedRouters[router], "Router not allowed");
        require(path.length >= 2, "Bad path");
        require(amountIn > 0, "Zero amount");

        uint256 amountToSwap = amountIn - ((amountIn * FEE_BPS) / 10_000);
        require(amountToSwap > 0, "Amount too small");

        return _quote(router, path, amountToSwap);
    }

    function setFeeRecipient(address _feeRecipient) external onlyOwner {
        require(_feeRecipient != address(0), "Zero address");
        feeRecipient = _feeRecipient;
        emit FeeRecipientUpdated(_feeRecipient);
    }

    function setRouterAllowed(address router, bool allowed) external onlyOwner {
        require(router != address(0), "Zero router");
        allowedRouters[router] = allowed;
        emit RouterAllowlistUpdated(router, allowed);
    }

    function pause() external onlyOwner {
        _pause();
    }

    function unpause() external onlyOwner {
        _unpause();
    }

    function rescueToken(address token, address to, uint256 amount) external onlyOwner {
        IERC20(token).safeTransfer(to, amount);
    }

    function _validateTradeInput(
        uint256 amountIn,
        address recipient,
        address router,
        address[] calldata path,
        uint256 slippageBps
    ) private view {
        require(recipient != address(0), "Zero recipient");
        require(path.length >= 2, "Bad path");
        require(amountIn > 0, "Zero amount");
        require(allowedRouters[router], "Router not allowed");
        require(slippageBps <= 10_000, "Bad slippage");
    }

    function _quote(
        address router,
        address[] calldata path,
        uint256 amountIn
    ) private view returns (uint256 amountOut) {
        uint256[] memory amounts = IRouter(router).getAmountsOut(amountIn, path);
        amountOut = amounts[amounts.length - 1];
    }
}
