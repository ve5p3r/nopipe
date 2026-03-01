// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/**
 * @dev MockRouter: simulates Uniswap V2-style router for testing.
 *      1:1 ratio: amountOut = amountIn.
 *      Pulls path[0] from msg.sender, sends path[-1] to `to`.
 *      Must be pre-funded with output tokens.
 */
contract MockRouter {

    function getAmountsOut(uint256 amountIn, address[] memory path)
        external pure returns (uint256[] memory amounts)
    {
        amounts = new uint256[](path.length);
        for (uint256 i = 0; i < path.length; i++) {
            amounts[i] = amountIn; // 1:1
        }
    }

    function swapExactTokensForTokensSupportingFeeOnTransferTokens(
        uint256 amountIn,
        uint256 /* amountOutMin */,
        address[] calldata path,
        address to,
        uint256 /* deadline */
    ) external {
        IERC20(path[0]).transferFrom(msg.sender, address(this), amountIn);
        IERC20(path[path.length - 1]).transfer(to, amountIn);
    }

    function swapExactTokensForETHSupportingFeeOnTransferTokens(
        uint256 amountIn,
        uint256 /* amountOutMin */,
        address[] calldata path,
        address to,
        uint256 /* deadline */
    ) external {
        IERC20(path[0]).transferFrom(msg.sender, address(this), amountIn);
        payable(to).transfer(amountIn);
    }

    function swapExactETHForTokensSupportingFeeOnTransferTokens(
        uint256 /* amountOutMin */,
        address[] calldata path,
        address to,
        uint256 /* deadline */
    ) external payable {
        IERC20(path[path.length - 1]).transfer(to, msg.value);
    }

    receive() external payable {}
}
