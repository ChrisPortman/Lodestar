use lodestar_core::test_support::InMemoryStore;

lodestar_core::port_contract_tests!(#[tokio::test], () => InMemoryStore::new());
