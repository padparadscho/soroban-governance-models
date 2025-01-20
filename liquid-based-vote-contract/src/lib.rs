#![no_std]

use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, String,
    Symbol, Vec,
};

const VOTE_FOR: Symbol = symbol_short!("FOR");
const VOTE_AGAINST: Symbol = symbol_short!("AGAINST");
const VOTE_ABSTAIN: Symbol = symbol_short!("ABSTAIN");

const MAX_PROPOSAL_DURATION: u64 = 1_296_000;
const MIN_PROPOSAL_DURATION: u64 = 432000;
const MAX_DELEGATES_NUMBER: u32 = 10;

const PROPOSALS_TTL_EXTENSION: u32 = 2_100_000;
const PROPOSAL_TTL_BUFFER: u32 = 604_800;
const DELEGATES_TTL_EXTENSION: u32 = 2_100_000;
const USER_ACTION_TTL_EXTENSION: u32 = 1_600_000;

#[contracttype]
pub enum LiquidBasedVoteContractDataKey {
    Admin,
    Token,
    Delegates,
    Proposal(Symbol),
    Proposals,
    UserVote(Symbol, Address),
    UserDelegation(Symbol, Address),
    DelegatePower(Symbol, Address),
    DelegateVote(Symbol, Address),
}

#[contracttype]
#[derive(Clone)]
pub struct LiquidBasedVoteProposalData {
    pub description: String,
    pub delegation_deadline: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub total_for: i128,
    pub total_against: i128,
    pub total_abstain: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct LiquidBasedVoteDelegationData {
    pub delegate: Address,
    pub amount: i128,
}

#[contracttype]
pub struct LiquidBasedVoteProposalSummary {
    pub id: Symbol,
    pub description: String,
    pub status: bool,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiquidBasedVoteActionStatus {
    None,
    Voted,
    Delegated,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiquidBasedVoteContractErrors {
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
    DelegateLimitReached = 13,
    DelegateNotFound = 14,
    DelegateAlreadyVoted = 15,
    NotDelegate = 16,
    DelegationDeadlineEnded = 17,
    DelegationDeadlineInPast = 18,
    DelegationDeadlineAfterStart = 19,
}

#[contract]
pub struct LiquidBasedVoteContract;

#[contractimpl]
impl LiquidBasedVoteContract {
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

    fn calculate_user_action_ttl(env: &Env, proposal_end_time: u64) -> u32 {
        let now = env.ledger().timestamp();
        let proposal_duration = if proposal_end_time > now {
            proposal_end_time - now
        } else {
            0
        };

        let min_ttl = proposal_duration as u32 + PROPOSAL_TTL_BUFFER;
        min_ttl.max(USER_ACTION_TTL_EXTENSION)
    }

    pub fn __constructor(
        env: Env,
        admin: Address,
        token: Address,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        if env
            .storage()
            .instance()
            .has(&LiquidBasedVoteContractDataKey::Admin)
        {
            return Err(LiquidBasedVoteContractErrors::ContractAlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&LiquidBasedVoteContractDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&LiquidBasedVoteContractDataKey::Token, &token);
        Ok(())
    }

    pub fn set_delegates(
        env: Env,
        delegates: Vec<Address>,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Admin)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        if delegates.len() > MAX_DELEGATES_NUMBER {
            return Err(LiquidBasedVoteContractErrors::DelegateLimitReached);
        }

        let mut delegates_map = Map::new(&env);
        for delegate in delegates.iter() {
            delegates_map.set(delegate, ());
        }
        env.storage()
            .persistent()
            .set(&LiquidBasedVoteContractDataKey::Delegates, &delegates_map);
        env.storage().persistent().extend_ttl(
            &LiquidBasedVoteContractDataKey::Delegates,
            DELEGATES_TTL_EXTENSION,
            DELEGATES_TTL_EXTENSION,
        );

        env.events().publish(("SET_DELEGATES",), delegates);
        Ok(())
    }

    pub fn add_delegates(
        env: Env,
        to_add: Vec<Address>,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Admin)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        let mut delegates_map: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));

        for delegate in to_add.iter() {
            if !delegates_map.contains_key(delegate.clone()) {
                delegates_map.set(delegate, ());
            }
        }

        if delegates_map.len() > MAX_DELEGATES_NUMBER {
            return Err(LiquidBasedVoteContractErrors::DelegateLimitReached);
        }

        env.storage()
            .persistent()
            .set(&LiquidBasedVoteContractDataKey::Delegates, &delegates_map);
        env.storage().persistent().extend_ttl(
            &LiquidBasedVoteContractDataKey::Delegates,
            DELEGATES_TTL_EXTENSION,
            DELEGATES_TTL_EXTENSION,
        );

        env.events().publish(("ADD_DELEGATES",), to_add);
        Ok(())
    }

    pub fn remove_delegates(
        env: Env,
        to_remove: Vec<Address>,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Admin)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        let mut delegates_map: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));

        for delegate in to_remove.iter() {
            delegates_map.remove(delegate);
        }

        env.storage()
            .persistent()
            .set(&LiquidBasedVoteContractDataKey::Delegates, &delegates_map);
        env.storage().persistent().extend_ttl(
            &LiquidBasedVoteContractDataKey::Delegates,
            DELEGATES_TTL_EXTENSION,
            DELEGATES_TTL_EXTENSION,
        );

        env.events().publish(("REMOVE_DELEGATES",), to_remove);
        Ok(())
    }

    pub fn create_proposal(
        env: Env,
        id: Symbol,
        description: String,
        delegation_deadline: u64,
        start_time: u64,
        end_time: u64,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Admin)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        let now = env.ledger().timestamp();

        if delegation_deadline < now {
            return Err(LiquidBasedVoteContractErrors::DelegationDeadlineInPast);
        }

        if delegation_deadline >= start_time {
            return Err(LiquidBasedVoteContractErrors::DelegationDeadlineAfterStart);
        }

        if start_time < now {
            return Err(LiquidBasedVoteContractErrors::StartTimeInPast);
        }

        if start_time >= end_time {
            return Err(LiquidBasedVoteContractErrors::StartTimeAfterEnd);
        }

        if end_time - start_time > MAX_PROPOSAL_DURATION {
            return Err(LiquidBasedVoteContractErrors::DurationTooLong);
        }
        if end_time - start_time < MIN_PROPOSAL_DURATION {
            return Err(LiquidBasedVoteContractErrors::DurationTooShort);
        }

        let proposal_key = LiquidBasedVoteContractDataKey::Proposal(id.clone());
        if env.storage().persistent().has(&proposal_key) {
            return Err(LiquidBasedVoteContractErrors::ProposalAlreadyExists);
        }

        let proposal = LiquidBasedVoteProposalData {
            description,
            delegation_deadline,
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
            .get(&LiquidBasedVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        proposals.push_back(id.clone());
        env.storage()
            .persistent()
            .set(&LiquidBasedVoteContractDataKey::Proposals, &proposals);
        env.storage().persistent().extend_ttl(
            &LiquidBasedVoteContractDataKey::Proposals,
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
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        user.require_auth();

        let user_vote_key = LiquidBasedVoteContractDataKey::UserVote(id.clone(), user.clone());
        let user_delegation_key =
            LiquidBasedVoteContractDataKey::UserDelegation(id.clone(), user.clone());
        if env.storage().persistent().has(&user_vote_key)
            || env.storage().persistent().has(&user_delegation_key)
        {
            return Err(LiquidBasedVoteContractErrors::UserAlreadyVoted);
        }

        let proposal_key = LiquidBasedVoteContractDataKey::Proposal(id.clone());
        let mut proposal: LiquidBasedVoteProposalData = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(LiquidBasedVoteContractErrors::ProposalNotFound)?;

        let now = env.ledger().timestamp();
        if now < proposal.start_time || now > proposal.end_time {
            return Err(LiquidBasedVoteContractErrors::VotingNotActive);
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Token)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let token_balance = token_client.balance(&user);
        if token_balance <= 0 {
            return Err(LiquidBasedVoteContractErrors::UserCannotVote);
        }

        if choice == VOTE_FOR {
            proposal.total_for = proposal.total_for.saturating_add(token_balance);
        } else if choice == VOTE_AGAINST {
            proposal.total_against = proposal.total_against.saturating_add(token_balance);
        } else if choice == VOTE_ABSTAIN {
            proposal.total_abstain = proposal.total_abstain.saturating_add(token_balance);
        } else {
            return Err(LiquidBasedVoteContractErrors::InvalidChoice);
        }

        env.storage()
            .persistent()
            .set(&user_vote_key, &token_balance);
        env.storage().persistent().set(&proposal_key, &proposal);

        let user_action_ttl = Self::calculate_user_action_ttl(&env, proposal.end_time);
        env.storage()
            .persistent()
            .extend_ttl(&user_vote_key, user_action_ttl, user_action_ttl);

        let proposal_ttl = Self::calculate_proposal_ttl(&env, proposal.end_time);
        env.storage()
            .persistent()
            .extend_ttl(&proposal_key, proposal_ttl, proposal_ttl);

        env.events()
            .publish(("VOTE", id, user), (choice, token_balance));
        Ok(())
    }

    pub fn delegate(
        env: Env,
        user: Address,
        id: Symbol,
        delegate_address: Address,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        user.require_auth();

        let user_vote_key = LiquidBasedVoteContractDataKey::UserVote(id.clone(), user.clone());
        let user_delegation_key =
            LiquidBasedVoteContractDataKey::UserDelegation(id.clone(), user.clone());
        if env.storage().persistent().has(&user_vote_key)
            || env.storage().persistent().has(&user_delegation_key)
        {
            return Err(LiquidBasedVoteContractErrors::UserAlreadyVoted);
        }

        let proposal_key = LiquidBasedVoteContractDataKey::Proposal(id.clone());
        let proposal: LiquidBasedVoteProposalData =
            env.storage()
                .persistent()
                .get(&proposal_key)
                .ok_or(LiquidBasedVoteContractErrors::ProposalNotFound)?;
        let now = env.ledger().timestamp();
        if now >= proposal.delegation_deadline {
            return Err(LiquidBasedVoteContractErrors::DelegationDeadlineEnded);
        }

        let delegates: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));
        if !delegates.contains_key(delegate_address.clone()) {
            return Err(LiquidBasedVoteContractErrors::DelegateNotFound);
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Token)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let balance = token_client.balance(&user);
        if balance <= 0 {
            return Err(LiquidBasedVoteContractErrors::UserCannotVote);
        }

        let delegate_power_key =
            LiquidBasedVoteContractDataKey::DelegatePower(id.clone(), delegate_address.clone());
        let current_power: i128 = env
            .storage()
            .persistent()
            .get(&delegate_power_key)
            .unwrap_or(0);
        let new_power = current_power.saturating_add(balance);
        env.storage()
            .persistent()
            .set(&delegate_power_key, &new_power);

        let delegation = LiquidBasedVoteDelegationData {
            delegate: delegate_address.clone(),
            amount: balance,
        };
        env.storage()
            .persistent()
            .set(&user_delegation_key, &delegation);

        let user_action_ttl = Self::calculate_user_action_ttl(&env, proposal.end_time);
        env.storage().persistent().extend_ttl(
            &delegate_power_key,
            user_action_ttl,
            user_action_ttl,
        );
        env.storage().persistent().extend_ttl(
            &user_delegation_key,
            user_action_ttl,
            user_action_ttl,
        );

        env.events()
            .publish(("DELEGATE", id, user), delegate_address);
        Ok(())
    }

    pub fn delegate_vote(
        env: Env,
        delegate: Address,
        id: Symbol,
        choice: Symbol,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        delegate.require_auth();

        let delegates: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));
        if !delegates.contains_key(delegate.clone()) {
            return Err(LiquidBasedVoteContractErrors::NotDelegate);
        }

        let proposal_key = LiquidBasedVoteContractDataKey::Proposal(id.clone());
        let mut proposal: LiquidBasedVoteProposalData = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(LiquidBasedVoteContractErrors::ProposalNotFound)?;
        let now = env.ledger().timestamp();
        if now < proposal.start_time || now > proposal.end_time {
            return Err(LiquidBasedVoteContractErrors::VotingNotActive);
        }

        let delegate_vote_key =
            LiquidBasedVoteContractDataKey::DelegateVote(id.clone(), delegate.clone());
        if env.storage().persistent().has(&delegate_vote_key) {
            return Err(LiquidBasedVoteContractErrors::DelegateAlreadyVoted);
        }

        let delegate_power_key =
            LiquidBasedVoteContractDataKey::DelegatePower(id.clone(), delegate.clone());
        let power: i128 = env
            .storage()
            .persistent()
            .get(&delegate_power_key)
            .unwrap_or(0);

        if choice == VOTE_FOR {
            proposal.total_for = proposal.total_for.saturating_add(power);
        } else if choice == VOTE_AGAINST {
            proposal.total_against = proposal.total_against.saturating_add(power);
        } else if choice == VOTE_ABSTAIN {
            proposal.total_abstain = proposal.total_abstain.saturating_add(power);
        } else {
            return Err(LiquidBasedVoteContractErrors::InvalidChoice);
        }

        env.storage().persistent().set(&proposal_key, &proposal);
        env.storage().persistent().set(&delegate_vote_key, &choice);

        let proposal_ttl = Self::calculate_proposal_ttl(&env, proposal.end_time);
        env.storage()
            .persistent()
            .extend_ttl(&proposal_key, proposal_ttl, proposal_ttl);

        let user_action_ttl = Self::calculate_user_action_ttl(&env, proposal.end_time);
        env.storage()
            .persistent()
            .extend_ttl(&delegate_vote_key, user_action_ttl, user_action_ttl);

        env.events()
            .publish(("VOTE", id, delegate), (choice, power));
        Ok(())
    }

    pub fn transfer_admin(
        env: Env,
        new_admin: Address,
    ) -> Result<(), LiquidBasedVoteContractErrors> {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Admin)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;

        current_admin.require_auth();

        env.storage()
            .instance()
            .set(&LiquidBasedVoteContractDataKey::Admin, &new_admin);

        env.events()
            .publish(("ADMIN", "TRANSFERRED"), (current_admin, new_admin));
        Ok(())
    }

    pub fn get_governance_details(env: Env) -> Vec<LiquidBasedVoteProposalSummary> {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        let mut summary = Vec::new(&env);
        let now = env.ledger().timestamp();

        for id in proposals.iter() {
            if let Some(proposal) = env
                .storage()
                .persistent()
                .get::<_, LiquidBasedVoteProposalData>(&LiquidBasedVoteContractDataKey::Proposal(
                    id.clone(),
                ))
            {
                let status = now >= proposal.start_time && now <= proposal.end_time;
                summary.push_back(LiquidBasedVoteProposalSummary {
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
    ) -> Result<LiquidBasedVoteProposalData, LiquidBasedVoteContractErrors> {
        let proposal: LiquidBasedVoteProposalData = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Proposal(id))
            .ok_or(LiquidBasedVoteContractErrors::ProposalNotFound)?;
        Ok(proposal)
    }

    pub fn get_user_details(
        env: Env,
        user: Address,
    ) -> Result<Vec<(Symbol, LiquidBasedVoteActionStatus, i128)>, LiquidBasedVoteContractErrors>
    {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));

        let token_address: Address = env
            .storage()
            .instance()
            .get(&LiquidBasedVoteContractDataKey::Token)
            .ok_or(LiquidBasedVoteContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let current_balance = token_client.balance(&user);

        let mut results = Vec::new(&env);

        let delegates: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&LiquidBasedVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));
        let is_delegate = delegates.contains_key(user.clone());

        for id in proposals.iter() {
            let user_vote_key = LiquidBasedVoteContractDataKey::UserVote(id.clone(), user.clone());
            let user_delegation_key =
                LiquidBasedVoteContractDataKey::UserDelegation(id.clone(), user.clone());

            let (action, power) = if let Some(voted_power) =
                env.storage().persistent().get::<_, i128>(&user_vote_key)
            {
                (LiquidBasedVoteActionStatus::Voted, voted_power)
            } else if let Some(delegation) = env
                .storage()
                .persistent()
                .get::<_, LiquidBasedVoteDelegationData>(&user_delegation_key)
            {
                (LiquidBasedVoteActionStatus::Delegated, delegation.amount)
            } else if is_delegate {
                let delegate_power_key =
                    LiquidBasedVoteContractDataKey::DelegatePower(id.clone(), user.clone());
                let delegated_power: i128 = env
                    .storage()
                    .persistent()
                    .get(&delegate_power_key)
                    .unwrap_or(0);
                if delegated_power > 0 {
                    (LiquidBasedVoteActionStatus::None, delegated_power)
                } else {
                    (LiquidBasedVoteActionStatus::None, current_balance)
                }
            } else {
                (LiquidBasedVoteActionStatus::None, current_balance)
            };
            results.push_back((id.clone(), action, power));
        }
        Ok(results)
    }
}

mod test;
