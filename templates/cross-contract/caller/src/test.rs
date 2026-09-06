use super::*;
use receiver::Receiver;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol};

/// Test setup: registers both caller and receiver contracts
fn setup(env: &Env) -> (Address, Address) {
    env.mock_all_auths();

    // Register the receiver contract
    let receiver_contract = env.register(Receiver, ());

    // Register the caller contract
    let caller_contract = env.register(Caller, ());

    (caller_contract, receiver_contract)
}

#[test]
fn test_cross_contract_call_with_auth() {
    let env = Env::default();
    let (caller_contract, receiver_contract) = setup(&env);

    let caller = CallerClient::new(&env, &caller_contract);
    let authorized_user = Address::generate(&env);
    let message = symbol_short!("hello");

    // Invoke caller which will call receiver
    // Both authorization requirements are satisfied by the test environment
    let result = caller.invoke_receiver(&receiver_contract, &authorized_user, &message);

    // Verify the call succeeded and returned the message
    assert_eq!(result, message);
}

#[test]
fn test_cross_contract_call_multiple_invocations() {
    let env = Env::default();
    let (caller_contract, receiver_contract) = setup(&env);

    let caller = CallerClient::new(&env, &caller_contract);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Make multiple calls with different users and messages
    let msg1 = symbol_short!("first");
    let result1 = caller.invoke_receiver(&receiver_contract, &alice, &msg1);
    assert_eq!(result1, msg1);

    let msg2 = symbol_short!("second");
    let result2 = caller.invoke_receiver(&receiver_contract, &bob, &msg2);
    assert_eq!(result2, msg2);

    // Different message for same user
    let msg3 = symbol_short!("third");
    let result3 = caller.invoke_receiver(&receiver_contract, &alice, &msg3);
    assert_eq!(result3, msg3);
}

#[test]
fn test_end_to_end_cross_contract_auth() {
    // This test demonstrates the complete flow:
    // 1. A user authorizes a transaction
    // 2. The caller contract processes it
    // 3. The caller invokes the receiver contract
    // 4. The receiver checks authorization (which propagates from step 1)
    // 5. Both contracts succeed

    let env = Env::default();
    env.mock_all_auths();

    let receiver = env.register(Receiver, ());
    let caller = env.register(Caller, ());

    let caller_client = CallerClient::new(&env, &caller);
    let user = Address::generate(&env);

    // The key insight: even though the receiver contract calls require_auth(),
    // the authorization is satisfied by the original transaction's auth_entries,
    // which the test environment automatically handles.
    let message = symbol_short!("authed");
    let result = caller_client.invoke_receiver(&receiver, &user, &message);

    assert_eq!(result, message);
}
