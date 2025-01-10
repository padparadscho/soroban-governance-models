# Soroban Governance Models

A comprehensive suite of [Soroban smart contracts](https://developers.stellar.org/) implementing various governance and voting mechanisms.

## Overview

### [Token-Gated Vote Contract](/token-gated-vote-contract/)

Implements a "**one holder, one vote**" democratic governance model where every token holder receives equal voting weight. Token ownership above zero qualifies users to vote, with each holder getting exactly one vote.

### [Token-Weighted Vote Contract](/token-weighted-vote-contract/)

Implements a "**token balance equals voting power**" plutocratic governance model where each user's vote carries weight proportional to their token balance, creating a system where economic stake determines governance power.

## License

This project is licensed under the [MIT License](/LICENSE).
