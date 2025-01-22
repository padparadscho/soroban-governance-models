# Soroban Governance Models

A comprehensive suite of [Soroban smart contracts](https://developers.stellar.org/) implementing various governance and voting mechanisms.

## Overview

### [Token-Gated Vote Contract](/token-gated-vote-contract/)

Implements a "**one holder, one vote**" democratic governance model where every token holder receives equal voting weight. Token ownership above zero qualifies users to vote, with each holder getting exactly one vote.

### [Token-Weighted Vote Contract](/token-weighted-vote-contract/)

Implements a "**token balance equals voting power**" plutocratic governance model where each user's vote carries weight proportional to their token balance, creating a system where economic stake determines governance power.

### [Representative Vote Contract](/representative-vote-contract/)

Implements a "**delegated voting**" governance model where token holders assign their voting power to representatives who vote on their behalf, enabling scalable governance while maintaining democratic principles.

### [Liquid-Based Vote Contract](/liquid-based-vote-contract/)

Implements a "**liquid democracy**" hybrid governance model where users can vote directly with token-weighted power or delegate their voting power to representatives.

## License

This project is licensed under the [MIT License](/LICENSE).
