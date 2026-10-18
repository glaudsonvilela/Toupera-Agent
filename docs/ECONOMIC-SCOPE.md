# Economic Scope

## V1 boundary

Only Solidity/EVM economic security for vaults and oracles: vault accounting, share/asset relations, pricing and conversions, fees, rounding, oracle assumptions and related external calls. Priority operations are `deposit`, `mint`, `withdraw`, `redeem`, `previewDeposit`, `previewMint`, `previewWithdraw`, `previewRedeem`, `convertToShares`, `convertToAssets`, `totalAssets`, `totalSupply`, `getPrice`, and `latestPrice` including fallback behavior.

Out of scope for this phase: Solana, bridges, NFTs, frontend security, consensus, wallets, phishing, MEV, and general blockchain cybersecurity.

## Economic invariants

- **ROUND-TRIP VALUE:** deposit then redeem must not yield an unexpected gain.
- **SHARE CONSERVATION:** minted and burned shares must preserve accounting.
- **ASSET CONSERVATION:** `totalAssets` must reflect assets actually controlled.
- **PRICE SYMMETRY:** economically opposite operations must use compatible pricing rules.
- **ORACLE CONSISTENCY:** deposit/mint/withdraw/redeem must respect coherent oracle assumptions.
- **DECIMAL CONSISTENCY:** token and oracle conversions must preserve scale.
- **ROUNDING SAFETY:** rounding must not create exploitable extra value.
- **FEE CONSISTENCY:** equivalent paths must not bypass fees.
- **PREVIEW CONSISTENCY:** `preview*` results must be economically compatible with execution.
- **FIRST DEPOSITOR SAFETY:** the first deposit must not enable artificial share inflation.
- **DONATION SAFETY:** direct vault transfers must not create an improper gain.

## Priority detector identifiers

`PRICE_SOURCE_ASYMMETRY`, `DECIMAL_MISMATCH`, `ROUNDING_ASYMMETRY`, `PREVIEW_EXECUTION_MISMATCH`, `SHARE_INFLATION`, `FIRST_DEPOSITOR_ATTACK`, `DONATION_MANIPULATION`, `FEE_BYPASS`, `STALE_ORACLE`, `ORACLE_FALLBACK_MISMATCH`, `ACCOUNTING_DRIFT`, `ASSET_SHARE_INCONSISTENCY`, `UNEXPECTED_VALUE_CREATION`. These are planned identifiers, not implemented detectors.
