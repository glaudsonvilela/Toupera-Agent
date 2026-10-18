# Security Scope

Toupera Agent is intended only for contracts the operator owns, laboratory projects, local forks, and targets explicitly authorized by security or bug bounty programs.

The system must not automatically attack mainnet or send offensive transactions to real contracts. Exploit validation belongs in unit tests, Anvil, Foundry local forks, or explicitly authorized environments. A generated hypothesis remains unverified until a deterministic local test reproduces it; human review is required before reporting.
