use super::*;
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    Address, BytesN, Env,
};

const FLOAT: i128 = 1_000_000;
const LIMITS_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../benchmarks/budget-baselines.env"
);
const BASELINES: &str = include_str!("../../../benchmarks/budget-baselines.env");

fn setup(window: u32) -> (Env, RefundVaultClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().disable_resource_limits();
    let merchant = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token = sac.address();
    StellarAssetClient::new(&env, &token).mint(&merchant, &FLOAT);
    let contract_id = env.register(RefundVault, ());
    let client = RefundVaultClient::new(&env, &contract_id);
    client.initialize(&merchant, &token, &window);
    (env, client, merchant)
}

fn limit(key: &str) -> u64 {
    for line in BASELINES.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim() == key {
                return value
                    .trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("invalid budget value for {key}"));
            }
        }
    }
    panic!("missing budget key {key}");
}

fn assert_resources(env: &Env, prefix: &str) {
    let resources = env.cost_estimate().resources();
    let read_entries = u64::from(resources.memory_read_entries + resources.disk_read_entries);
    let read_bytes = u64::from(resources.disk_read_bytes);
    let write_entries = u64::from(resources.write_entries);
    let write_bytes = u64::from(resources.write_bytes);

    let read_entries_limit = limit(&std::format!("{prefix}_READ_ENTRIES_LIMIT"));
    let read_bytes_limit = limit(&std::format!("{prefix}_READ_BYTES_LIMIT"));
    let write_entries_limit = limit(&std::format!("{prefix}_WRITE_ENTRIES_LIMIT"));
    let write_bytes_limit = limit(&std::format!("{prefix}_WRITE_BYTES_LIMIT"));

    assert!(
        read_entries <= read_entries_limit,
        "{prefix}: read entries {read_entries} exceeded {read_entries_limit}"
    );
    assert!(
        read_bytes <= read_bytes_limit,
        "{prefix}: disk-read bytes {read_bytes} exceeded {read_bytes_limit}"
    );
    assert!(
        write_entries <= write_entries_limit,
        "{prefix}: write entries {write_entries} exceeded {write_entries_limit}"
    );
    assert!(
        write_bytes <= write_bytes_limit,
        "{prefix}: write bytes {write_bytes} exceeded {write_bytes_limit}"
    );

    let budget = env.cost_estimate().budget();
    std::println!(
        "BUDGET|{prefix}|cpu={}|mem={}|read_entries={read_entries}|read_bytes={read_bytes}|write_entries={write_entries}|write_bytes={write_bytes}",
        budget.cpu_instruction_cost(),
        budget.memory_bytes_cost(),
    );
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "REFUND_DEPOSIT_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "REFUND_DEPOSIT_MEM_LIMIT")]
fn budget_deposit() {
    let (env, client, merchant) = setup(100);
    env.cost_estimate().budget().reset_unlimited();
    client.deposit(&merchant, &500_000);
    assert_resources(&env, "REFUND_DEPOSIT");
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "REFUND_REFUND_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "REFUND_REFUND_MEM_LIMIT")]
fn budget_refund() {
    let (env, client, merchant) = setup(100);
    client.deposit(&merchant, &500_000);
    let payment_ref = BytesN::from_array(&env, &[7u8; 32]);
    let buyer = Address::generate(&env);
    env.cost_estimate().budget().reset_unlimited();
    client.refund(&payment_ref, &buyer, &120_000, &0);
    assert_resources(&env, "REFUND_REFUND");
}
