# Saguaro Gatekeeper Instructions

This document provides an overview of all instructions available in the Saguaro Gatekeeper program, organized by their primary purpose.

## Overview

The Saguaro Gatekeeper program provides on-chain slot gating functionality for Solana validators. It uses a bitmap-based approach to efficiently track which slots in an epoch are gated for sandwich facilitating validators.

## Core Instructions (CRUD Operations)

These instructions form the primary interface for managing sandwich validator permissions using clear CRUD semantics.

### `set_sandwich_validators` (CREATE)

**Purpose**: Creates a new SandwichValidators account for a specific epoch.

**Operation**: CREATE operation in CRUD pattern

**Key Features**:
- Creates account with initial 10KB size (Solana System Program limitation)
- Initializes bitmap with all slots ungated (zeros)
- Only creates the account - does not set any slots
- Must be followed by `expand_sandwich_validators_bitmap` for full capacity

**Parameters**:
- `epoch_arg: u16` - The epoch number for this validator set

**Security Notes**:
- Requires multisig authority as signer
- Prevents duplicate account creation
- Manages rent-exemption through proper lamport transfers

**Usage Pattern**:
```rust
// Step 1: Create account (10KB)
set_sandwich_validators(epoch: 123)

// Step 2: Expand to full size (54KB) 
expand_sandwich_validators_bitmap()

// Step 3: Set slots
modify_sandwich_validators(slots_to_gate: [slot1, slot2, ...])
```

---

### `validate_sandwich_validators` (READ)

**Purpose**: Validates whether the current slot is gated for sandwich facilitating validators.

**Operation**: READ operation in CRUD pattern

**Key Features**:
- Public instruction safe for Cross-Program Invocation (CPI)
- Automatically derives current epoch and slot from Clock sysvar
- Fail-open design: returns Ok if PDA doesn't exist
- Highly optimized for minimal compute usage

**Parameters**: None (derives epoch/slot from Clock sysvar)

**Return Behavior**:
- `Ok(())` - Slot is not gated or PDA doesn't exist
- `SlotIsGated` error - Current slot is explicitly gated

**Security Notes**:
- Validates PDA derivation to prevent bypass attacks
- No signer required (public validation)
- Uses Clock sysvar for reliable epoch/slot information

---

### `modify_sandwich_validators` (UPDATE)

**Purpose**: Modifies slots in an existing SandwichValidators account.

**Operation**: UPDATE operation in CRUD pattern

**Key Features**:
- Supports both gating slots (set to true) and ungating slots (set to false)
- Can gate and ungate slots in the same transaction
- Validates slot limits and prevents duplicates
- Handles account resizing if needed

**Parameters**:
- `epoch_arg: u16` - The epoch number
- `slots_to_gate: Vec<u64>` - Slots to mark as gated
- `slots_to_ungate: Vec<u64>` - Slots to mark as ungated

**Validation**:
- Maximum 100 slots per transaction
- Prevents duplicate slots within arrays
- Validates slot ranges for the epoch
- Requires at least one operation (gate or ungate)

**Security Notes**:
- Requires multisig authority as signer
- Validates PDA existence and authority match
- Prevents malformed operations

---

### `close_sandwich_validator` (DELETE)

**Purpose**: Closes a SandwichValidators PDA for a past epoch and refunds rent.

**Operation**: DELETE operation in CRUD pattern

**Key Features**:
- Only allows closing PDAs for past epochs
- Refunds rent to the authority
- Prevents premature closure of current/future epochs

**Parameters**:
- `epoch_to_close: u16` - The epoch number to close

**Security Notes**:
- Requires multisig authority as signer
- Validates epoch is in the past
- Secure rent refund mechanism

## Helper Instructions

These instructions provide low-level utilities and account management functionality.

### `expand_sandwich_validators_bitmap`

**Purpose**: Expands a SandwichValidators account to full size without writing data.

**Key Features**:
- Expands account beyond initial 10KB limit
- Required for full epoch capacity (432,000 slots)
- Multiple calls needed: 10KB → 54KB requires ~5 expansion calls
- Does not modify bitmap data

**Use Case**: Must be called after `set_sandwich_validators` to achieve full capacity.

**Security Notes**:
- Requires multisig authority as signer
- Validates account ownership

---

### `append_data_sandwich_validators_bitmap`

**Purpose**: Low-level utility for writing pre-computed bitmap data directly.

**Key Features**:
- Appends raw bitmap data to the account
- Useful for bulk operations or migrations
- Bypasses individual slot validation
- Most users should use `modify_sandwich_validators` instead

**Parameters**:
- `data: Vec<u8>` - Raw bitmap data to append

**Warning**: This is a low-level operation. Prefer `modify_sandwich_validators` for normal use.

**Security Notes**:
- Requires multisig authority as signer
- No validation of data content

---

### `clear_data_sandwich_validators_bitmap`

**Purpose**: Clears all data in the sandwich validators bitmap account.

**Key Features**:
- Sets all slots in the bitmap to ungated (false)
- Efficient way to reset an entire epoch
- Preserves account structure

**Use Case**: Useful for resetting an epoch's gating configuration.

**Security Notes**:
- Requires multisig authority as signer
- Irreversible operation

## Architecture Notes

### Account Structure

```rust
pub struct SandwichValidators {
    pub epoch: u16,           // Epoch number (2 bytes)
    pub bump: u8,             // PDA bump seed (1 byte)
    pub padding: [u8; 5],     // Alignment padding (5 bytes)
    // Followed by bitmap data (54,000 bytes for full capacity)
}
```

### Storage Capacity

- **Total account size**: 54,016 bytes
- **Bitmap data size**: 54,000 bytes  
- **Slot capacity**: 432,000 slots per epoch
- **Initial size**: 10,240 bytes (System Program limit)

### PDA Derivation

PDAs are derived using:
```
seeds = [
    b"sandwich_validators",
    multisig_authority.key(),
    epoch.to_le_bytes()
]
```

### Bitmap Format

Each bit represents one slot:
- `0` = Slot is not gated (normal operation)
- `1` = Slot is gated (sandwich facilitating validators blocked)

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 6000 | `SlotIsGated` | Current slot is gated and cannot be used |
| 6001 | `EpochMismatch` | Epoch in PDA does not match validation epoch |
| 6002 | `AuthorityMismatch` | Authority in PDA does not match provided authority |
| 6003 | `InvalidAuthority` | Invalid authority provided |
| 6004 | `InvalidPda` | Invalid PDA provided |
| 6005 | `InvalidEpoch` | Invalid epoch provided |
| 6006 | `EpochNotFinished` | Epoch not finished, cannot be closed |
| 6007 | `TooManySlots` | Too many slots in operation |
| 6008 | `BumpMismatch` | Account bump mismatch |
| 6009 | `RentNotMet` | Rent exempt balance not met |
| 6010 | `DuplicateSlots` | Duplicate slots in operation |
| 6011 | `EmptySlotList` | Empty slot list provided |
| 6012 | `SlotOutOfRange` | Slot number outside acceptable range |

## Usage Examples

### Basic Workflow

```typescript
// 1. CREATE: Create account for epoch 123
await setSandwichValidators(program, {
  epoch: 123,
  multisigAuthority: authority.publicKey
});

// 2. Expand to full size
await expandSandwichValidatorsBitmap(program, {
  epoch: 123,
  multisigAuthority: authority.publicKey
});

// 3. UPDATE: Gate specific slots
await modifySandwichValidators(program, {
  epoch: 123,
  slotsToGate: [slot1, slot2, slot3],
  multisigAuthority: authority.publicKey
});

// 4. READ: Validate current slot (in epoch 123)
await validateSandwichValidators(program, {
  multisigAuthority: authority.publicKey
});

// 5. DELETE: Close when epoch is finished
await closeSandwichValidator(program, {
  epoch: 123,
  multisigAuthority: authority.publicKey
});
```

### Slot Manipulation

```typescript
// Gate additional slots
await modifySandwichValidators(program, {
  epoch: 123,
  slotsToGate: [newSlot1, newSlot2],
  multisigAuthority: authority.publicKey
});

// Ungate previously gated slots
await modifySandwichValidators(program, {
  epoch: 123,
  slotsToUngate: [oldSlot1, oldSlot2],
  multisigAuthority: authority.publicKey
});

// Gate and ungate in same transaction
await modifySandwichValidators(program, {
  epoch: 123,
  slotsToGate: [newSlot],
  slotsToUngate: [oldSlot],
  multisigAuthority: authority.publicKey
});
```