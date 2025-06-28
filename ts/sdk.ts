import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";


/**
 * The PDA seed prefix for the LargeBitmap account.
 * This must match the value in the Rust program.
 */
export const SANDWICH_VALIDATORS_SEED_PREFIX = "sandwich_validators";

/**
 * Maximum number of slots allowed per transaction.
 * This must match the value in the Rust program.
 */
export const MAX_SLOTS_PER_TRANSACTION = 100;

/**
 * Number of slots per epoch (432,000).
 * This must match the value in the Rust program.
 */
export const SLOTS_PER_EPOCH = 432_000;

/**
 * Constants for sandwich validators bitmap operations.
 */
export const FULL_BITMAP_SIZE_BYTES = 54_000; // 432,000 bits / 8 = 54,000 bytes
export const INITIAL_ACCOUNT_SIZE = 10240; // Initial 10KB allocation
export const MAX_REALLOC_SIZE = 10240; // Maximum bytes per realloc operation
export const SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE = 16; // discriminator(8) + epoch(2) + bump(1) + padding(5)
export const TARGET_ACCOUNT_SIZE = SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE + FULL_BITMAP_SIZE_BYTES; // 54,016 bytes total


/**
 * Derives the Program Derived Address (PDA) for the SandwichValidators account.
 *
 * @param multisigAuthority The public key of the multisig authority.
 * @param epoch The epoch number (as a BN).
 * @param programId The program ID.
 * @returns An object containing the PDA public key (`pda`) and the bump seed (`bump`).
 */
export const getSandwichValidatorsPda = (
  multisigAuthority: PublicKey,
  epoch: anchor.BN,
  programId: PublicKey
): { pda: PublicKey; bump: number } => {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [
      Buffer.from(SANDWICH_VALIDATORS_SEED_PREFIX),
      multisigAuthority.toBuffer(),
      epoch.toBuffer("le", 2), // Epoch is u16, so 2 bytes little-endian
    ],
    programId
  );
  return { pda, bump };
};

// --- Instruction Wrapper Functions ---

/**
 * Creates a MethodsBuilder to call the `setSandwichValidators` instruction.
 * 
 * **CRUD Operation: CREATE**
 * This instruction only creates the account with initial 10KB size - no slots are set.
 * Use `expandSandwichValidatorsBitmap` to expand to full size.
 * Use `modifySandwichValidators` to gate/ungate slots.
 */
export const setSandwichValidators = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .setSandwichValidators(args.epoch)
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};

/**
 * Creates a MethodsBuilder to call the `modifySandwichValidators` instruction.
 * 
 * **CRUD Operation: UPDATE**
 * This instruction supports both gating slots (set to true) and ungating slots (set to false).
 */
export const modifySandwichValidators = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    slotsToGate?: anchor.BN[];
    slotsToUngate?: anchor.BN[];
    multisigAuthority: PublicKey;
  }
) => {
  const slotsToGate = args.slotsToGate || [];
  const slotsToUngate = args.slotsToUngate || [];

  // Client-side validation to prevent encoding errors
  if (slotsToGate.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot gate more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${slotsToGate.length}.`);
  }

  if (slotsToUngate.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot ungate more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${slotsToUngate.length}.`);
  }

  if (slotsToGate.length === 0 && slotsToUngate.length === 0) {
    throw new Error(`No operation specified: Must provide either slotsToGate or slotsToUngate (or both).`);
  }

  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .modifySandwichValidators(args.epoch, slotsToGate, slotsToUngate)
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};


/**
 * Creates a MethodsBuilder to call the `validateSandwichValidators` instruction.
 */
export const validateSandwichValidators = async (
  program: Program<SaguaroGatekeeper>,
  args: {
    multisigAuthority: PublicKey;
    epoch?: number; // Optional: specify epoch manually for testing
  }
) => {
  let targetEpoch: number;

  if (args.epoch !== undefined) {
    // Use manually specified epoch (for testing)
    targetEpoch = args.epoch;
  } else {
    // Get epoch info from the connection
    const epochInfo = await program.provider.connection.getEpochInfo("processed");

    // In local test validators, the epoch is typically 0 regardless of slot
    // The Clock sysvar will also report epoch 0
    targetEpoch = epochInfo.epoch;
  }

  // Derive the PDA for the target epoch
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(targetEpoch),
    program.programId
  );

  return program.methods
    .validateSandwichValidators()
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
    });
};


/**
 * Creates a MethodsBuilder to call the `closeSandwichValidator` instruction.
 */
export const closeSandwichValidator = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .closeSandwichValidator(args.epoch)
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};

/**
 * Creates a MethodsBuilder to call the `expandSandwichValidatorsBitmap` instruction.
 */
export const expandSandwichValidatorsBitmap = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .expandSandwichValidatorsBitmap()
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};

// --- Utility Instruction Wrapper Functions ---


/**
 * Creates a MethodsBuilder to call the `appendDataSandwichValidatorsBitmap` instruction.
 * 
 * **Utility Operation**: Raw bitmap data writing
 * This is a low-level utility for writing pre-computed bitmap data.
 * Most users should use `modifySandwichValidators` instead.
 */
export const appendDataSandwichValidatorsBitmap = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
    data: Buffer;
  }
) => {
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  // Keep as Buffer for Borsh serialization
  const dataArray = args.data;
  
  return program.methods
    .appendDataSandwichValidatorsBitmap(dataArray)
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
    });
};

/**
 * Creates a MethodsBuilder to call the `clearDataSandwichValidatorsBitmap` instruction.
 * 
 * **Utility Operation**: Clear all bitmap data (ungate all slots)
 * This sets all slots in the bitmap to ungated (false).
 */
export const clearDataSandwichValidatorsBitmap = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .clearDataSandwichValidatorsBitmap()
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
    });
};

/**
 * Prepares instructions to create and populate a sandwich validators account.
 * Uses the streamlined approach: set_sandwich_validators + expand_bitmap + append_data.
 */
export const prepareLargeBitmapTransaction = async (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
    bitmapData: Buffer;
  }
) => {
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  // Step 1: Create the account using set_sandwich_validators (10KB initial size)
  const createIx = await setSandwichValidators(program, {
    epoch: args.epoch,
    multisigAuthority: args.multisigAuthority,
  }).instruction();

  // Step 2: Calculate number of expansion instructions needed
  const totalExpansionNeeded = TARGET_ACCOUNT_SIZE - INITIAL_ACCOUNT_SIZE;
  const numExpansions = Math.ceil(totalExpansionNeeded / MAX_REALLOC_SIZE);
  
  const expandInstructions = [];
  for (let i = 0; i < numExpansions; i++) {
    const expandIx = await expandSandwichValidatorsBitmap(program, {
      epoch: args.epoch,
      multisigAuthority: args.multisigAuthority,
    }).instruction();
    expandInstructions.push(expandIx);
  }

  // Step 3: Write data in chunks (if data provided)
  const writeInstructions = [];
  if (args.bitmapData.length > 0) {
    const chunkSize = 900; // Conservative chunk size for data writing
    
    for (let offset = 0; offset < args.bitmapData.length; offset += chunkSize) {
      const chunk = args.bitmapData.slice(offset, offset + chunkSize);
      const writeIx = await appendDataSandwichValidatorsBitmap(program, {
        epoch: args.epoch,
        multisigAuthority: args.multisigAuthority,
        data: chunk,
      }).instruction();
      writeInstructions.push(writeIx);
    }
  }

  return {
    createInstruction: createIx,
    expandInstructions,
    writeInstructions,
    sandwichValidatorsPda: pda,
    totalExpansions: numExpansions,
    targetSize: TARGET_ACCOUNT_SIZE,
  };
};

/**
 * Helper function to create a bitmap buffer for a given set of slots within an epoch.
 * @param slots Array of slot numbers to mark as gated
 * @param epoch The epoch number
 * @returns Buffer containing the bitmap data
 */
export const createBitmapForSlots = (slots: number[], epoch: number): Buffer => {
  // Input validation with overflow protection
  if (!Number.isInteger(epoch) || epoch < 0 || epoch > 65535) {
    throw new Error(`Invalid epoch: ${epoch}. Must be a u16 (0-65535)`);
  }
  
  if (!Array.isArray(slots)) {
    throw new Error("Slots must be an array");
  }
  
  if (slots.length > MAX_SLOTS_PER_TRANSACTION * 100) { // Conservative limit for utility functions
    throw new Error(`Too many slots: ${slots.length}. Consider processing in smaller batches.`);
  }
  
  // Calculate bitmap size needed (full epoch = 54,000 bytes)
  const bitmapSize = FULL_BITMAP_SIZE_BYTES;
  const bitmap = Buffer.alloc(bitmapSize, 0);

  const epochStart = epoch * SLOTS_PER_EPOCH;
  const epochEnd = epochStart + SLOTS_PER_EPOCH - 1;
  
  // Check for duplicates and validate ranges in a single pass
  const seenSlots = new Set<number>();
  
  for (const slot of slots) {
    // Input validation
    if (!Number.isInteger(slot) || slot < 0) {
      throw new Error(`Invalid slot: ${slot}. Must be a non-negative integer`);
    }
    
    // Check for duplicates
    if (seenSlots.has(slot)) {
      throw new Error(`Duplicate slot: ${slot}`);
    }
    seenSlots.add(slot);
    
    // Validate slot is within this epoch with clear error message
    if (slot < epochStart || slot > epochEnd) {
      throw new Error(`Slot ${slot} is not within epoch ${epoch} (valid range: ${epochStart}-${epochEnd})`);
    }
    
    // Calculate slot offset within the epoch with overflow protection
    const slotOffset = slot - epochStart;
    
    // Set the bit for this slot with bounds checking
    const byteIndex = Math.floor(slotOffset / 8);
    const bitIndex = slotOffset % 8;
    
    if (byteIndex >= bitmapSize) {
      throw new Error(`Bitmap overflow: slot ${slot} requires byte index ${byteIndex}, but bitmap is only ${bitmapSize} bytes`);
    }
    
    bitmap[byteIndex] |= (1 << bitIndex);
  }
  
  return bitmap;
};

/**
 * Example function demonstrating safe CPI usage from another Solana program.
 * This shows how third-party programs should call validateSandwichValidators.
 * 
 * CPI Integration Notes:
 * - No Signer Required: multisigAuthority is not a signer for validation
 * - Fail-Open Design: If PDA doesn't exist, validation passes
 * - Atomic Protection: SlotIsGated error fails entire transaction
 * - Minimal Compute: Optimized for low compute usage
 * 
 * Example usage:
 * ```typescript
 * // In your program's instruction handler:
 * import { validateSandwichValidators } from "path/to/sdk";
 * 
 * export const myProgramInstruction = async (
 *   program: Program<MyProgram>,
 *   args: { multisigAuthority: PublicKey }
 * ) => {
 *   // Step 1: Validate sandwich protection before executing your logic
 *   try {
 *     await validateSandwichValidators(program, {
 *       multisigAuthority: args.multisigAuthority,
 *     }).rpc();
 *   } catch (error) {
 *     if (error.message.includes("SlotIsGated")) {
 *       throw new Error("Current slot is protected from sandwich attacks");
 *     }
 *     // Other errors indicate validation succeeded (fail-open design)
 *   }
 *   
 *   // Step 2: Execute your program logic safely
 *   return program.methods.myInstruction().accounts({});
 * };
 * ```
 */
export const cpiUsageExample = "See above documentation block for CPI integration example";

