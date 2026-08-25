use super::*;
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    vec, Address, Bytes, BytesN, Env, Vec,
};

const LIMITS_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../benchmarks/budget-baselines.env"
);
const BASELINES: &str = include_str!("../../../benchmarks/budget-baselines.env");

fn setup() -> (Env, ReceiptAnchorClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().disable_resource_limits();
    let contract_id = env.register(ReceiptAnchor, ());
    let client = ReceiptAnchorClient::new(&env, &contract_id);
    let merchant = Address::generate(&env);
    client.initialize(&merchant);
    (env, client)
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

fn hash_pair(env: &Env, a: &BytesN<32>, b: &BytesN<32>) -> BytesN<32> {
    let (lo, hi) = if a.to_array() <= b.to_array() {
        (a.to_array(), b.to_array())
    } else {
        (b.to_array(), a.to_array())
    };
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(&lo);
    combined[32..].copy_from_slice(&hi);
    let digest = env
        .crypto()
        .sha256(&Bytes::from_slice(env, &combined))
        .to_array();
    BytesN::from_array(env, &digest)
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "RECEIPT_ANCHOR_BATCH_1_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "RECEIPT_ANCHOR_BATCH_1_MEM_LIMIT")]
fn budget_anchor_batch_count_1() {
    let (env, client) = setup();
    let root = BytesN::from_array(&env, &[1u8; 32]);
    env.cost_estimate().budget().reset_unlimited();
    client.anchor_batch(&root, &1, &0, &100);
    assert_resources(&env, "RECEIPT_ANCHOR_BATCH_1");
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "RECEIPT_ANCHOR_BATCH_500_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "RECEIPT_ANCHOR_BATCH_500_MEM_LIMIT")]
fn budget_anchor_batch_count_midpoint() {
    let (env, client) = setup();
    let root = BytesN::from_array(&env, &[2u8; 32]);
    let midpoint = MAX_BATCH_SIZE / 2;
    env.cost_estimate().budget().reset_unlimited();
    client.anchor_batch(&root, &midpoint, &0, &100);
    assert_resources(&env, "RECEIPT_ANCHOR_BATCH_500");
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "RECEIPT_ANCHOR_BATCH_1000_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "RECEIPT_ANCHOR_BATCH_1000_MEM_LIMIT")]
fn budget_anchor_batch_count_max() {
    let (env, client) = setup();
    let root = BytesN::from_array(&env, &[3u8; 32]);
    env.cost_estimate().budget().reset_unlimited();
    client.anchor_batch(&root, &MAX_BATCH_SIZE, &0, &100);
    assert_resources(&env, "RECEIPT_ANCHOR_BATCH_1000");
}

fn run_verify_budget(depth: usize, prefix: &str) -> Env {
    let (env, client) = setup();
    let leaf = BytesN::from_array(&env, &[7u8; 32]);
    let mut root = leaf.clone();
    let mut proof: Vec<BytesN<32>> = vec![&env];
    for i in 0..depth {
        let sibling = BytesN::from_array(&env, &[(i as u8).wrapping_add(20); 32]);
        root = hash_pair(&env, &root, &sibling);
        proof.push_back(sibling);
    }
    let batch_id = client.anchor_batch(&root, &1, &0, &100);
    env.cost_estimate().budget().reset_unlimited();
    assert!(client.verify_receipt(&batch_id, &leaf, &proof));
    assert_resources(&env, prefix);
    env
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "RECEIPT_VERIFY_DEPTH_0_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "RECEIPT_VERIFY_DEPTH_0_MEM_LIMIT")]
fn budget_verify_receipt_depth_0() {
    let env = run_verify_budget(0, "RECEIPT_VERIFY_DEPTH_0");
    let _ = &env;
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "RECEIPT_VERIFY_DEPTH_5_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "RECEIPT_VERIFY_DEPTH_5_MEM_LIMIT")]
fn budget_verify_receipt_depth_5() {
    let env = run_verify_budget(5, "RECEIPT_VERIFY_DEPTH_5");
    let _ = &env;
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "RECEIPT_VERIFY_DEPTH_10_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "RECEIPT_VERIFY_DEPTH_10_MEM_LIMIT")]
fn budget_verify_receipt_depth_10() {
    let env = run_verify_budget(10, "RECEIPT_VERIFY_DEPTH_10");
    let _ = &env;
}

#[test]
#[budget_cpu_lt(env_file = LIMITS_FILE, env = "RECEIPT_PRUNE_64_CPU_LIMIT")]
#[budget_mem_lt(env_file = LIMITS_FILE, env = "RECEIPT_PRUNE_64_MEM_LIMIT")]
fn budget_prune_batches_64() {
    let (env, client) = setup();
    let root = BytesN::from_array(&env, &[9u8; 32]);
    for i in 0..64u32 {
        env.ledger().with_mut(|li| li.sequence_number = i + 1);
        client.anchor_batch(&root, &1, &(i as u64), &(i as u64 + 1));
    }
    env.ledger().with_mut(|li| li.sequence_number = 1000);
    env.cost_estimate().budget().reset_unlimited();
    client.prune_batches(&1000);
    assert_resources(&env, "RECEIPT_PRUNE_64");
}
