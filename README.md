# SOUL Token Smart Contract

A Solana program implementing the SOUL token with vesting and distribution mechanics.

## Token Distribution

Total Supply: 10,000,000,000 SOUL (10 billion)

- Team: 10% (1,000,000,000 SOUL)
  - Vesting: 25% every 6 months
  - No tokens at TGE
  
- DEX Liquidity: 5% (500,000,000 SOUL)
  - Available for liquidity provision
  
- CEX + Marketing: 10% (1,000,000,000 SOUL)
  - 100% unlocked at TGE
  
- Development: 30% (3,000,000,000 SOUL)
  - 5% unlocked at TGE
  - Remaining tokens unlock over 6 periods
  
- Community Rewards: 15% (1,500,000,000 SOUL)
  - 30% unlocked at TGE
  - Remaining tokens unlock 200M every 60 days
  
- Pre-sale: 30% (3,000,000,000 SOUL)
  - Managed by separate contract

## Prerequisites

- Rust 1.70.0 or later
- Solana Tool Suite 1.16.0 or later
- Anchor Framework 0.27.0 or later

## Setup

1. Install dependencies:
```bash
cargo install --git https://github.com/project-serum/anchor anchor-cli --locked
```

2. Build the program:
```bash
anchor build
```

3. Deploy to devnet:
```bash
anchor deploy --provider.cluster devnet
```

## Testing

Run the test suite:
```bash
anchor test
```

## Security

This contract handles token distribution and vesting mechanics. Please ensure proper security audits before mainnet deployment.

## License

All rights reserved. This code is proprietary and confidential. 