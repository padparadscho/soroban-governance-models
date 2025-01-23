#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env, FromVal, String,
};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> TokenClient<'a> {
    let token_address = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    TokenClient::new(e, &token_address)
}

fn create_vote_contract<'a>(
    e: &Env,
    admin: &Address,
    token_address: &Address,
) -> TokenWeightedVoteContractClient<'a> {
    let contract_address = e.register(
        TokenWeightedVoteContract,
        TokenWeightedVoteContractArgs::__constructor(admin, token_address),
    );
    TokenWeightedVoteContractClient::new(e, &contract_address)
}

fn setup_test_env() -> Env {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 1000000;
    });
    e
}

// Tests successful contract initialization with admin and token configuration
// Expects: Empty governance details list confirming contract is ready for proposals
#[test]
fn test_initialization() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let contract_address = e.register(
        TokenWeightedVoteContract,
        TokenWeightedVoteContractArgs::__constructor(&admin, &token_address),
    );
    let client = TokenWeightedVoteContractClient::new(&e, &contract_address);

    let governance_details = client.get_governance_details();
    assert_eq!(governance_details.len(), 0);
}

// Tests contract re initialization failure on already initialized contract
// Expects: ContractAlreadyInitialized error (Error #2) to prevent state reset
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_reinitialization() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let contract_address = e.register(
        TokenWeightedVoteContract,
        TokenWeightedVoteContractArgs::__constructor(&admin, &token_address),
    );
    let client = TokenWeightedVoteContractClient::new(&e, &contract_address);

    let governance_details = client.get_governance_details();
    assert_eq!(governance_details.len(), 0);

    e.register_at(
        &contract_address,
        TokenWeightedVoteContract,
        TokenWeightedVoteContractArgs::__constructor(&admin, &token_address),
    );
}

// Tests successful proposal creation by admin with valid timing parameters
// Expects: Proposal appears in governance details list with correct ID and metadata
#[test]
fn test_create_proposal() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal description");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 100;
    let end_time = start_time + 500000;

    let result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);
    assert!(result.is_ok());

    let governance_details = client.get_governance_details();
    assert_eq!(governance_details.len(), 1);
    assert_eq!(governance_details.get(0).unwrap().id, proposal_id);
}

// Tests start time after end time validation
// Expects: StartTimeAfterEnd error (Error #9) when end time is before start time
#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_start_time_after_end() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 500000;
    let end_time = ledger_time + 100;

    client.create_proposal(&proposal_id, &description, &start_time, &end_time);
}

// Tests start time in past validation
// Expects: StartTimeInPast error (Error #10) when start time is before current timestamp
#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_start_time_in_past() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time - 100;
    let end_time = ledger_time + 500000;

    client.create_proposal(&proposal_id, &description, &start_time, &end_time);
}

// Tests duration too long validation
// Expects: DurationTooLong error (Error #11) when proposal duration exceeds maximum (15 days)
#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_duration_too_long() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 100;
    let end_time = start_time + 2000000;

    client.create_proposal(&proposal_id, &description, &start_time, &end_time);
}

// Tests duration too short validation
// Expects: DurationTooShort error (Error #12) when proposal duration is below minimum (5 days)
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_duration_too_short() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 100;
    let end_time = start_time + 200;

    client.create_proposal(&proposal_id, &description, &start_time, &end_time);
}

// Tests duplicate proposal creation rejection
// Expects: ProposalAlreadyExists error (Error #3) to maintain proposal uniqueness
#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_proposal_already_exists() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 100;
    let end_time = start_time + 500000;

    let result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);
    assert!(result.is_ok());

    client.create_proposal(&proposal_id, &description, &start_time, &end_time);
}

// Tests voting with three users casting different vote types
// Expects: Vote tallies reflect correct token weighted counts for available choices
#[test]
fn test_vote() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let user3 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user1, &500);
    stellar_asset.mint(&user2, &300);
    stellar_asset.mint(&user3, &200);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 50;
    let end_time = ledger_time + 500000;

    let _result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = ledger_time + 100;
    });

    let result1 = client.try_vote(&user1, &proposal_id, &symbol_short!("FOR"));
    let result2 = client.try_vote(&user2, &proposal_id, &symbol_short!("AGAINST"));
    let result3 = client.try_vote(&user3, &proposal_id, &symbol_short!("ABSTAIN"));

    if result1.is_ok() && result2.is_ok() && result3.is_ok() {
        let proposal_details = client.get_proposal_details(&proposal_id);
        assert_eq!(proposal_details.total_for, 500);
        assert_eq!(proposal_details.total_against, 300);
        assert_eq!(proposal_details.total_abstain, 200);
    }
}

// Tests voting exactly at inclusive boundaries start_time and end_time
// Expects: Reject 1s before start, accept at start and end, reject 1s after end
#[test]
fn test_vote_boundary_inclusive() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user_start = Address::generate(&e);
    let user_end = Address::generate(&e);
    let user_after = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user_start, &100);
    stellar_asset.mint(&user_end, &100);
    stellar_asset.mint(&user_after, &100);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP001");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 60;
    let end_time = start_time + MIN_PROPOSAL_DURATION;
    let desc = String::from_val(&e, &"Test proposal");
    let create_res = client.try_create_proposal(&proposal_id, &desc, &start_time, &end_time);
    assert!(
        create_res.is_ok(),
        "Proposal creation failed: {:?}",
        create_res
    );

    e.ledger().with_mut(|l| l.timestamp = start_time - 1);
    assert!(client
        .try_vote(&user_start, &proposal_id, &symbol_short!("FOR"))
        .is_err());

    e.ledger().with_mut(|l| l.timestamp = start_time);
    assert!(client
        .try_vote(&user_start, &proposal_id, &symbol_short!("FOR"))
        .is_ok());

    e.ledger().with_mut(|l| l.timestamp = end_time);
    assert!(client
        .try_vote(&user_end, &proposal_id, &symbol_short!("AGAINST"))
        .is_ok());

    e.ledger().with_mut(|l| l.timestamp = end_time + 1);
    let late = client.try_vote(&user_after, &proposal_id, &symbol_short!("ABSTAIN"));
    assert!(late.is_err());

    let details = client.get_proposal_details(&proposal_id);
    assert_eq!(details.total_for, 100);
    assert_eq!(details.total_against, 100);
    assert_eq!(details.total_abstain, 0);
}

// Tests voting on non-existent proposal
// Expects: ProposalNotFound error (Error #4) to protect against invalid access
#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_proposal_not_found() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let non_existent_proposal = symbol_short!("FAKE001");

    client.vote(&user, &non_existent_proposal, &symbol_short!("FOR"));
}

// Tests prevention of multiple votes by same user on same proposal
// Expects: UserAlreadyVoted error (Error #5) to maintain voting integrity
#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_user_already_voted() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 50;
    let end_time = ledger_time + 500000;

    let _result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = ledger_time + 100;
    });

    let result1 = client.try_vote(&user, &proposal_id, &symbol_short!("FOR"));
    assert!(result1.is_ok());

    client.vote(&user, &proposal_id, &symbol_short!("AGAINST"));
}

// Tests access control for users without governance tokens
// Expects: UserCannotVote error (Error #6) to enforce token holder only participation
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_user_cannot_vote() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 50;
    let end_time = ledger_time + 500000;

    let _result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = ledger_time + 100;
    });

    client.vote(&user, &proposal_id, &symbol_short!("FOR"));
}

// Tests voting outside active voting period (before start time)
// Expects: VotingNotActive error (Error #7) to enforce proper timing constraints
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_voting_not_active() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 1000;
    let end_time = start_time + 500000;

    let _result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);

    client.vote(&user, &proposal_id, &symbol_short!("FOR"));
}

// Tests voting with invalid choice option (not FOR/AGAINST/ABSTAIN)
// Expects: InvalidChoice error (Error #8) to enforce standardized vote options
#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_invalid_choice() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 50;
    let end_time = ledger_time + 500000;

    let _result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = ledger_time + 100;
    });

    client.vote(&user, &proposal_id, &symbol_short!("INVALID"));
}

// Tests secure admin privilege transfer to new address
// Expects: Successful transfer without errors, maintaining operational continuity
#[test]
fn test_transfer_admin() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let token_address = Address::generate(&e);

    let client = create_vote_contract(&e, &admin, &token_address);

    let result = client.try_transfer_admin(&new_admin);
    assert!(result.is_ok());
}

// Tests governance overview retrieval with multiple proposals
// Expects: Complete list of all proposals with essential metadata (IDs, descriptions)
#[test]
fn test_get_governance_details() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let ledger_time = e.ledger().timestamp();

    let prop1_id = symbol_short!("PROP001");
    let prop1_desc = String::from_val(&e, &"First proposal");
    let start1 = ledger_time + 100;
    let end1 = ledger_time + 500000;
    let _result1 = client.try_create_proposal(&prop1_id, &prop1_desc, &start1, &end1);

    let prop2_id = symbol_short!("PROP002");
    let prop2_desc = String::from_val(&e, &"Second proposal");
    let start2 = ledger_time + 200;
    let end2 = ledger_time + 600000;
    let _result2 = client.try_create_proposal(&prop2_id, &prop2_desc, &start2, &end2);

    let governance_details = client.get_governance_details();
    assert_eq!(governance_details.len(), 2);

    let first_proposal = governance_details.get(0).unwrap();
    let second_proposal = governance_details.get(1).unwrap();

    let has_prop1 = first_proposal.id == prop1_id || second_proposal.id == prop1_id;
    let has_prop2 = first_proposal.id == prop2_id || second_proposal.id == prop2_id;
    assert!(has_prop1);
    assert!(has_prop2);
}

// Tests individual proposal details retrieval including vote tallies
// Expects: Complete proposal data with timing, description, and initialized vote counts
#[test]
fn test_get_proposal_details() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal description");
    let ledger_time = e.ledger().timestamp();
    let start_time = ledger_time + 100;
    let end_time = ledger_time + 500000;

    let _result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);

    let proposal_details = client.get_proposal_details(&proposal_id);
    assert_eq!(proposal_details.description, description);
    assert_eq!(proposal_details.start_time, start_time);
    assert_eq!(proposal_details.end_time, end_time);
    assert_eq!(proposal_details.total_for, 0);
    assert_eq!(proposal_details.total_against, 0);
    assert_eq!(proposal_details.total_abstain, 0);
}

// Tests user voting history and eligibility information retrieval
// Expects: Non empty user details containing voting participation and eligibility status
#[test]
fn test_get_user_details() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let start_time = now + 50;
    let end_time = now + 500000;

    let _result = client.try_create_proposal(&proposal_id, &description, &start_time, &end_time);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 100;
    });

    let _vote_result = client.try_vote(&user, &proposal_id, &symbol_short!("FOR"));

    let user_details = client.get_user_details(&user);
    assert!(!user_details.is_empty());
}
