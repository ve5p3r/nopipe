// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/Base64.sol";
import "@openzeppelin/contracts/utils/Strings.sol";

/**
 * @title OperatorNFT
 * @notice Nopipe operator license NFT.
 *
 * Tiers:
 *   1 = Operator
 *   2 = Pro
 *   3 = Enterprise
 *
 * Supply:
 *   MAX_SUPPLY = 200 (network capacity with direct servicing)
 *   GENESIS_SUPPLY = 100 (founding cohort, Gauntlet-gated)
 *
 * Core upgrades:
 * - O(1) access checks via highestTier mapping
 * - Soulbound lock only during founding lock period; normal transfer after expiry
 * - batchMint for launch
 * - on-chain base64 tokenURI metadata
 */
contract OperatorNFT is ERC721, Ownable {
    using Strings for uint256;

    uint8 public constant TIER_OPERATOR = 1;
    uint8 public constant TIER_PRO = 2;
    uint8 public constant TIER_ENTERPRISE = 3;

    uint256 public constant MAX_SUPPLY = 200;
    uint256 public constant MAX_PRO = 150;
    uint256 public constant MAX_ENTERPRISE = 50;
    uint256 public constant GENESIS_SUPPLY = 100;

    uint256 public constant SOULBOUND_DAYS = 180;

    uint256 private _nextTokenId;

    uint256 public totalSupply;
    uint256 public operatorSupply;
    uint256 public proSupply;
    uint256 public enterpriseSupply;

    mapping(uint256 => uint8) private _tier;
    mapping(uint256 => bool) private _soulbound;
    mapping(uint256 => uint256) private _mintedAt;

    // O(1) access primitive
    mapping(address => uint8) public highestTier;

    // Approved minters (e.g., Gauntlet relayer for auto-mint)
    mapping(address => bool) public approvedMinters;

    event MinterUpdated(address indexed minter, bool approved);

    // Track per-tier counts per owner to support accurate highestTier updates on transfer/burn
    mapping(address => mapping(uint8 => uint256)) private _tierBalances;

    // Optional owner token enumeration (used by tests / UX)
    mapping(address => uint256[]) private _ownedTokens;
    mapping(uint256 => uint256) private _ownedTokensIndex;

    event Minted(address indexed to, uint256 indexed tokenId, uint8 tier, bool soulbound);

    constructor() ERC721("Nopipe Operator", "NPOP") Ownable(msg.sender) {}

    // ─────────────────────────────────────────────────────────────────────────
    // Minter role
    // ─────────────────────────────────────────────────────────────────────────

    modifier onlyMinter() {
        require(msg.sender == owner() || approvedMinters[msg.sender], "Not authorized minter");
        _;
    }

    function setMinter(address minter, bool approved) external onlyOwner {
        require(minter != address(0), "Zero minter");
        approvedMinters[minter] = approved;
        emit MinterUpdated(minter, approved);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Minting
    // ─────────────────────────────────────────────────────────────────────────

    function mint(address to, uint8 tier, bool soulbound_) external onlyMinter {
        _mintInternal(to, tier, soulbound_);
    }

    /**
     * @notice Batch mint for founding launch.
     * @dev Launch mints are soulbound by default.
     */
    function batchMint(address[] calldata recipients, uint8[] calldata tiers) external onlyMinter {
        require(recipients.length == tiers.length, "Length mismatch");

        for (uint256 i = 0; i < recipients.length; i++) {
            _mintInternal(recipients[i], tiers[i], true);
        }
    }

    function _mintInternal(address to, uint8 tier, bool soulbound_) private {
        require(to != address(0), "Zero recipient");
        require(tier >= TIER_OPERATOR && tier <= TIER_ENTERPRISE, "Invalid tier");
        require(totalSupply < MAX_SUPPLY, "Max supply reached");

        if (tier == TIER_PRO) {
            require(proSupply < MAX_PRO, "Pro cap reached");
            proSupply++;
        } else if (tier == TIER_ENTERPRISE) {
            require(enterpriseSupply < MAX_ENTERPRISE, "Enterprise cap reached");
            enterpriseSupply++;
        } else {
            operatorSupply++;
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
     * @param operator Wallet being evaluated for tier-gated access.
     * @param minTier Minimum tier required for access.
     * @return True when the wallet satisfies the requested minimum tier.
     */
    function hasAccess(address operator, uint8 minTier) external view returns (bool) {
        if (minTier <= TIER_OPERATOR) return true;
        return highestTier[operator] >= minTier;
    }

    function tokensOfOwner(address holder) external view returns (uint256[] memory) {
        return _ownedTokens[holder];
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
            '{"name":"Nopipe Operator #', tokenId.toString(),
            '","tier":', uint256(tier).toString(),
            ',"attributes":[{"trait_type":"Tier","value":"', _tierName(tier), '"}]}'
        );

        return string.concat("data:application/json;base64,", Base64.encode(bytes(json)));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Burn
    // ─────────────────────────────────────────────────────────────────────────

    function burn(uint256 tokenId) external {
        address tokenOwner = _ownerOf(tokenId);
        require(tokenOwner != address(0), "Token does not exist");
        require(_isAuthorized(tokenOwner, msg.sender, tokenId), "Not authorized");
        require(!_soulbound[tokenId], "Soulbound: burn disabled");

        uint8 tier = _tier[tokenId];

        totalSupply--;
        if (tier == TIER_PRO) {
            proSupply--;
        } else if (tier == TIER_ENTERPRISE) {
            enterpriseSupply--;
        } else {
            operatorSupply--;
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

    function _removeFromOwnerTokens(address holder, uint256 tokenId) private {
        uint256 idx = _ownedTokensIndex[tokenId];
        uint256 last = _ownedTokens[holder].length - 1;

        if (idx != last) {
            uint256 lastToken = _ownedTokens[holder][last];
            _ownedTokens[holder][idx] = lastToken;
            _ownedTokensIndex[lastToken] = idx;
        }

        _ownedTokens[holder].pop();
        delete _ownedTokensIndex[tokenId];
    }

    function _incrementTier(address holder, uint8 tier) private {
        _tierBalances[holder][tier] += 1;
        if (tier > highestTier[holder]) {
            highestTier[holder] = tier;
        }
    }

    function _decrementTier(address holder, uint8 tier) private {
        uint256 current = _tierBalances[holder][tier];
        if (current > 0) {
            _tierBalances[holder][tier] = current - 1;
        }

        if (highestTier[holder] == tier && _tierBalances[holder][tier] == 0) {
            if (_tierBalances[holder][TIER_ENTERPRISE] > 0) {
                highestTier[holder] = TIER_ENTERPRISE;
            } else if (_tierBalances[holder][TIER_PRO] > 0) {
                highestTier[holder] = TIER_PRO;
            } else if (_tierBalances[holder][TIER_OPERATOR] > 0) {
                highestTier[holder] = TIER_OPERATOR;
            } else {
                highestTier[holder] = 0;
            }
        }
    }

    // Tier names for on-chain metadata
    function _tierName(uint8 tier) private pure returns (string memory) {
        if (tier == TIER_ENTERPRISE) return "Enterprise";
        if (tier == TIER_PRO) return "Pro";
        return "Operator";
    }
}
