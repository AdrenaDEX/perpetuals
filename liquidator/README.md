## Liquidation script

Run the script and compete with other liquidators to liquidate un-healthy positions and earn rewards.

### Pre-requires

- Node.js version 16+

TODO

### Run

TODO

### Code architecture

- **Client**: Allow interactions with Adrena program onchain.
- **PriceFeed**: Keep updated tokens prices.
- **PositionsBank**: Keep updated positions list.
- **LiquidatorBot**: Listen to tokens prices changes and positions changes and try and liquidate unhealthy positions.
