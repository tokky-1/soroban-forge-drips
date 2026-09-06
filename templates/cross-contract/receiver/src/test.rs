use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

#[test]
fn test_authorized_action() {
    let env = Env::default();
    env.mock_all_auths();

    let contract = env.register(Receiver, ());
    let client = ReceiverClient::new(&env, &contract);

    let caller = Address::generate(&env);
    let message = symbol_short!("test");

    let result = client.authorized_action(&caller, &message);

    assert_eq!(result, message);
}
