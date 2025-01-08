#![no_std]

use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, String,
    Symbol, Vec,
};

const VOTE_FOR: Symbol = symbol_short!("FOR");
const VOTE_AGAINST: Symbol = symbol_short!("AGAINST");
const VOTE_ABSTAIN: Symbol = symbol_short!("ABSTAIN");

const MAX_PROPOSAL_DURATION: u64 = 1292000;
const MIN_PROPOSAL_DURATION: u64 = 432000;

const PROPOSALS_TTL_EXTENSION: u32 = 2_100_000;
const PROPOSAL_TTL_BUFFER: u32 = 604_800;
const VOTE_TTL_EXTENSION: u32 = 1_600_000;

#[contracttype]
pub enum TokenGatedContractDataKey {
    Admin,
    Token,
    Proposal(Symbol),
    Proposals,
    Votes(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct TokenGatedProposalData {
    pub description: String,
    pub start_time: u64,
    pub end_time: u64,
    pub total_for: i128,
    pub total_against: i128,
    pub total_abstain: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct TokenGatedProposalSummary {
    pub id: Symbol,
    pub description: String,
    pub status: TokenGatedProposalStatus,
}

#[contracttype]
#[derive(Clone, Copy)]
pub enum TokenGatedProposalStatus {
    Pending,
    Active,
    Ended,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenGatedContractErrors {
    ContractNotInitialized = 1,
    ContractAlreadyInitialized = 2,
    ProposalAlreadyExists = 3,
    ProposalNotFound = 4,
    UserAlreadyVoted = 5,
    UserCannotVote = 6,
    VotingNotActive = 7,
    InvalidChoice = 8,
    StartTimeAfterEnd = 9,
    StartTimeInPast = 10,
    DurationTooLong = 11,
    DurationTooShort = 12,
}

#[contract]
pub struct TokenGatedContract;

#[contractimpl]
impl TokenGatedContract {
    fn calculate_proposal_ttl(env: &Env, proposal_end_time: u64) -> u32 {
        let ledger_time = env.ledger().timestamp();
        let proposal_duration = if proposal_end_time > ledger_time {
            proposal_end_time - ledger_time
        } else {
            0
        };

        let min_ttl = proposal_duration as u32 + PROPOSAL_TTL_BUFFER;
        min_ttl.max(PROPOSALS_TTL_EXTENSION)
    }

    fn compute_proposal_status(
        ledger_time: u64,
        proposal: &TokenGatedProposalData,
    ) -> TokenGatedProposalStatus {
        if ledger_time < proposal.start_time {
            TokenGatedProposalStatus::Pending
        } else if ledger_time <= proposal.end_time {
            TokenGatedProposalStatus::Active
        } else {
            TokenGatedProposalStatus::Ended
        }
    }

    fn validate_proposal_times(
        ledger_time: u64,
        start_time: u64,
        end_time: u64,
    ) -> Result<(), TokenGatedContractErrors> {
        if start_time >= end_time {
            return Err(TokenGatedContractErrors::StartTimeAfterEnd);
        }
        if start_time < ledger_time {
            return Err(TokenGatedContractErrors::StartTimeInPast);
        }
        let duration = end_time - start_time;
        if duration > MAX_PROPOSAL_DURATION {
            return Err(TokenGatedContractErrors::DurationTooLong);
        }
        if duration < MIN_PROPOSAL_DURATION {
            return Err(TokenGatedContractErrors::DurationTooShort);
        }
        Ok(())
    }

    pub fn __constructor(
        env: Env,
        admin: Address,
        token: Address,
    ) -> Result<(), TokenGatedContractErrors> {
        if env
            .storage()
            .instance()
            .has(&TokenGatedContractDataKey::Admin)
        {
            return Err(TokenGatedContractErrors::ContractAlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&TokenGatedContractDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&TokenGatedContractDataKey::Token, &token);
        Ok(())
    }

    pub fn create_proposal(
        env: Env,
        id: Symbol,
        description: String,
        start_time: u64,
        end_time: u64,
    ) -> Result<(), TokenGatedContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&TokenGatedContractDataKey::Admin)
            .ok_or(TokenGatedContractErrors::ContractNotInitialized)?;
        admin.require_auth();
        let ledger_time = env.ledger().timestamp();
        Self::validate_proposal_times(ledger_time, start_time, end_time)?;

        let proposal_key = TokenGatedContractDataKey::Proposal(id.clone());
        if env.storage().persistent().has(&proposal_key) {
            return Err(TokenGatedContractErrors::ProposalAlreadyExists);
        }

        let proposal = TokenGatedProposalData {
            description,
            start_time,
            end_time,
            total_for: 0,
            total_against: 0,
            total_abstain: 0,
        };
        env.storage().persistent().set(&proposal_key, &proposal);

        let proposal_ttl = Self::calculate_proposal_ttl(&env, end_time);
        env.storage()
            .persistent()
            .extend_ttl(&proposal_key, proposal_ttl, proposal_ttl);

        let mut proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&TokenGatedContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        proposals.push_back(id.clone());
        env.storage()
            .persistent()
            .set(&TokenGatedContractDataKey::Proposals, &proposals);

        env.storage().persistent().extend_ttl(
            &TokenGatedContractDataKey::Proposals,
            PROPOSALS_TTL_EXTENSION,
            PROPOSALS_TTL_EXTENSION,
        );

        env.events().publish(("PROPOSAL", "CREATED"), id);
        Ok(())
    }

    pub fn vote(
        env: Env,
        user: Address,
        id: Symbol,
        choice: Symbol,
    ) -> Result<(), TokenGatedContractErrors> {
        user.require_auth();

        let proposal_key = TokenGatedContractDataKey::Proposal(id.clone());
        let mut proposal: TokenGatedProposalData = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(TokenGatedContractErrors::ProposalNotFound)?;

        let ledger_time = env.ledger().timestamp();
        if ledger_time < proposal.start_time || ledger_time > proposal.end_time {
            return Err(TokenGatedContractErrors::VotingNotActive);
        }

        let votes_key = TokenGatedContractDataKey::Votes(user.clone());
        let mut votes: Map<Symbol, bool> = env
            .storage()
            .persistent()
            .get(&votes_key)
            .unwrap_or(Map::new(&env));

        if votes.contains_key(id.clone()) {
            return Err(TokenGatedContractErrors::UserAlreadyVoted);
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&TokenGatedContractDataKey::Token)
            .ok_or(TokenGatedContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let token_balance = token_client.balance(&user);
        if token_balance <= 0 {
            return Err(TokenGatedContractErrors::UserCannotVote);
        }

        if choice == VOTE_FOR {
            proposal.total_for = proposal.total_for.saturating_add(1);
        } else if choice == VOTE_AGAINST {
            proposal.total_against = proposal.total_against.saturating_add(1);
        } else if choice == VOTE_ABSTAIN {
            proposal.total_abstain = proposal.total_abstain.saturating_add(1);
        } else {
            return Err(TokenGatedContractErrors::InvalidChoice);
        }

        votes.set(id.clone(), true);

        env.storage().persistent().set(&proposal_key, &proposal);
        env.storage().persistent().set(&votes_key, &votes);

        let proposal_ttl = Self::calculate_proposal_ttl(&env, proposal.end_time);
        env.storage()
            .persistent()
            .extend_ttl(&proposal_key, proposal_ttl, proposal_ttl);

        env.storage()
            .persistent()
            .extend_ttl(&votes_key, VOTE_TTL_EXTENSION, VOTE_TTL_EXTENSION);

        env.events().publish(("VOTE", id, user), (choice, 1));
        Ok(())
    }

    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), TokenGatedContractErrors> {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&TokenGatedContractDataKey::Admin)
            .ok_or(TokenGatedContractErrors::ContractNotInitialized)?;

        current_admin.require_auth();

        env.storage()
            .instance()
            .set(&TokenGatedContractDataKey::Admin, &new_admin);

        env.events()
            .publish(("ADMIN", "TRANSFERRED"), (current_admin, new_admin));
        Ok(())
    }

    pub fn get_governance_details(env: Env) -> Vec<TokenGatedProposalSummary> {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&TokenGatedContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        let mut summary = Vec::new(&env);

        let ledger_time = env.ledger().timestamp();

        for id in proposals.iter() {
            if let Some(proposal) = env
                .storage()
                .persistent()
                .get::<TokenGatedContractDataKey, TokenGatedProposalData>(
                    &TokenGatedContractDataKey::Proposal(id.clone()),
                )
            {
                let status = Self::compute_proposal_status(ledger_time, &proposal);
                summary.push_back(TokenGatedProposalSummary {
                    id: id.clone(),
                    description: proposal.description.clone(),
                    status,
                });
            }
        }
        summary
    }

    pub fn get_proposal_details(
        env: Env,
        id: Symbol,
    ) -> Result<TokenGatedProposalData, TokenGatedContractErrors> {
        let proposal: TokenGatedProposalData = env
            .storage()
            .persistent()
            .get(&TokenGatedContractDataKey::Proposal(id))
            .ok_or(TokenGatedContractErrors::ProposalNotFound)?;
        Ok(proposal)
    }

    pub fn get_user_details(
        env: Env,
        user: Address,
    ) -> Result<Vec<(Symbol, bool, i128)>, TokenGatedContractErrors> {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&TokenGatedContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));

        let votes_key = TokenGatedContractDataKey::Votes(user.clone());
        let votes: Map<Symbol, bool> = env
            .storage()
            .persistent()
            .get(&votes_key)
            .unwrap_or(Map::new(&env));

        let token_address: Address = env
            .storage()
            .instance()
            .get(&TokenGatedContractDataKey::Token)
            .ok_or(TokenGatedContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let token_balance = token_client.balance(&user);

        let voting_power = if token_balance > 0 { 1 } else { 0 };

        let mut results = Vec::new(&env);
        for id in proposals.iter() {
            if let Some(_) = votes.get(id.clone()) {
                results.push_back((id.clone(), true, voting_power));
            } else {
                results.push_back((id.clone(), false, voting_power));
            }
        }
        Ok(results)
    }
}

mod test;
