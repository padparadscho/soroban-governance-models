#![no_std]

use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, Map, String, Symbol, Vec,
};

const MAX_PROPOSAL_DURATION: u64 = 1_296_000;
const MIN_PROPOSAL_DURATION: u64 = 432000;

const PROPOSALS_TTL_EXTENSION: u32 = 2_100_000;
const PROPOSAL_TTL_BUFFER: u32 = 604_800;

#[contracttype]
pub enum QuadraticVoteContractDataKey {
    Admin,
    Token,
    Proposal(Symbol),
    Proposals,
    Votes(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct QuadraticVoteProposalData {
    pub description: String,
    pub start_time: u64,
    pub end_time: u64,
    pub options: Vec<Symbol>,
    pub max_options_per_vote: u32,
    pub total_votes: Map<Symbol, i128>,
}

#[contracttype]
pub struct QuadraticVoteProposalSummary {
    pub id: Symbol,
    pub description: String,
    pub status: bool,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuadraticVoteContractErrors {
    ContractNotInitialized = 1,
    ContractAlreadyInitialized = 2,
    ProposalAlreadyExists = 3,
    ProposalNotFound = 4,
    UserAlreadyVoted = 5,
    UserCannotVote = 6,
    VotingNotActive = 7,
    InvalidOption = 8,
    StartTimeAfterEnd = 9,
    StartTimeInPast = 10,
    DurationTooLong = 11,
    DurationTooShort = 12,
    TooManyOptions = 20,
    InvalidVoteCount = 21,
    InsufficientBalance = 22,
    NoVotesSubmitted = 23,
}

#[contract]
pub struct QuadraticVoteContract;

#[contractimpl]
impl QuadraticVoteContract {
    fn calculate_proposal_ttl(env: &Env, proposal_end_time: u64) -> u32 {
        let now = env.ledger().timestamp();
        let proposal_duration = if proposal_end_time > now {
            proposal_end_time - now
        } else {
            0
        };
        let min_ttl = proposal_duration as u32 + PROPOSAL_TTL_BUFFER;
        min_ttl.max(PROPOSALS_TTL_EXTENSION)
    }

    fn validate_proposal_times(
        ledger_time: u64,
        start_time: u64,
        end_time: u64,
    ) -> Result<(), QuadraticVoteContractErrors> {
        if start_time >= end_time {
            return Err(QuadraticVoteContractErrors::StartTimeAfterEnd);
        }
        if start_time < ledger_time {
            return Err(QuadraticVoteContractErrors::StartTimeInPast);
        }
        let duration = end_time - start_time;
        if duration > MAX_PROPOSAL_DURATION {
            return Err(QuadraticVoteContractErrors::DurationTooLong);
        }
        if duration < MIN_PROPOSAL_DURATION {
            return Err(QuadraticVoteContractErrors::DurationTooShort);
        }
        Ok(())
    }

    fn quadratic_influence(votes: u32) -> i128 {
        if votes == 0 {
            return 0;
        }
        let sqrt_approx = Self::integer_sqrt(votes as u64) as i128;
        sqrt_approx * 1000
    }

    fn integer_sqrt(n: u64) -> u64 {
        if n < 2 {
            return n;
        }
        let mut left = 1u64;
        let mut right = n;
        while left <= right {
            let mid = left + (right - left) / 2;
            if mid <= n / mid {
                left = mid + 1;
            } else {
                right = mid - 1;
            }
        }
        right
    }

    pub fn __constructor(
        env: Env,
        admin: Address,
        token: Address,
    ) -> Result<(), QuadraticVoteContractErrors> {
        if env
            .storage()
            .instance()
            .has(&QuadraticVoteContractDataKey::Admin)
        {
            return Err(QuadraticVoteContractErrors::ContractAlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&QuadraticVoteContractDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&QuadraticVoteContractDataKey::Token, &token);
        Ok(())
    }

    pub fn create_proposal(
        env: Env,
        id: Symbol,
        description: String,
        start_time: u64,
        end_time: u64,
        options: Vec<Symbol>,
        max_options_per_vote: u32,
    ) -> Result<(), QuadraticVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&QuadraticVoteContractDataKey::Admin)
            .ok_or(QuadraticVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        let now = env.ledger().timestamp();
        Self::validate_proposal_times(now, start_time, end_time)?;

        if options.is_empty() || max_options_per_vote == 0 || max_options_per_vote > options.len() {
            return Err(QuadraticVoteContractErrors::InvalidOption);
        }

        let proposal_key = QuadraticVoteContractDataKey::Proposal(id.clone());
        if env.storage().persistent().has(&proposal_key) {
            return Err(QuadraticVoteContractErrors::ProposalAlreadyExists);
        }

        let mut total_votes = Map::new(&env);
        for option in options.iter() {
            total_votes.set(option, 0);
        }

        let proposal = QuadraticVoteProposalData {
            description,
            start_time,
            end_time,
            options,
            max_options_per_vote,
            total_votes,
        };
        env.storage().persistent().set(&proposal_key, &proposal);
        let proposal_ttl = Self::calculate_proposal_ttl(&env, end_time);
        env.storage()
            .persistent()
            .extend_ttl(&proposal_key, proposal_ttl, proposal_ttl);

        let mut proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&QuadraticVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        proposals.push_back(id.clone());
        env.storage()
            .persistent()
            .set(&QuadraticVoteContractDataKey::Proposals, &proposals);
        env.storage().persistent().extend_ttl(
            &QuadraticVoteContractDataKey::Proposals,
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
        votes: Map<Symbol, u32>,
    ) -> Result<(), QuadraticVoteContractErrors> {
        user.require_auth();

        let proposal_key = QuadraticVoteContractDataKey::Proposal(id.clone());
        let mut proposal: QuadraticVoteProposalData = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(QuadraticVoteContractErrors::ProposalNotFound)?;

        let now = env.ledger().timestamp();
        if now < proposal.start_time || now > proposal.end_time {
            return Err(QuadraticVoteContractErrors::VotingNotActive);
        }

        let votes_key = QuadraticVoteContractDataKey::Votes(user.clone());
        let mut user_votes: Map<Symbol, Map<Symbol, u32>> = env
            .storage()
            .persistent()
            .get(&votes_key)
            .unwrap_or(Map::new(&env));

        if user_votes.contains_key(id.clone()) {
            return Err(QuadraticVoteContractErrors::UserAlreadyVoted);
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&QuadraticVoteContractDataKey::Token)
            .ok_or(QuadraticVoteContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let token_balance = token_client.balance(&user);
        if token_balance <= 0 {
            return Err(QuadraticVoteContractErrors::UserCannotVote);
        }

        if votes.is_empty() {
            return Err(QuadraticVoteContractErrors::NoVotesSubmitted);
        }
        if votes.len() > proposal.max_options_per_vote {
            return Err(QuadraticVoteContractErrors::TooManyOptions);
        }

        let mut total_cost: i128 = 0;
        for (option, v) in votes.iter() {
            if !proposal.options.contains(&option) {
                return Err(QuadraticVoteContractErrors::InvalidOption);
            }
            if v == 0 {
                return Err(QuadraticVoteContractErrors::InvalidVoteCount);
            }
            total_cost += (v as i128) * (v as i128);
        }

        if total_cost > token_balance {
            return Err(QuadraticVoteContractErrors::InsufficientBalance);
        }

        for (option, v) in votes.iter() {
            let current_votes = proposal.total_votes.get(option.clone()).unwrap_or(0);
            let influence = Self::quadratic_influence(v);
            proposal
                .total_votes
                .set(option.clone(), current_votes.saturating_add(influence));
        }

        user_votes.set(id.clone(), votes.clone());
        env.storage().persistent().set(&proposal_key, &proposal);
        env.storage().persistent().set(&votes_key, &user_votes);

        let proposal_ttl = Self::calculate_proposal_ttl(&env, proposal.end_time);
        env.storage()
            .persistent()
            .extend_ttl(&proposal_key, proposal_ttl, proposal_ttl);
        env.storage()
            .persistent()
            .extend_ttl(&votes_key, proposal_ttl, proposal_ttl);

        env.events().publish(("VOTE", id, user), votes.len());
        Ok(())
    }

    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), QuadraticVoteContractErrors> {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&QuadraticVoteContractDataKey::Admin)
            .ok_or(QuadraticVoteContractErrors::ContractNotInitialized)?;
        current_admin.require_auth();

        env.storage()
            .instance()
            .set(&QuadraticVoteContractDataKey::Admin, &new_admin);

        env.events()
            .publish(("ADMIN", "TRANSFERRED"), (current_admin, new_admin));
        Ok(())
    }

    pub fn get_governance_details(env: Env) -> Vec<QuadraticVoteProposalSummary> {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&QuadraticVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        let mut summary = Vec::new(&env);
        let now = env.ledger().timestamp();

        for id in proposals.iter() {
            if let Some(proposal) = env
                .storage()
                .persistent()
                .get::<_, QuadraticVoteProposalData>(&QuadraticVoteContractDataKey::Proposal(
                    id.clone(),
                ))
            {
                let status = now >= proposal.start_time && now <= proposal.end_time;
                summary.push_back(QuadraticVoteProposalSummary {
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
    ) -> Result<QuadraticVoteProposalData, QuadraticVoteContractErrors> {
        let proposal: QuadraticVoteProposalData = env
            .storage()
            .persistent()
            .get(&QuadraticVoteContractDataKey::Proposal(id))
            .ok_or(QuadraticVoteContractErrors::ProposalNotFound)?;
        Ok(proposal)
    }

    pub fn get_vote_cost_and_influence(_env: Env, votes: u32) -> (u32, i128) {
        let cost = votes * votes;
        let influence = Self::quadratic_influence(votes);
        (cost, influence)
    }

    pub fn get_user_details(
        env: Env,
        user: Address,
    ) -> Result<Vec<(Symbol, bool, i128)>, QuadraticVoteContractErrors> {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&QuadraticVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));

        let votes_key = QuadraticVoteContractDataKey::Votes(user.clone());
        let user_votes: Map<Symbol, Map<Symbol, u32>> = env
            .storage()
            .persistent()
            .get(&votes_key)
            .unwrap_or(Map::new(&env));

        let token_address: Address = env
            .storage()
            .instance()
            .get(&QuadraticVoteContractDataKey::Token)
            .ok_or(QuadraticVoteContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let token_balance = token_client.balance(&user);

        let mut results = Vec::new(&env);
        for id in proposals.iter() {
            if let Some(user_vote_map) = user_votes.get(id.clone()) {
                let total_cost = user_vote_map
                    .iter()
                    .fold(0, |acc, (_, v)| acc + (v as i128) * (v as i128));
                results.push_back((id.clone(), true, total_cost));
            } else {
                results.push_back((id.clone(), false, token_balance));
            }
        }
        Ok(results)
    }
}
