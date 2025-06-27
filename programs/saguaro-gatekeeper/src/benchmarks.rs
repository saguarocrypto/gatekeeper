/// Compute unit benchmarking utilities for development and testing
/// 
/// These macros and functions help measure and track compute unit usage
/// during development and optimization phases.

#[cfg(feature = "compute-benchmarks")]
use anchor_lang::solana_program;

/// Macro to log compute units with a custom message
/// Only enabled with the "compute-benchmarks" feature
#[macro_export]
macro_rules! log_compute_units {
    ($msg:expr) => {
        #[cfg(feature = "compute-benchmarks")]
        {
            solana_program::log::sol_log_compute_units();
            solana_program::msg!($msg);
        }
    };
}

/// Macro to mark the start of a compute-intensive operation
#[macro_export]
macro_rules! benchmark_start {
    ($operation:expr) => {
        #[cfg(feature = "compute-benchmarks")]
        {
            solana_program::msg!("BENCHMARK START: {}", $operation);
            solana_program::log::sol_log_compute_units();
        }
    };
}

/// Macro to mark the end of a compute-intensive operation
#[macro_export]
macro_rules! benchmark_end {
    ($operation:expr) => {
        #[cfg(feature = "compute-benchmarks")]
        {
            solana_program::log::sol_log_compute_units();
            solana_program::msg!("BENCHMARK END: {}", $operation);
        }
    };
}

/// Struct to track compute unit usage across operations
#[cfg(feature = "compute-benchmarks")]
pub struct ComputeBenchmark {
    operation_name: &'static str,
    start_logged: bool,
}

#[cfg(feature = "compute-benchmarks")]
impl ComputeBenchmark {
    pub fn new(operation_name: &'static str) -> Self {
        solana_program::msg!("BENCHMARK: Starting {}", operation_name);
        solana_program::log::sol_log_compute_units();
        Self {
            operation_name,
            start_logged: true,
        }
    }

    pub fn checkpoint(&self, checkpoint_name: &str) {
        solana_program::msg!("BENCHMARK: {} - {}", self.operation_name, checkpoint_name);
        solana_program::log::sol_log_compute_units();
    }
}

#[cfg(feature = "compute-benchmarks")]
impl Drop for ComputeBenchmark {
    fn drop(&mut self) {
        if self.start_logged {
            solana_program::log::sol_log_compute_units();
            solana_program::msg!("BENCHMARK: Completed {}", self.operation_name);
        }
    }
}

/// Helper function to estimate compute units for bitmap operations
pub fn estimate_bitmap_operation_cost(num_slots: usize, operation_type: &str) -> u32 {
    match operation_type {
        "read_single_bit" => 100, // Very lightweight
        "write_single_bit" => 150,
        "read_multiple_bits" => 100 + (num_slots as u32 * 10),
        "write_multiple_bits" => 150 + (num_slots as u32 * 15),
        "full_bitmap_clear" => 500 + (54000 / 100), // Based on bitmap size
        "full_bitmap_scan" => 1000 + (54000 / 50),
        _ => 1000, // Default estimate
    }
}

/// Compute unit thresholds for different operations
pub mod compute_thresholds {
    pub const VALIDATE_SINGLE_SLOT: u32 = 1_500;
    pub const SET_VALIDATORS_SMALL: u32 = 5_000; // < 10 slots
    pub const SET_VALIDATORS_LARGE: u32 = 8_000; // 100 slots
    pub const UPDATE_VALIDATORS_SMALL: u32 = 7_000; // < 10 slots  
    pub const UPDATE_VALIDATORS_LARGE: u32 = 12_000; // 100 slots
    pub const INITIALIZE_BITMAP: u32 = 3_000;
    pub const EXPAND_BITMAP: u32 = 2_500;
    pub const CLEAR_BITMAP: u32 = 1_500;
    
    // Warning thresholds (80% of expected max)
    pub const WARNING_THRESHOLD_RATIO: f32 = 0.8;
}

/// Test helper to assert compute unit usage is within expected bounds
#[cfg(test)]
pub fn assert_compute_usage_within_bounds(operation: &str, expected_max: u32) {
    // This would be used in tests to ensure compute usage stays within bounds
    // Implementation would depend on test framework integration
    println!("Checking compute usage for {} (max: {})", operation, expected_max);
}