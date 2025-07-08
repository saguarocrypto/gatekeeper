// Transaction limits
pub const MAX_SLOTS_PER_TRANSACTION: usize = 100;

// Epoch configuration 
pub const SLOTS_PER_EPOCH: usize = 432_000;

// Bitmap size constants
pub const FULL_BITMAP_SIZE_BYTES: usize = 54000; // 432,000 bits / 8 = 54,000 bytes
pub const INITIAL_BITMAP_SIZE_BYTES: usize = 9000; // Initial size within 10KB limit

// Account size constants
pub const LARGE_BITMAP_ACCOUNT_BASE_SIZE: usize = 8 + 2 + 1 + 5; // discriminator + epoch + bump + padding
pub const INITIAL_ACCOUNT_SIZE: usize = 10240; // Initial 10KB allocation
pub const TARGET_ACCOUNT_SIZE: usize = LARGE_BITMAP_ACCOUNT_BASE_SIZE + FULL_BITMAP_SIZE_BYTES; // 54KB for full epoch
pub const MAX_REALLOC_SIZE: usize = 10240; // Solana's 10KB reallocation limit per operation
