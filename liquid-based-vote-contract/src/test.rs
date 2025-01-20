#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env, FromVal, String, Vec,
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
) -> LiquidBasedVoteContractClient<'a> {
    let contract_address = e.register(
        LiquidBasedVoteContract,
        LiquidBasedVoteContractArgs::__constructor(admin, token_address),
    );
    LiquidBasedVoteContractClient::new(e, &contract_address)
}

fn setup_test_env() -> Env {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 1000000;
    });
    e
}

#[test]
fn test_initialization_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);

    let client = create_vote_contract(&e, &admin, &token_address);
    let governance_details = client.get_governance_details();
    assert_eq!(governance_details.len(), 0);
}

#[test]
fn test_delegate_management_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let delegate1 = Address::generate(&e);
    let delegate2 = Address::generate(&e);
    let delegate3 = Address::generate(&e);

    let client = create_vote_contract(&e, &admin, &token_address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate1.clone());
    delegates.push_back(delegate2.clone());

    let set_result = client.try_set_delegates(&delegates);
    assert!(set_result.is_ok());

    let mut add_delegates = Vec::new(&e);
    add_delegates.push_back(delegate3.clone());

    let add_result = client.try_add_delegates(&add_delegates);
    assert!(add_result.is_ok());

    let mut remove_delegates = Vec::new(&e);
    remove_delegates.push_back(delegate1.clone());

    let remove_result = client.try_remove_delegates(&remove_delegates);
    assert!(remove_result.is_ok());
}

#[test]
fn test_delegate_management_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);

    let client = create_vote_contract(&e, &admin, &token_address);

    let mut too_many_delegates = Vec::new(&e);
    for _i in 0..15 {
        too_many_delegates.push_back(Address::generate(&e));
    }

    let limit_result = client.try_set_delegates(&too_many_delegates);
    assert!(limit_result.is_err());
}

#[test]
fn test_create_proposal_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal description");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 50;
    let start_time = now + 100;
    let end_time = start_time + 500000;

    let result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );
    assert!(result.is_ok());

    let governance_details = client.get_governance_details();
    assert_eq!(governance_details.len(), 1);
    assert_eq!(governance_details.get(0).unwrap().id, proposal_id);
}

#[test]
fn test_create_proposal_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP001");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();

    let result1 = client.try_create_proposal(
        &proposal_id,
        &description,
        &(now - 100),
        &(now + 100),
        &(now + 500000),
    );
    assert!(result1.is_err());

    let result2 = client.try_create_proposal(
        &proposal_id,
        &description,
        &(now + 100),
        &(now + 500000),
        &(now + 100),
    );
    assert!(result2.is_err());

    let result3 = client.try_create_proposal(
        &proposal_id,
        &description,
        &(now + 200),
        &(now + 100),
        &(now + 500000),
    );
    assert!(result3.is_err());

    let result4 = client.try_create_proposal(
        &proposal_id,
        &description,
        &(now + 100),
        &(now + 200),
        &(now + 300),
    );
    assert!(result4.is_err());
}

#[test]
fn test_direct_vote_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user1, &1000);
    stellar_asset.mint(&user2, &500);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = start_time + 50;
    });

    let vote_result1 = client.try_vote(&user1, &proposal_id, &symbol_short!("FOR"));
    let vote_result2 = client.try_vote(&user2, &proposal_id, &symbol_short!("AGAINST"));

    assert!(vote_result1.is_ok());
    assert!(vote_result2.is_ok());

    let proposal_details = client.get_proposal_details(&proposal_id);
    assert_eq!(proposal_details.total_for, 1000);
    assert_eq!(proposal_details.total_against, 500);
    assert_eq!(proposal_details.total_abstain, 0);
}

#[test]
fn test_delegation_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let delegate1 = Address::generate(&e);
    let delegate2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user1, &1000);
    stellar_asset.mint(&user2, &500);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate1.clone());
    delegates.push_back(delegate2.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 50;
    });

    let delegate_result1 = client.try_delegate(&user1, &proposal_id, &delegate1);
    let delegate_result2 = client.try_delegate(&user2, &proposal_id, &delegate2);

    assert!(delegate_result1.is_ok());
    assert!(delegate_result2.is_ok());
}

#[test]
fn test_delegation_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);
    let delegate = Address::generate(&e);
    let non_delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    let non_delegate_result = client.try_delegate(&user, &proposal_id, &non_delegate);
    assert!(non_delegate_result.is_err());

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = delegation_deadline + 50;
    });

    let deadline_result = client.try_delegate(&user, &proposal_id, &delegate);
    assert!(deadline_result.is_err());
}

#[test]
fn test_delegate_vote_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let user3 = Address::generate(&e);
    let delegate1 = Address::generate(&e);
    let delegate2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user1, &1000);
    stellar_asset.mint(&user2, &500);
    stellar_asset.mint(&user3, &300);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate1.clone());
    delegates.push_back(delegate2.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 50;
    });

    let _delegate_result1 = client.try_delegate(&user1, &proposal_id, &delegate1);
    let _delegate_result2 = client.try_delegate(&user2, &proposal_id, &delegate1);
    let _delegate_result3 = client.try_delegate(&user3, &proposal_id, &delegate2);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = start_time + 50;
    });

    let vote_result1 = client.try_delegate_vote(&delegate1, &proposal_id, &symbol_short!("FOR"));
    let vote_result2 =
        client.try_delegate_vote(&delegate2, &proposal_id, &symbol_short!("AGAINST"));

    assert!(vote_result1.is_ok());
    assert!(vote_result2.is_ok());

    let proposal_details = client.get_proposal_details(&proposal_id);
    assert_eq!(proposal_details.total_for, 1500);
    assert_eq!(proposal_details.total_against, 300);
    assert_eq!(proposal_details.total_abstain, 0);
}

#[test]
fn test_liquid_democracy_hybrid_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let direct_voter1 = Address::generate(&e);
    let direct_voter2 = Address::generate(&e);
    let delegator1 = Address::generate(&e);
    let delegator2 = Address::generate(&e);
    let delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&direct_voter1, &2000);
    stellar_asset.mint(&direct_voter2, &1500);
    stellar_asset.mint(&delegator1, &1000);
    stellar_asset.mint(&delegator2, &800);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test hybrid proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 50;
    });

    let _delegate_result1 = client.try_delegate(&delegator1, &proposal_id, &delegate);
    let _delegate_result2 = client.try_delegate(&delegator2, &proposal_id, &delegate);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = start_time + 50;
    });

    let _direct_vote1 = client.try_vote(&direct_voter1, &proposal_id, &symbol_short!("FOR"));
    let _direct_vote2 = client.try_vote(&direct_voter2, &proposal_id, &symbol_short!("AGAINST"));

    let _delegate_vote = client.try_delegate_vote(&delegate, &proposal_id, &symbol_short!("FOR"));

    let proposal_details = client.get_proposal_details(&proposal_id);
    assert_eq!(proposal_details.total_for, 3800);
    assert_eq!(proposal_details.total_against, 1500);
    assert_eq!(proposal_details.total_abstain, 0);
}

#[test]
fn test_mutual_exclusivity_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);
    let delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 50;
    });

    let _delegate_result = client.try_delegate(&user, &proposal_id, &delegate);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = start_time + 50;
    });

    let vote_result = client.try_vote(&user, &proposal_id, &symbol_short!("FOR"));
    assert!(vote_result.is_err());
}

#[test]
fn test_transfer_admin_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let token_address = Address::generate(&e);

    let client = create_vote_contract(&e, &admin, &token_address);

    let result = client.try_transfer_admin(&new_admin);
    assert!(result.is_ok());
}

#[test]
fn test_get_governance_details_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let now = e.ledger().timestamp();

    let prop1_id = symbol_short!("PROP001");
    let prop1_desc = String::from_val(&e, &"First proposal");
    let delegation1 = now + 50;
    let start1 = now + 100;
    let end1 = now + 500000;
    let _result1 = client.try_create_proposal(&prop1_id, &prop1_desc, &delegation1, &start1, &end1);

    let prop2_id = symbol_short!("PROP002");
    let prop2_desc = String::from_val(&e, &"Second proposal");
    let delegation2 = now + 60;
    let start2 = now + 200;
    let end2 = now + 600000;
    let _result2 = client.try_create_proposal(&prop2_id, &prop2_desc, &delegation2, &start2, &end2);

    let governance_details = client.get_governance_details();
    assert_eq!(governance_details.len(), 2);

    let first_proposal = governance_details.get(0).unwrap();
    let second_proposal = governance_details.get(1).unwrap();

    let has_prop1 = first_proposal.id == prop1_id || second_proposal.id == prop1_id;
    let has_prop2 = first_proposal.id == prop2_id || second_proposal.id == prop2_id;
    assert!(has_prop1);
    assert!(has_prop2);
}

#[test]
fn test_get_proposal_details_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let token_address = Address::generate(&e);
    let client = create_vote_contract(&e, &admin, &token_address);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal description");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 50;
    let start_time = now + 100;
    let end_time = start_time + 500000;

    let _result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    let details = client.get_proposal_details(&proposal_id);
    assert_eq!(details.description, description);
    assert_eq!(details.delegation_deadline, delegation_deadline);
    assert_eq!(details.start_time, start_time);
    assert_eq!(details.end_time, end_time);
    assert_eq!(details.total_for, 0);
    assert_eq!(details.total_against, 0);
    assert_eq!(details.total_abstain, 0);
}

#[test]
fn test_get_user_details_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);
    let delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1500);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = now + 500000;

    let _result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 50;
    });

    let _delegate_result = client.try_delegate(&user, &proposal_id, &delegate);

    let user_details = client.get_user_details(&user);
    assert!(!user_details.is_empty());
}

#[test]
fn test_direct_vote_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = start_time - 50;
    });

    let early_vote_result = client.try_vote(&user, &proposal_id, &symbol_short!("FOR"));
    assert!(early_vote_result.is_err());

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = end_time + 50;
    });

    let late_vote_result = client.try_vote(&user, &proposal_id, &symbol_short!("FOR"));
    assert!(late_vote_result.is_err());

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = start_time + 50;
    });

    let invalid_choice_result = client.try_vote(&user, &proposal_id, &symbol_short!("INVALID"));
    assert!(invalid_choice_result.is_err());
}

#[test]
fn test_delegate_vote_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let user = Address::generate(&e);
    let delegate = Address::generate(&e);
    let non_delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&user, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 100;
    let start_time = now + 200;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 50;
    });

    let _delegate_result = client.try_delegate(&user, &proposal_id, &delegate);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = start_time + 50;
    });

    let non_delegate_vote =
        client.try_delegate_vote(&non_delegate, &proposal_id, &symbol_short!("FOR"));
    assert!(non_delegate_vote.is_err());

    let _vote_result = client.try_delegate_vote(&delegate, &proposal_id, &symbol_short!("FOR"));

    let double_vote_result =
        client.try_delegate_vote(&delegate, &proposal_id, &symbol_short!("AGAINST"));
    assert!(double_vote_result.is_err());
}
