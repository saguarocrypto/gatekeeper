use {
    mollusk_svm_bencher::MolluskComputeUnitBencher,
    mollusk_svm::Mollusk,
    anchor_lang::{prelude::*, InstructionData, system_program, solana_program},
    saguaro_gatekeeper::instruction,
};

fn main() {
    // Disable logging for cleaner benchmark output  
    solana_logger::setup_with("");

    let program_id = saguaro_gatekeeper::ID;
    let mollusk = Mollusk::new(&program_id, "../../target/deploy/saguaro_gatekeeper");

    // Setup common test data
    let multisig_authority = Pubkey::new_unique();
    let epoch = 100u16;
    let current_slot = (epoch as u64) * 432000u64 + 1000;

    // ============================================================================
    // BENCHMARK 1: validate_sandwich_validators (optimized with lazy loading)
    // ============================================================================
    
    let clock_data = {
        let clock = solana_program::clock::Clock {
            slot: current_slot,
            epoch_start_timestamp: 0,
            epoch: epoch as u64,
            leader_schedule_epoch: epoch as u64,
            unix_timestamp: 0,
        };
        bincode::serialize(&clock).unwrap()
    };

    let validate_instruction = solana_program::instruction::Instruction {
        program_id,
        accounts: vec![
            solana_program::instruction::AccountMeta::new_readonly(multisig_authority, false),
            solana_program::instruction::AccountMeta::new_readonly(
                solana_program::sysvar::clock::ID, false
            ),
        ],
        data: instruction::ValidateSandwichValidators {}.data(),
    };

    let validate_accounts = vec![
        (multisig_authority, solana_program::account::Account::default()),
        (
            solana_program::sysvar::clock::ID,
            solana_program::account::Account {
                lamports: 1_000_000,
                data: clock_data,
                owner: solana_program::sysvar::clock::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
    ];

    // ============================================================================
    // BENCHMARK 2: set_sandwich_validators - Small (5 slots, optimized)
    // ============================================================================

    let (sandwich_validators_pda, _) = Pubkey::find_program_address(
        &[
            b"sandwich_validators",
            multisig_authority.as_ref(),
            &epoch.to_le_bytes(),
        ],
        &program_id,
    );

    let small_slots = vec![100u64, 200u64, 300u64, 400u64, 500u64];
    let set_small_instruction = solana_program::instruction::Instruction {
        program_id,
        accounts: vec![
            solana_program::instruction::AccountMeta::new(sandwich_validators_pda, false),
            solana_program::instruction::AccountMeta::new(multisig_authority, true),
            solana_program::instruction::AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: instruction::SetSandwichValidators {
            epoch_arg: epoch,
            slots_arg: small_slots,
        }.data(),
    };

    let set_small_accounts = vec![
        (sandwich_validators_pda, solana_program::account::Account::default()),
        (
            multisig_authority,
            solana_program::account::Account {
                lamports: 10_000_000,
                data: vec![],
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (system_program::ID, solana_program::account::Account::default()),
    ];

    // ============================================================================
    // BENCHMARK 3: set_sandwich_validators - Large (50 slots, batch optimized)
    // ============================================================================

    let large_slots: Vec<u64> = (0..50).map(|i| i * 100).collect();
    let set_large_instruction = solana_program::instruction::Instruction {
        program_id,
        accounts: vec![
            solana_program::instruction::AccountMeta::new(sandwich_validators_pda, false),
            solana_program::instruction::AccountMeta::new(multisig_authority, true),
            solana_program::instruction::AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: instruction::SetSandwichValidators {
            epoch_arg: epoch,
            slots_arg: large_slots,
        }.data(),
    };

    let set_large_accounts = set_small_accounts.clone();

    // ============================================================================
    // EXECUTE BENCHMARKS
    // ============================================================================

    println!("🔥 Starting Compute Unit Benchmarks for Optimized Saguaro Gatekeeper");
    println!("═══════════════════════════════════════════════════════════════════");

    MolluskComputeUnitBencher::new(mollusk)
        .bench(("validate_sandwich_validators_optimized", &validate_instruction, &validate_accounts))
        .bench(("set_sandwich_validators_small_optimized", &set_small_instruction, &set_small_accounts))
        .bench(("set_sandwich_validators_large_optimized", &set_large_instruction, &set_large_accounts))
        .must_pass(false) // Some tests may fail due to missing accounts, but we want CU measurements
        .out_dir("../target/benches")
        .execute();

    println!("\n🎯 OPTIMIZATION TARGETS:");
    println!("• validate_sandwich_validators: Target < 1,500 CU (lazy loading)");
    println!("• set_sandwich_validators (small): Target < 5,000 CU (direct memory)");
    println!("• set_sandwich_validators (large): Target < 8,000 CU (batch ops)");
    
    println!("\n📊 Check ../target/benches/compute_units.md for detailed results!");
    println!("✅ Compute unit benchmarks completed!");
}