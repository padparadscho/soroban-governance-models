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
) -> RepresentativeVoteContractClient<'a> {
    let contract_address = e.register(
        RepresentativeVoteContract,
        RepresentativeVoteContractArgs::__constructor(admin, token_address),
    );
    RepresentativeVoteContractClient::new(e, &contract_address)
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
fn test_delegation_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator1 = Address::generate(&e);
    let delegator2 = Address::generate(&e);
    let delegate1 = Address::generate(&e);
    let delegate2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator1, &1000);
    stellar_asset.mint(&delegator2, &500);

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

    let delegate_result1 = client.try_delegate(&delegator1, &proposal_id, &delegate1);
    let delegate_result2 = client.try_delegate(&delegator2, &proposal_id, &delegate2);

    assert!(delegate_result1.is_ok());
    assert!(delegate_result2.is_ok());
}

#[test]
fn test_delegation_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator = Address::generate(&e);
    let delegate = Address::generate(&e);
    let non_delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator, &1000);

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

    let non_delegate_result = client.try_delegate(&delegator, &proposal_id, &non_delegate);
    assert!(non_delegate_result.is_err());

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 150;
    });

    let deadline_result = client.try_delegate(&delegator, &proposal_id, &delegate);
    assert!(deadline_result.is_err());
}

#[test]
fn test_vote_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator1 = Address::generate(&e);
    let delegator2 = Address::generate(&e);
    let delegator3 = Address::generate(&e);
    let delegate1 = Address::generate(&e);
    let delegate2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator1, &1000);
    stellar_asset.mint(&delegator2, &500);
    stellar_asset.mint(&delegator3, &300);

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

    let _delegate_result1 = client.try_delegate(&delegator1, &proposal_id, &delegate1);
    let _delegate_result2 = client.try_delegate(&delegator2, &proposal_id, &delegate1);
    let _delegate_result3 = client.try_delegate(&delegator3, &proposal_id, &delegate2);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 250;
    });

    let vote_result1 = client.try_vote(&delegate1, &proposal_id, &symbol_short!("FOR"));
    let vote_result2 = client.try_vote(&delegate2, &proposal_id, &symbol_short!("AGAINST"));

    assert!(vote_result1.is_ok());
    assert!(vote_result2.is_ok());

    let proposal_details = client.get_proposal_details(&proposal_id);
    assert_eq!(proposal_details.total_for, 1500);
    assert_eq!(proposal_details.total_against, 300);
    assert_eq!(proposal_details.total_abstain, 0);
}

#[test]
fn test_vote_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator = Address::generate(&e);
    let delegate = Address::generate(&e);
    let non_delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator, &1000);

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

    let _delegate_result = client.try_delegate(&delegator, &proposal_id, &delegate);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 250;
    });

    let non_delegate_vote = client.try_vote(&non_delegate, &proposal_id, &symbol_short!("FOR"));
    assert!(non_delegate_vote.is_err());

    let _vote_result = client.try_vote(&delegate, &proposal_id, &symbol_short!("FOR"));

    let double_vote_result = client.try_vote(&delegate, &proposal_id, &symbol_short!("AGAINST"));
    assert!(double_vote_result.is_err());
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
fn test_get_delegator_details_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator = Address::generate(&e);
    let delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator, &1500);

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

    let _delegate_result = client.try_delegate(&delegator, &proposal_id, &delegate);

    let delegator_details = client.get_user_details(&delegator);
    assert!(!delegator_details.is_empty());
}

#[test]
fn test_revoke_delegation_success() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator = Address::generate(&e);
    let delegate1 = Address::generate(&e);
    let delegate2 = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate1.clone());
    delegates.push_back(delegate2.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 200000;
    let start_time = now + 300000;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    let _delegate_result = client.try_delegate(&delegator, &proposal_id, &delegate1);

    let delegator_details = client.get_user_details(&delegator);
    assert_eq!(delegator_details.get(0).unwrap().1, true);
    assert_eq!(delegator_details.get(0).unwrap().2, 1000);
    assert_eq!(delegator_details.get(0).unwrap().3, true);
    assert!(delegator_details.get(0).unwrap().4 > 0);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 100000;
    });

    let revoke_result = client.try_revoke_delegation(&delegator, &proposal_id);
    assert!(revoke_result.is_ok());

    let delegator_details_after = client.get_user_details(&delegator);
    assert_eq!(delegator_details_after.get(0).unwrap().1, false);
    assert_eq!(delegator_details_after.get(0).unwrap().2, 0);
    assert_eq!(delegator_details_after.get(0).unwrap().3, false);
    assert_eq!(delegator_details_after.get(0).unwrap().4, 0);

    let re_delegate_result = client.try_delegate(&delegator, &proposal_id, &delegate2);
    assert!(re_delegate_result.is_ok());

    // Check final delegation
    let final_details = client.get_user_details(&delegator);
    assert_eq!(final_details.get(0).unwrap().1, true);
    assert_eq!(final_details.get(0).unwrap().2, 1000);
    assert_eq!(final_details.get(0).unwrap().3, true);
    assert!(final_details.get(0).unwrap().4 > 0);
}

#[test]
fn test_revoke_delegation_fails() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator = Address::generate(&e);
    let delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 200000;
    let start_time = now + 300000;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    let _delegate_result = client.try_delegate(&delegator, &proposal_id, &delegate);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 180000;
    });

    let revoke_result = client.try_revoke_delegation(&delegator, &proposal_id);
    assert!(revoke_result.is_err());

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 250000;
    });

    let late_revoke_result = client.try_revoke_delegation(&delegator, &proposal_id);
    assert!(late_revoke_result.is_err());

    let non_delegated_delegator = Address::generate(&e);
    stellar_asset.mint(&non_delegated_delegator, &500);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 50;
    });

    let no_delegation_result = client.try_revoke_delegation(&non_delegated_delegator, &proposal_id);
    assert!(no_delegation_result.is_err());
}

#[test]
fn test_can_revoke_delegation() {
    let e = setup_test_env();
    let admin = Address::generate(&e);
    let delegator = Address::generate(&e);
    let delegate = Address::generate(&e);

    let token = create_token_contract(&e, &admin);
    let stellar_asset = StellarAssetClient::new(&e, &token.address);
    stellar_asset.mint(&delegator, &1000);

    let client = create_vote_contract(&e, &admin, &token.address);

    let mut delegates = Vec::new(&e);
    delegates.push_back(delegate.clone());
    let _set_result = client.try_set_delegates(&delegates);

    let proposal_id = symbol_short!("PROP1");
    let description = String::from_val(&e, &"Test proposal");
    let now = e.ledger().timestamp();
    let delegation_deadline = now + 200000;
    let start_time = now + 300000;
    let end_time = start_time + 500000;

    let _proposal_result = client.try_create_proposal(
        &proposal_id,
        &description,
        &delegation_deadline,
        &start_time,
        &end_time,
    );

    let delegator_details_before = client.get_user_details(&delegator);
    let details_before = delegator_details_before
        .iter()
        .find(|d| d.0 == proposal_id)
        .unwrap();
    assert!(!details_before.3);
    assert_eq!(details_before.4, 0);

    let _delegate_result = client.try_delegate(&delegator, &proposal_id, &delegate);

    let delegator_details_after = client.get_user_details(&delegator);
    let details_after = delegator_details_after
        .iter()
        .find(|d| d.0 == proposal_id)
        .unwrap();
    assert!(details_after.3);
    assert!(details_after.4 > 170000);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 100000;
    });

    let delegator_details_partial = client.get_user_details(&delegator);
    let details_partial = delegator_details_partial
        .iter()
        .find(|d| d.0 == proposal_id)
        .unwrap();
    assert!(details_partial.3);
    assert!(details_partial.4 > 0);
    assert!(details_partial.4 < 100000);

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 180000;
    });

    let delegator_details_expired = client.get_user_details(&delegator);
    let details_expired = delegator_details_expired
        .iter()
        .find(|d| d.0 == proposal_id)
        .unwrap();
    assert!(!details_expired.3);
    assert_eq!(details_expired.4, 0);

    let delegator2 = Address::generate(&e);
    stellar_asset.mint(&delegator2, &500);

    let proposal_id2 = symbol_short!("PROP2");
    let short_deadline = now + 50000;
    let short_start = now + 60000;
    let short_end = short_start + 500000;

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = now + 30000;
    });

    let _proposal2_result = client.try_create_proposal(
        &proposal_id2,
        &description,
        &short_deadline,
        &short_start,
        &short_end,
    );

    let _delegate2_result = client.try_delegate(&delegator2, &proposal_id2, &delegate);

    let delegator_details_capped = client.get_user_details(&delegator2);
    let details_capped = delegator_details_capped
        .iter()
        .find(|d| d.0 == proposal_id2)
        .unwrap();
    assert!(details_capped.3);
    assert!(details_capped.4 <= 20000);
}
