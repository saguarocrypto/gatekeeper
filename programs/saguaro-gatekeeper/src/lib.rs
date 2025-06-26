use anchor_lang::prelude::*;

pub mod constants;
pub mod instructions;

pub const MAX_SLOTS_PER_TRANSACTION: usize = 100;
pub const SLOTS_PER_EPOCH: usize = 432_000;
pub const MAX_SLOTS_PER_EPOCH: usize = 10000;

// Bitmap size for 432,000 slots per epoch
pub const FULL_BITMAP_SIZE_BYTES: usize = 54000; // 432,000 bits / 8 = 54,000 bytes
pub const INITIAL_BITMAP_SIZE_BYTES: usize = 9000; // Initial size within 10KB limit

// Maximum storage capacity: 10MB
pub const MAX_STORAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const MAX_BITMAP_SIZE_BYTES: usize = MAX_STORAGE_SIZE - 16; // 10MB minus metadata

// Account size constants
pub const SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE: usize = 8 + 2 + 4 + 1; // discriminator + epoch + vec_len + bump
pub const LARGE_BITMAP_ACCOUNT_BASE_SIZE: usize = 8 + 2 + 1 + 5; // discriminator + epoch + bump + padding
pub const INITIAL_ACCOUNT_SIZE: usize = 10240; // Initial 10KB allocation
pub const TARGET_ACCOUNT_SIZE: usize = LARGE_BITMAP_ACCOUNT_BASE_SIZE + FULL_BITMAP_SIZE_BYTES; // 54KB for full epoch
pub const MAX_ACCOUNT_SIZE: usize = MAX_STORAGE_SIZE; // 10MB maximum
pub const MAX_REALLOC_SIZE: usize = 10240; // Solana's 10KB reallocation limit per operation


declare_id!("saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3");

#[program]
pub mod saguaro_gatekeeper {
    use super::*;

    /// Set slots assigned to sandwich facilitating validators for a specific epoch.
    /// This instruction creates or overwrites a PDA containing slot assignments.
    /// 
    /// # Security Notes:
    /// - Requires multisig authority as signer
    /// - Enforces maximum slot limits to prevent DoS attacks
    /// - Manages rent-exemption through proper lamport transfers
    pub fn set_sandwich_validators(
        ctx: Context<SetSandwichValidators>,
        epoch_arg: u16,
        slots_arg: Vec<u64>,
    ) -> Result<()> {
        instructions::set_sandwich_validators_handler(ctx, epoch_arg, slots_arg)
    }

    /// Update slots in an existing SandwichValidators PDA.
    /// Supports both adding new slots and removing existing slots.
    /// 
    /// # Security Notes:
    /// - Requires multisig authority as signer
    /// - Validates PDA existence and authority match
    /// - Validates slot limits and prevents duplicates
    pub fn update_sandwich_validator(
        ctx: Context<UpdateSandwichValidator>,
        epoch_arg: u16,
        new_slots: Vec<u64>,
        remove_slots: Vec<u64>,
    ) -> Result<()> {
        instructions::update_sandwich_validator_handler(ctx, epoch_arg, new_slots, remove_slots)
    }


    /// Validate whether the current slot is gated for sandwich facilitating validators.
    /// This is a public instruction safe for Cross-Program Invocation (CPI).
    /// 
    /// # CPI Safety:
    /// - Returns Ok if PDA doesn't exist (allows normal operation)
    /// - Returns SlotIsGated error only if slot is explicitly gated
    /// - Always performs validation for CPI compatibility
    /// - Derives current epoch and slot from Clock sysvar internally
    pub fn validate_sandwich_validators(
        ctx: Context<ValidateSandwichValidators>,
    ) -> Result<()> {
        instructions::validate_sandwich_validators_handler(ctx)
    }

    /// Close a SandwichValidators PDA for a past epoch and refund rent.
    /// 
    /// # Security Notes:
    /// - Only allows closing PDAs for past epochs
    /// - Requires multisig authority as signer
    pub fn close_sandwich_validator(
        ctx: Context<CloseSandwichValidator>,
        epoch_to_close: u16,
    ) -> Result<()> {
        instructions::close_sandwich_validator_handler(ctx, epoch_to_close)
    }

    /// Initialize a large bitmap account with 10KB initial allocation.
    /// This is the first step in the two-step allocation process.
    pub fn initialize_large_bitmap(
        ctx: Context<InitializeLargeBitmap>,
        epoch_arg: u16,
    ) -> Result<()> {
        instructions::initialize_large_bitmap_handler(ctx, epoch_arg)
    }

    /// Expand the bitmap account to full size without writing data.
    /// This is used to expand the account beyond the initial 10KB limit.
    pub fn expand_bitmap(
        ctx: Context<ExpandAndWriteBitmap>,
    ) -> Result<()> {
        instructions::expand_bitmap_handler(ctx)
    }

    /// Expand the bitmap account to full size and optionally write data.
    /// This uses realloc() to expand beyond the initial 10KB limit.
    pub fn expand_and_write_bitmap(
        ctx: Context<ExpandAndWriteBitmap>,
        data_chunk: Vec<u8>,
        chunk_offset: u64,
    ) -> Result<()> {
        instructions::expand_and_write_bitmap_handler(ctx, data_chunk, chunk_offset)
    }

    /// Append data to the large bitmap account.
    pub fn append_data(
        ctx: Context<AppendData>,
        data: Vec<u8>,
    ) -> Result<()> {
        instructions::append_data_handler(ctx, data)
    }

    /// Clear all data in the large bitmap account.
    pub fn clear_data(
        ctx: Context<ClearData>,
    ) -> Result<()> {
        instructions::clear_data_handler(ctx)
    }

}

/// Account storing the validator slot assignments for a specific epoch using a bitmap.
/// PDA derived from `SEED_PREFIX`, `multisig_authority` key, and `epoch`.
/// The multisig_authority is not stored as it can be derived from the PDA seeds.
#[account]
#[derive(Debug)]
pub struct SandwichValidators {
    /// The epoch number (u16) to which these slot assignments apply.
    pub epoch: u16,
    /// A bitmap where each bit represents whether a slot within the epoch is gated.
    /// Due to Solana's 10KB account size limit, this bitmap covers the first portion
    /// of slots in each epoch. Bit N represents slot (epoch * 432,000 + N).
    /// Defaults to all false (all slots ungated).
    pub slots: Vec<u8>,
    /// The canonical bump seed used for PDA derivation.
    pub bump: u8,
}

impl SandwichValidators {
    /// Checks if a specific slot is gated within this epoch
    pub fn is_slot_gated(&self, slot: u64) -> bool {
        let epoch_start = (self.epoch as u64) * SLOTS_PER_EPOCH as u64;
        let epoch_end = epoch_start + SLOTS_PER_EPOCH as u64;
        
        // Check if slot is within this epoch's range
        if slot < epoch_start || slot >= epoch_end {
            return false;
        }
        
        let slot_offset = (slot - epoch_start) as usize;
        let max_trackable_slots = INITIAL_BITMAP_SIZE_BYTES * 8;
        
        // If slot is beyond our bitmap capacity, we assume it's not gated
        if slot_offset >= max_trackable_slots {
            return false;
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        
        if byte_index >= self.slots.len() {
            return false;
        }
        
        (self.slots[byte_index] >> bit_index) & 1 == 1
    }
    
    /// Sets a slot as gated within this epoch
    pub fn set_slot_gated(&mut self, slot: u64, gated: bool) -> Result<()> {
        let epoch_start = (self.epoch as u64) * SLOTS_PER_EPOCH as u64;
        let epoch_end = epoch_start + SLOTS_PER_EPOCH as u64;
        
        // Check if slot is within this epoch's range
        if slot < epoch_start || slot >= epoch_end {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        let slot_offset = (slot - epoch_start) as usize;
        let max_trackable_slots = INITIAL_BITMAP_SIZE_BYTES * 8; // Maximum slots we can track
        
        // Check if slot is within our bitmap capacity
        if slot_offset >= max_trackable_slots {
            // For slots beyond our bitmap capacity, we cannot track them
            // This is a limitation due to Solana's 10KB account size constraint
            msg!("Warning: Slot {} is beyond trackable range (max offset: {})", slot, max_trackable_slots - 1);
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        // Ensure bitmap is properly initialized
        if self.slots.len() != INITIAL_BITMAP_SIZE_BYTES {
            self.slots.resize(INITIAL_BITMAP_SIZE_BYTES, 0);
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        
        if gated {
            self.slots[byte_index] |= 1 << bit_index;
        } else {
            self.slots[byte_index] &= !(1 << bit_index);
        }
        
        Ok(())
    }
}

impl SandwichValidators {
    pub const SEED_PREFIX: &'static [u8] = constants::SEED_PREFIX;
}

/// Large bitmap account that can store 432,000 slots using zero-copy patterns.
/// Data is accessed manually to avoid heap allocation issues.
#[account(zero_copy)]
#[repr(C)]
pub struct LargeBitmap {
    /// The epoch number (u16) to which these slot assignments apply.
    pub epoch: u16,
    /// The canonical bump seed used for PDA derivation.
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 5],
}

impl LargeBitmap {
    pub const SEED_PREFIX: &'static [u8] = b"large_bitmap";
    pub const DATA_OFFSET: usize = 16; // discriminator (8) + epoch (2) + bump (1) + padding (5)

    /// Gets a reference to the bitmap data within the account
    pub fn bitmap_data<'a>(&self, account_data: &'a [u8]) -> &'a [u8] {
        &account_data[Self::DATA_OFFSET..]
    }

    /// Gets a mutable reference to the bitmap data within the account
    pub fn bitmap_data_mut<'a>(&self, account_data: &'a mut [u8]) -> &'a mut [u8] {
        &mut account_data[Self::DATA_OFFSET..]
    }

    /// Checks if a specific slot is gated within this epoch
    pub fn is_slot_gated(&self, slot: u64, account_data: &[u8]) -> bool {
        let epoch_start = (self.epoch as u64) * SLOTS_PER_EPOCH as u64;
        let epoch_end = epoch_start + SLOTS_PER_EPOCH as u64;
        
        // Check if slot is within this epoch's range
        if slot < epoch_start || slot >= epoch_end {
            return false;
        }
        
        let slot_offset = (slot - epoch_start) as usize;
        
        // Check if slot is within the epoch range
        if slot_offset >= SLOTS_PER_EPOCH {
            return false;
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        
        let bitmap = self.bitmap_data(account_data);
        if byte_index >= bitmap.len() {
            return false;
        }
        
        (bitmap[byte_index] >> bit_index) & 1 == 1
    }
    
    /// Sets a slot as gated within this epoch
    pub fn set_slot_gated(&self, slot: u64, gated: bool, account_data: &mut [u8]) -> Result<()> {
        let epoch_start = (self.epoch as u64) * SLOTS_PER_EPOCH as u64;
        let epoch_end = epoch_start + SLOTS_PER_EPOCH as u64;
        
        // Check if slot is within this epoch's range
        if slot < epoch_start || slot >= epoch_end {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        let slot_offset = (slot - epoch_start) as usize;
        
        // Check if slot is within our bitmap capacity
        if slot_offset >= SLOTS_PER_EPOCH {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        
        let bitmap = self.bitmap_data_mut(account_data);
        if byte_index >= bitmap.len() {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        if gated {
            bitmap[byte_index] |= 1 << bit_index;
        } else {
            bitmap[byte_index] &= !(1 << bit_index);
        }
        
        Ok(())
    }

    /// Sets multiple slots as gated
    pub fn set_slots_gated(&self, slots: &[u64], gated: bool, account_data: &mut [u8]) -> Result<()> {
        for &slot in slots {
            self.set_slot_gated(slot, gated, account_data)?;
        }
        Ok(())
    }

    /// Clears all slots (sets all to ungated)
    pub fn clear_all_slots(&self, account_data: &mut [u8]) {
        let bitmap = self.bitmap_data_mut(account_data);
        bitmap.fill(0);
    }
}

/// Accounts for the `set_sandwich_validators` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16, slots_arg: Vec<u64>)]
pub struct SetSandwichValidators<'info> {
    /// CHECK: This account is manually validated and initialized in the instruction handler
    #[account(
        mut,
        seeds = [b"sandwich_validators", multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountInfo<'info>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `validate_sandwich_validators` instruction.
/// Derives current epoch/slot from Clock sysvar and validates PDA manually.
#[derive(Accounts)]
pub struct ValidateSandwichValidators<'info> {
    /// The multisig authority account used in PDA derivation.
    /// Not a signer as validation is public.
    /// CHECK: This is used for PDA derivation only
    pub multisig_authority: AccountInfo<'info>,
    /// The Clock sysvar to get current epoch and slot
    pub clock: Sysvar<'info, Clock>,
}

/// Accounts for the `update_sandwich_validator` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16, new_slots: Vec<u64>, remove_slots: Vec<u64>)]
pub struct UpdateSandwichValidator<'info> {
    /// CHECK: This account is manually validated in the instruction handler
    #[account(
        mut,
        seeds = [b"sandwich_validators", multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountInfo<'info>,

    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}


/// Accounts for the `close_sandwich_validator` instruction.
#[derive(Accounts)]
#[instruction(epoch_to_close: u16)]
pub struct CloseSandwichValidator<'info> {
    #[account(
        mut,
        close = multisig_authority,
        seeds = [b"sandwich_validators", multisig_authority.key().as_ref(), &epoch_to_close.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: Account<'info, SandwichValidators>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `initialize_large_bitmap` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16)]
pub struct InitializeLargeBitmap<'info> {
    /// CHECK: This account is manually validated and initialized in the instruction handler
    #[account(
        mut,
        seeds = [LargeBitmap::SEED_PREFIX, multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub large_bitmap: AccountInfo<'info>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `expand_and_write_bitmap` instruction.
#[derive(Accounts)]
pub struct ExpandAndWriteBitmap<'info> {
    /// CHECK: This account is manually validated in the instruction handler
    #[account(mut)]
    pub large_bitmap: AccountInfo<'info>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `append_data` instruction.
#[derive(Accounts)]
pub struct AppendData<'info> {
    #[account(mut)]
    pub large_bitmap: AccountLoader<'info, LargeBitmap>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
}

/// Accounts for the `clear_data` instruction.
#[derive(Accounts)]
pub struct ClearData<'info> {
    #[account(mut)]
    pub large_bitmap: AccountLoader<'info, LargeBitmap>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
}


/// Events emitted by the Saguaro Gatekeeper program for monitoring
#[event]
pub struct SandwichValidatorsSet {
    pub authority: Pubkey,
    pub epoch: u16,
    pub slot_count: u16,
}

#[event]
pub struct SandwichValidatorsUpdated {
    pub authority: Pubkey,
    pub epoch: u16,
    pub slots_added: u16,
    pub slots_removed: u16,
    pub total_slots: u16,
}

#[event]
pub struct SandwichValidatorsClosed {
    pub authority: Pubkey,
    pub epoch: u16,
}


/// Custom error codes for the Saguaro Gatekeeper program.
#[error_code]
pub enum GatekeeperError {
    #[msg("The current slot is gated and cannot be used.")]
    SlotIsGated,
    #[msg("Epoch in PDA does not match current validation epoch.")]
    EpochMismatch,
    #[msg("Authority in PDA does not match provided authority.")]
    AuthorityMismatch,
    #[msg("Invalid authority provided for the operation.")]
    InvalidAuthority,
    #[msg("Invalid PDA provided for the operation.")]
    InvalidPda,
    #[msg("An invalid epoch was provided for the operation.")]
    InvalidEpoch,
    #[msg("The epoch has not finished yet and cannot be closed.")]
    EpochNotFinished,
    #[msg("The provided list of slots exceeds the maximum allowed size.")]
    TooManySlots,
    #[msg("Account bump does not match expected value.")]
    BumpMismatch,
    #[msg("Account rent exempt balance not met after transfer.")]
    RentNotMet,
    #[msg("Slot numbers must be unique within an epoch.")]
    DuplicateSlots,
    #[msg("Empty slot list may not be intentional.")]
    EmptySlotList,
    #[msg("Slot number is outside the acceptable range.")]
    SlotOutOfRange,
    #[msg("Missing sandwich validators account in remaining_accounts.")]
    MissingSandwichValidatorsAccount,
    #[msg("Invalid sandwich validators PDA provided.")]
    InvalidSandwichValidatorsPDA,
}