// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/Base64.sol";
import "@openzeppelin/contracts/utils/Strings.sol";

/**
 * @title OperatorNFT
 * @notice Polyclaw operator license NFT.
 *
 * Tiers:
 *   1 = Free
 *   2 = Pro
 *   3 = Institutional
 *
 * Core upgrades:
 * - O(1) access checks via highestTier mapping
 * - Soulbound lock only during founding lock period; normal transfer after expiry
 * - batchMint for launch
 * - on-chain base64 tokenURI metadata
 */
contract OperatorNFT is ERC721, Ownable {
    using Strings for uint256;

    uint8 public constant TIER_FREE = 1;
    uint8 public constant TIER_PRO = 2;
    uint8 public constant TIER_INSTITUTIONAL = 3;

    uint256 public constant MAX_SUPPLY = 500;
    uint256 public constant MAX_PRO = 400;
    uint256 public constant MAX_INSTITUTIONAL = 100;

    uint256 public constant SOULBOUND_DAYS = 180;

    uint256 private _nextTokenId;

    uint256 public totalSupply;
    uint256 public proSupply;
    uint256 public institutionalSupply;

    mapping(uint256 => uint8) private _tier;
    mapping(uint256 => bool) private _soulbound;
    mapping(uint256 => uint256) private _mintedAt;

    // O(1) access primitive
    mapping(address => uint8) public highestTier;

    // Track per-tier counts per owner to support accurate highestTier updates on transfer/burn
    mapping(address => mapping(uint8 => uint256)) private _tierBalances;

    // Optional owner token enumeration (used by tests / UX)
    mapping(address => uint256[]) private _ownedTokens;
    mapping(uint256 => uint256) private _ownedTokensIndex;

    event Minted(address indexed to, uint256 indexed tokenId, uint8 tier, bool soulbound);

    constructor() ERC721("Polyclaw Operator", "PCOP") Ownable(msg.sender) {}

    // ─────────────────────────────────────────────────────────────────────────
    // Minting
    // ─────────────────────────────────────────────────────────────────────────

    function mint(address to, uint8 tier, bool soulbound_) external onlyOwner {
        _mintInternal(to, tier, soulbound_);
    }

    /**
     * @notice Batch mint for founding launch.
     * @dev Launch mints are soulbound by default.
     */
    function batchMint(address[] calldata recipients, uint8[] calldata tiers) external onlyOwner {
        require(recipients.length == tiers.length, "Length mismatch");

        for (uint256 i = 0; i < recipients.length; i++) {
            _mintInternal(recipients[i], tiers[i], true);
        }
    }

    function _mintInternal(address to, uint8 tier, bool soulbound_) private {
        require(to != address(0), "Zero recipient");
        require(tier >= TIER_FREE && tier <= TIER_INSTITUTIONAL, "Invalid tier");
        require(totalSupply < MAX_SUPPLY, "Max supply reached");

        if (tier == TIER_PRO) {
            require(proSupply < MAX_PRO, "Pro cap reached");
            proSupply++;
        } else if (tier == TIER_INSTITUTIONAL) {
            require(institutionalSupply < MAX_INSTITUTIONAL, "Institutional cap reached");
            institutionalSupply++;
        }

        uint256 tokenId = ++_nextTokenId;
        totalSupply++;

        _tier[tokenId] = tier;
        _soulbound[tokenId] = soulbound_;
        _mintedAt[tokenId] = block.timestamp;

        _safeMint(to, tokenId);

        emit Minted(to, tokenId, tier, soulbound_);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Access / tier views
    // ─────────────────────────────────────────────────────────────────────────

    function tokenTier(uint256 tokenId) external view returns (uint8) {
        require(_ownerOf(tokenId) != address(0), "Token does not exist");
        return _tier[tokenId];
    }

    /**
     * @notice O(1) access check.
     */
    function hasAccess(address operator, uint8 minTier) external view returns (bool) {
        if (minTier <= TIER_FREE) return true;
        return highestTier[operator] >= minTier;
    }

    function tokensOfOwner(address owner) external view returns (uint256[] memory) {
        return _ownedTokens[owner];
    }

    function isSoulbound(uint256 tokenId) external view returns (bool) {
        require(_ownerOf(tokenId) != address(0), "Token does not exist");
        if (!_soulbound[tokenId]) return false;
        return block.timestamp < _mintedAt[tokenId] + (SOULBOUND_DAYS * 1 days);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Metadata
    // ─────────────────────────────────────────────────────────────────────────

    function tokenURI(uint256 tokenId) public view override returns (string memory) {
        require(_ownerOf(tokenId) != address(0), "Token does not exist");

        uint8 tier = _tier[tokenId];
        string memory json = string.concat(
            '{"name":"Polyclaw Operator #', tokenId.toString(),
            '","tier":', uint256(tier).toString(),
            ',"attributes":[{"trait_type":"Tier","value":"', _tierName(tier), '"}]}'
        );

        return string.concat("data:application/json;base64,", Base64.encode(bytes(json)));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Burn
    // ─────────────────────────────────────────────────────────────────────────

    function burn(uint256 tokenId) external {
        address owner = _ownerOf(tokenId);
        require(owner != address(0), "Token does not exist");
        require(_isAuthorized(owner, msg.sender, tokenId), "Not authorized");

        uint8 tier = _tier[tokenId];

        totalSupply--;
        if (tier == TIER_PRO) {
            proSupply--;
        } else if (tier == TIER_INSTITUTIONAL) {
            institutionalSupply--;
        }

        _burn(tokenId);

        delete _tier[tokenId];
        delete _soulbound[tokenId];
        delete _mintedAt[tokenId];
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Transfer hook
    // ─────────────────────────────────────────────────────────────────────────

    function _update(address to, uint256 tokenId, address auth)
        internal
        override
        returns (address from)
    {
        from = _ownerOf(tokenId);

        // Soulbound lock enforced only while lock period is active.
        // Mint: from == 0, Burn: to == 0, both are always allowed.
        if (from != address(0) && to != address(0)) {
            require(
                !_soulbound[tokenId] ||
                    block.timestamp >= _mintedAt[tokenId] + (SOULBOUND_DAYS * 1 days),
                "Soulbound: transfer locked"
            );
        }

        from = super._update(to, tokenId, auth);

        // Keep owner indexes and highestTier mapping accurate.
        if (from != to) {
            uint8 tier = _tier[tokenId];

            if (from != address(0)) {
                _removeFromOwnerTokens(from, tokenId);
                _decrementTier(from, tier);
            }

            if (to != address(0)) {
                _ownedTokensIndex[tokenId] = _ownedTokens[to].length;
                _ownedTokens[to].push(tokenId);
                _incrementTier(to, tier);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────────────────────────────────

    function _removeFromOwnerTokens(address owner, uint256 tokenId) private {
        uint256 idx = _ownedTokensIndex[tokenId];
        uint256 last = _ownedTokens[owner].length - 1;

        if (idx != last) {
            uint256 lastToken = _ownedTokens[owner][last];
            _ownedTokens[owner][idx] = lastToken;
            _ownedTokensIndex[lastToken] = idx;
        }

        _ownedTokens[owner].pop();
        delete _ownedTokensIndex[tokenId];
    }

    function _incrementTier(address owner, uint8 tier) private {
        _tierBalances[owner][tier] += 1;
        if (tier > highestTier[owner]) {
            highestTier[owner] = tier;
        }
    }

    function _decrementTier(address owner, uint8 tier) private {
        uint256 current = _tierBalances[owner][tier];
        if (current > 0) {
            _tierBalances[owner][tier] = current - 1;
        }

        if (highestTier[owner] == tier && _tierBalances[owner][tier] == 0) {
            if (_tierBalances[owner][TIER_INSTITUTIONAL] > 0) {
                highestTier[owner] = TIER_INSTITUTIONAL;
            } else if (_tierBalances[owner][TIER_PRO] > 0) {
                highestTier[owner] = TIER_PRO;
            } else if (_tierBalances[owner][TIER_FREE] > 0) {
                highestTier[owner] = TIER_FREE;
            } else {
                highestTier[owner] = 0;
            }
        }
    }

    function _tierName(uint8 tier) private pure returns (string memory) {
        if (tier == TIER_INSTITUTIONAL) return "Institutional";
        if (tier == TIER_PRO) return "Pro";
        return "Free";
    }
}
