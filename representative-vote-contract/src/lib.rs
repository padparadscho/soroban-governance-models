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
const DELEGATION_GRACE_PERIOD: u64 = 172_800;

const PROPOSALS_TTL_EXTENSION: u32 = 2_100_000;
const PROPOSAL_TTL_BUFFER: u32 = 604_800;
const DELEGATES_TTL_EXTENSION: u32 = 2_100_000;
const DELEGATOR_TTL_EXTENSION: u32 = 1_600_000;

#[contracttype]
pub enum RepresentativeVoteContractDataKey {
    Admin,
    Token,
    Delegates,
    Proposal(Symbol),
    Proposals,
    DelegatorData(Symbol, Address),
    DelegateData(Symbol, Address),
}

#[contracttype]
#[derive(Clone)]
pub struct RepresentativeVoteProposalData {
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
pub struct RepresentativeVoteDelegationData {
    pub delegate: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct RepresentativeVotePowerData {
    pub accumulated_power: i128,
    pub choice: Option<Symbol>,
}

#[contracttype]
pub struct RepresentativeVoteProposalSummary {
    pub id: Symbol,
    pub description: String,
    pub status: bool,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepresentativeVoteContractErrors {
    ContractNotInitialized = 1,
    ContractAlreadyInitialized = 2,
    ProposalAlreadyExists = 3,
    ProposalNotFound = 4,
    DelegatorAlreadyDelegated = 5,
    DelegatorCannotDelegate = 6,
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
    DelegationGracePeriodEnded = 20,
}

#[contract]
pub struct RepresentativeVoteContract;

#[contractimpl]
impl RepresentativeVoteContract {
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

    fn calculate_delegation_ttl(env: &Env, proposal_end_time: u64) -> u32 {
        let now = env.ledger().timestamp();
        let proposal_duration = if proposal_end_time > now {
            proposal_end_time - now
        } else {
            0
        };

        let min_ttl = proposal_duration as u32 + PROPOSAL_TTL_BUFFER;
        min_ttl.max(DELEGATOR_TTL_EXTENSION)
    }

    fn check_revocation_eligibility(
        env: &Env,
        delegator: &Address,
        id: &Symbol,
        now: u64,
    ) -> (bool, u64) {
        Self::calculate_grace_period_status(env, delegator, id, now).0
    }

    fn calculate_grace_period_status(
        env: &Env,
        delegator: &Address,
        id: &Symbol,
        now: u64,
    ) -> (
        (bool, u64),
        Option<RepresentativeVoteDelegationData>,
        Option<RepresentativeVoteProposalData>,
    ) {
        let proposal_key = RepresentativeVoteContractDataKey::Proposal(id.clone());
        let proposal: Option<RepresentativeVoteProposalData> =
            env.storage().persistent().get(&proposal_key);

        let proposal = match proposal {
            Some(p) => p,
            None => return ((false, 0), None, None),
        };

        if now >= proposal.delegation_deadline {
            return ((false, 0), None, Some(proposal));
        }

        let delegation_key =
            RepresentativeVoteContractDataKey::DelegatorData(id.clone(), delegator.clone());
        let delegation: Option<RepresentativeVoteDelegationData> =
            env.storage().persistent().get(&delegation_key);

        let delegation_time = match &delegation {
            Some(d) => d.timestamp,
            None => return ((false, 0), None, Some(proposal)),
        };

        let grace_period_end = delegation_time.saturating_add(DELEGATION_GRACE_PERIOD);
        let effective_grace_end = grace_period_end.min(proposal.delegation_deadline);

        let result = if now <= effective_grace_end {
            let remaining_time = effective_grace_end.saturating_sub(now);
            (true, remaining_time)
        } else {
            (false, 0)
        };

        (result, delegation, Some(proposal))
    }

    pub fn __constructor(
        env: Env,
        admin: Address,
        token: Address,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        if env
            .storage()
            .instance()
            .has(&RepresentativeVoteContractDataKey::Admin)
        {
            return Err(RepresentativeVoteContractErrors::ContractAlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&RepresentativeVoteContractDataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&RepresentativeVoteContractDataKey::Token, &token);
        Ok(())
    }

    pub fn set_delegates(
        env: Env,
        delegates: Vec<Address>,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&RepresentativeVoteContractDataKey::Admin)
            .ok_or(RepresentativeVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        if delegates.len() > MAX_DELEGATES_NUMBER {
            return Err(RepresentativeVoteContractErrors::DelegateLimitReached);
        }

        let mut delegates_map = Map::new(&env);
        for delegate in delegates.iter() {
            delegates_map.set(delegate, ());
        }
        env.storage().persistent().set(
            &RepresentativeVoteContractDataKey::Delegates,
            &delegates_map,
        );

        env.storage().persistent().extend_ttl(
            &RepresentativeVoteContractDataKey::Delegates,
            DELEGATES_TTL_EXTENSION,
            DELEGATES_TTL_EXTENSION,
        );

        env.events().publish(("SET_DELEGATES",), delegates);
        Ok(())
    }

    pub fn add_delegates(
        env: Env,
        to_add: Vec<Address>,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&RepresentativeVoteContractDataKey::Admin)
            .ok_or(RepresentativeVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        let mut delegates_map: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));

        for delegate in to_add.iter() {
            if !delegates_map.contains_key(delegate.clone()) {
                delegates_map.set(delegate, ());
            }
        }

        if delegates_map.len() > MAX_DELEGATES_NUMBER {
            return Err(RepresentativeVoteContractErrors::DelegateLimitReached);
        }

        env.storage().persistent().set(
            &RepresentativeVoteContractDataKey::Delegates,
            &delegates_map,
        );

        env.storage().persistent().extend_ttl(
            &RepresentativeVoteContractDataKey::Delegates,
            DELEGATES_TTL_EXTENSION,
            DELEGATES_TTL_EXTENSION,
        );

        env.events().publish(("ADD_DELEGATES",), to_add);
        Ok(())
    }

    pub fn remove_delegates(
        env: Env,
        to_remove: Vec<Address>,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&RepresentativeVoteContractDataKey::Admin)
            .ok_or(RepresentativeVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        let mut delegates_map: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));

        for delegate in to_remove.iter() {
            delegates_map.remove(delegate);
        }

        env.storage().persistent().set(
            &RepresentativeVoteContractDataKey::Delegates,
            &delegates_map,
        );

        env.storage().persistent().extend_ttl(
            &RepresentativeVoteContractDataKey::Delegates,
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
    ) -> Result<(), RepresentativeVoteContractErrors> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&RepresentativeVoteContractDataKey::Admin)
            .ok_or(RepresentativeVoteContractErrors::ContractNotInitialized)?;
        admin.require_auth();

        let now = env.ledger().timestamp();
        if delegation_deadline < now {
            return Err(RepresentativeVoteContractErrors::DelegationDeadlineInPast);
        }
        if delegation_deadline >= start_time {
            return Err(RepresentativeVoteContractErrors::DelegationDeadlineAfterStart);
        }
        if start_time < now {
            return Err(RepresentativeVoteContractErrors::StartTimeInPast);
        }
        if start_time >= end_time {
            return Err(RepresentativeVoteContractErrors::StartTimeAfterEnd);
        }
        if end_time - start_time > MAX_PROPOSAL_DURATION {
            return Err(RepresentativeVoteContractErrors::DurationTooLong);
        }
        if end_time - start_time < MIN_PROPOSAL_DURATION {
            return Err(RepresentativeVoteContractErrors::DurationTooShort);
        }

        let proposal_key = RepresentativeVoteContractDataKey::Proposal(id.clone());
        if env.storage().persistent().has(&proposal_key) {
            return Err(RepresentativeVoteContractErrors::ProposalAlreadyExists);
        }

        let proposal = RepresentativeVoteProposalData {
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
            .get(&RepresentativeVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        proposals.push_back(id.clone());
        env.storage()
            .persistent()
            .set(&RepresentativeVoteContractDataKey::Proposals, &proposals);
        env.storage().persistent().extend_ttl(
            &RepresentativeVoteContractDataKey::Proposals,
            PROPOSALS_TTL_EXTENSION,
            PROPOSALS_TTL_EXTENSION,
        );

        env.events().publish(("PROPOSAL", "CREATED"), id);
        Ok(())
    }

    pub fn delegate(
        env: Env,
        delegator: Address,
        id: Symbol,
        delegate_address: Address,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        delegator.require_auth();

        let proposal_key = RepresentativeVoteContractDataKey::Proposal(id.clone());
        let proposal: RepresentativeVoteProposalData = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(RepresentativeVoteContractErrors::ProposalNotFound)?;

        let now = env.ledger().timestamp();
        if now >= proposal.delegation_deadline {
            return Err(RepresentativeVoteContractErrors::DelegationDeadlineEnded);
        }

        let delegator_delegation_key =
            RepresentativeVoteContractDataKey::DelegatorData(id.clone(), delegator.clone());
        if env.storage().persistent().has(&delegator_delegation_key) {
            return Err(RepresentativeVoteContractErrors::DelegatorAlreadyDelegated);
        }

        let delegates: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));
        if !delegates.contains_key(delegate_address.clone()) {
            return Err(RepresentativeVoteContractErrors::DelegateNotFound);
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&RepresentativeVoteContractDataKey::Token)
            .ok_or(RepresentativeVoteContractErrors::ContractNotInitialized)?;
        let token_client = TokenClient::new(&env, &token_address);
        let balance = token_client.balance(&delegator);
        if balance <= 0 {
            return Err(RepresentativeVoteContractErrors::DelegatorCannotDelegate);
        }

        let representative_power_key =
            RepresentativeVoteContractDataKey::DelegateData(id.clone(), delegate_address.clone());
        let current_power_data: Option<RepresentativeVotePowerData> =
            env.storage().persistent().get(&representative_power_key);

        let new_power_data = match current_power_data {
            Some(mut data) => {
                data.accumulated_power = data.accumulated_power.saturating_add(balance);
                data
            }
            None => RepresentativeVotePowerData {
                accumulated_power: balance,
                choice: None,
            },
        };

        env.storage()
            .persistent()
            .set(&representative_power_key, &new_power_data);

        let delegation_ttl = Self::calculate_delegation_ttl(&env, proposal.end_time);
        env.storage().persistent().extend_ttl(
            &representative_power_key,
            delegation_ttl,
            delegation_ttl,
        );

        let delegation = RepresentativeVoteDelegationData {
            delegate: delegate_address.clone(),
            amount: balance,
            timestamp: now,
        };
        env.storage()
            .persistent()
            .set(&delegator_delegation_key, &delegation);
        env.storage().persistent().extend_ttl(
            &delegator_delegation_key,
            delegation_ttl,
            delegation_ttl,
        );

        env.events()
            .publish(("DELEGATE", id, delegator), delegate_address);
        Ok(())
    }

    pub fn revoke_delegation(
        env: Env,
        delegator: Address,
        id: Symbol,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        delegator.require_auth();

        let proposal_key = RepresentativeVoteContractDataKey::Proposal(id.clone());
        let proposal: RepresentativeVoteProposalData = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(RepresentativeVoteContractErrors::ProposalNotFound)?;

        let now = env.ledger().timestamp();
        if now >= proposal.delegation_deadline {
            return Err(RepresentativeVoteContractErrors::DelegationDeadlineEnded);
        }

        let delegator_delegation_key =
            RepresentativeVoteContractDataKey::DelegatorData(id.clone(), delegator.clone());
        let delegation: RepresentativeVoteDelegationData = env
            .storage()
            .persistent()
            .get(&delegator_delegation_key)
            .ok_or(RepresentativeVoteContractErrors::DelegatorAlreadyDelegated)?;

        let grace_period_end = delegation.timestamp.saturating_add(DELEGATION_GRACE_PERIOD);
        let effective_grace_end = grace_period_end.min(proposal.delegation_deadline);

        if now > effective_grace_end {
            return Err(RepresentativeVoteContractErrors::DelegationGracePeriodEnded);
        }

        let representative_power_key = RepresentativeVoteContractDataKey::DelegateData(
            id.clone(),
            delegation.delegate.clone(),
        );
        let current_power_data: Option<RepresentativeVotePowerData> =
            env.storage().persistent().get(&representative_power_key);

        if let Some(mut data) = current_power_data {
            data.accumulated_power = data.accumulated_power.saturating_sub(delegation.amount);

            if data.accumulated_power > 0 {
                env.storage()
                    .persistent()
                    .set(&representative_power_key, &data);
            } else {
                env.storage().persistent().remove(&representative_power_key);
            }
        }

        env.storage().persistent().remove(&delegator_delegation_key);

        env.events()
            .publish(("REVOKE_DELEGATION", id, delegator), delegation.delegate);
        Ok(())
    }

    pub fn vote(
        env: Env,
        delegate: Address,
        id: Symbol,
        choice: Symbol,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        delegate.require_auth();

        let delegates: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));
        if !delegates.contains_key(delegate.clone()) {
            return Err(RepresentativeVoteContractErrors::NotDelegate);
        }

        let proposal_key = RepresentativeVoteContractDataKey::Proposal(id.clone());
        let mut proposal: RepresentativeVoteProposalData = env
            .storage()
            .persistent()
            .get(&proposal_key)
            .ok_or(RepresentativeVoteContractErrors::ProposalNotFound)?;

        let now = env.ledger().timestamp();
        if now < proposal.start_time || now > proposal.end_time {
            return Err(RepresentativeVoteContractErrors::VotingNotActive);
        }

        let representative_power_key =
            RepresentativeVoteContractDataKey::DelegateData(id.clone(), delegate.clone());

        let mut power_data: RepresentativeVotePowerData = env
            .storage()
            .persistent()
            .get(&representative_power_key)
            .unwrap_or(RepresentativeVotePowerData {
                accumulated_power: 0,
                choice: None,
            });

        if power_data.choice.is_some() {
            return Err(RepresentativeVoteContractErrors::DelegateAlreadyVoted);
        }

        let power = power_data.accumulated_power;

        if choice == VOTE_FOR {
            proposal.total_for = proposal.total_for.saturating_add(power);
        } else if choice == VOTE_AGAINST {
            proposal.total_against = proposal.total_against.saturating_add(power);
        } else if choice == VOTE_ABSTAIN {
            proposal.total_abstain = proposal.total_abstain.saturating_add(power);
        } else {
            return Err(RepresentativeVoteContractErrors::InvalidChoice);
        }

        env.storage().persistent().set(&proposal_key, &proposal);

        let proposal_ttl = Self::calculate_proposal_ttl(&env, proposal.end_time);
        env.storage()
            .persistent()
            .extend_ttl(&proposal_key, proposal_ttl, proposal_ttl);

        power_data.choice = Some(choice.clone());
        env.storage()
            .persistent()
            .set(&representative_power_key, &power_data);

        let delegation_ttl = Self::calculate_delegation_ttl(&env, proposal.end_time);
        env.storage().persistent().extend_ttl(
            &representative_power_key,
            delegation_ttl,
            delegation_ttl,
        );

        env.events()
            .publish(("VOTE", id, delegate), (choice, power));
        Ok(())
    }

    pub fn transfer_admin(
        env: Env,
        new_admin: Address,
    ) -> Result<(), RepresentativeVoteContractErrors> {
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&RepresentativeVoteContractDataKey::Admin)
            .ok_or(RepresentativeVoteContractErrors::ContractNotInitialized)?;

        current_admin.require_auth();

        env.storage()
            .instance()
            .set(&RepresentativeVoteContractDataKey::Admin, &new_admin);

        env.events()
            .publish(("ADMIN", "TRANSFERRED"), (current_admin, new_admin));
        Ok(())
    }

    pub fn get_governance_details(env: Env) -> Vec<RepresentativeVoteProposalSummary> {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        let mut summary = Vec::new(&env);
        let now = env.ledger().timestamp();

        for id in proposals.iter() {
            if let Some(proposal) = env
                .storage()
                .persistent()
                .get::<_, RepresentativeVoteProposalData>(
                    &RepresentativeVoteContractDataKey::Proposal(id.clone()),
                )
            {
                let status = now >= proposal.start_time && now <= proposal.end_time;
                summary.push_back(RepresentativeVoteProposalSummary {
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
    ) -> Result<RepresentativeVoteProposalData, RepresentativeVoteContractErrors> {
        let proposal: RepresentativeVoteProposalData = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Proposal(id))
            .ok_or(RepresentativeVoteContractErrors::ProposalNotFound)?;
        Ok(proposal)
    }

    pub fn get_delegator_details(
        env: Env,
        delegator: Address,
    ) -> Result<Vec<(Symbol, bool, i128, bool, u64)>, RepresentativeVoteContractErrors> {
        let proposals: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Proposals)
            .unwrap_or(Vec::new(&env));
        let mut results = Vec::new(&env);

        let delegates: Map<Address, ()> = env
            .storage()
            .persistent()
            .get(&RepresentativeVoteContractDataKey::Delegates)
            .unwrap_or(Map::new(&env));
        let is_delegate = delegates.contains_key(delegator.clone());
        let now = env.ledger().timestamp();

        for id in proposals.iter() {
            let delegator_delegation_key =
                RepresentativeVoteContractDataKey::DelegatorData(id.clone(), delegator.clone());

            if let Some(delegation) = env
                .storage()
                .persistent()
                .get::<_, RepresentativeVoteDelegationData>(&delegator_delegation_key)
            {
                let (can_revoke, remaining_time) =
                    Self::check_revocation_eligibility(&env, &delegator, &id, now);
                results.push_back((
                    id.clone(),
                    true,
                    delegation.amount,
                    can_revoke,
                    remaining_time,
                ));
            } else if is_delegate {
                let representative_power_key =
                    RepresentativeVoteContractDataKey::DelegateData(id.clone(), delegator.clone());
                let power_data: RepresentativeVotePowerData = env
                    .storage()
                    .persistent()
                    .get(&representative_power_key)
                    .unwrap_or(RepresentativeVotePowerData {
                        accumulated_power: 0,
                        choice: None,
                    });
                results.push_back((id.clone(), false, power_data.accumulated_power, false, 0));
            } else {
                results.push_back((id.clone(), false, 0, false, 0));
            }
        }
        Ok(results)
    }

    pub fn get_user_details(
        env: Env,
        user: Address,
    ) -> Result<Vec<(Symbol, bool, i128, bool, u64)>, RepresentativeVoteContractErrors> {
        Self::get_delegator_details(env, user)
    }
}
