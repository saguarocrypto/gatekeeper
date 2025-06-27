import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";

/**
 * The PDA seed prefix for the SandwichValidators account.
 * This must match the value in the Rust program.
 */
export const SANDWICH_VALIDATORS_SEED_PREFIX = "sandwich_validators";

/**
 * The PDA seed prefix for the LargeBitmap account.
 * This must match the value in the Rust program.
 */
export const LARGE_BITMAP_SEED_PREFIX = "large_bitmap";

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
 * Constants for large bitmap operations.
 */
export const FULL_BITMAP_SIZE_BYTES = 54_000; // 432,000 bits / 8 = 54,000 bytes
export const INITIAL_ACCOUNT_SIZE = 10240; // Initial 10KB allocation
export const MAX_REALLOC_SIZE = 10240; // Maximum bytes per realloc operation
export const LARGE_BITMAP_ACCOUNT_BASE_SIZE = 16; // discriminator(8) + epoch(2) + bump(1) + padding(5)
export const TARGET_ACCOUNT_SIZE = LARGE_BITMAP_ACCOUNT_BASE_SIZE + FULL_BITMAP_SIZE_BYTES; // 54,016 bytes total

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

/**
 * Derives the Program Derived Address (PDA) for the LargeBitmap account.
 *
 * @param multisigAuthority The public key of the multisig authority.
 * @param epoch The epoch number (as a BN).
 * @param programId The program ID.
 * @returns An object containing the PDA public key (`pda`) and the bump seed (`bump`).
 */
export const getLargeBitmapPda = (
  multisigAuthority: PublicKey,
  epoch: anchor.BN,
  programId: PublicKey
): { pda: PublicKey; bump: number } => {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [
      Buffer.from(LARGE_BITMAP_SEED_PREFIX),
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
 */
export const setSandwichValidators = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    slots: anchor.BN[];
    multisigAuthority: PublicKey;
  }
) => {
  // Client-side validation to prevent encoding errors
  if (args.slots.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot set more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${args.slots.length}.`);
  }

  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .setSandwichValidators(args.epoch, args.slots)
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};

/**
 * Creates a MethodsBuilder to call the `updateSandwichValidator` instruction.
 * This instruction supports both adding new slots and removing existing slots.
 */
export const updateSandwichValidator = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    newSlots?: anchor.BN[];
    removeSlots?: anchor.BN[];
    multisigAuthority: PublicKey;
  }
) => {
  const newSlots = args.newSlots || [];
  const removeSlots = args.removeSlots || [];

  // Client-side validation to prevent encoding errors
  if (newSlots.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot add more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${newSlots.length}.`);
  }

  if (removeSlots.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot remove more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${removeSlots.length}.`);
  }

  if (newSlots.length === 0 && removeSlots.length === 0) {
    throw new Error(`No operation specified: Must provide either newSlots or removeSlots (or both).`);
  }

  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .updateSandwichValidator(args.epoch, newSlots, removeSlots)
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
 * Creates a MethodsBuilder to call the `expandSandwichValidators` instruction.
 * This expands an existing SandwichValidators PDA to full bitmap capacity.
 */
export const expandSandwichValidators = (
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
    .expandSandwichValidators(args.epoch)
    .accountsStrict({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
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

// --- Large Bitmap Instruction Wrapper Functions ---

/**
 * Creates a MethodsBuilder to call the `initializeLargeBitmap` instruction.
 */
export const initializeLargeBitmap = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getLargeBitmapPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .initializeLargeBitmap(args.epoch)
    .accountsStrict({
      largeBitmap: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};

/**
 * Creates a MethodsBuilder to call the `expandBitmap` instruction.
 */
export const expandBitmap = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getLargeBitmapPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .expandBitmap()
    .accountsStrict({
      largeBitmap: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};

/**
 * Creates a MethodsBuilder to call the `expandAndWriteBitmap` instruction.
 */
export const expandAndWriteBitmap = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
    dataChunk: Buffer;
    chunkOffset: number;
  }
) => {
  const { pda } = getLargeBitmapPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  // Keep as Buffer for Borsh serialization  
  const dataChunkArray = args.dataChunk;
  
  return program.methods
    .expandAndWriteBitmap(dataChunkArray, new anchor.BN(args.chunkOffset))
    .accountsStrict({
      largeBitmap: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    });
};

/**
 * Creates a MethodsBuilder to call the `appendData` instruction.
 */
export const appendData = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
    data: Buffer;
  }
) => {
  const { pda } = getLargeBitmapPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  // Keep as Buffer for Borsh serialization
  const dataArray = args.data;
  
  return program.methods
    .appendData(dataArray)
    .accountsStrict({
      largeBitmap: pda,
      multisigAuthority: args.multisigAuthority,
    });
};

/**
 * Creates a MethodsBuilder to call the `clearData` instruction.
 */
export const clearData = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getLargeBitmapPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .clearData()
    .accountsStrict({
      largeBitmap: pda,
      multisigAuthority: args.multisigAuthority,
    });
};

/**
 * Prepares a complete transaction to initialize and populate a large bitmap.
 * This function handles the multi-step process: initialization and chained expansion.
 */
export const prepareLargeBitmapTransaction = async (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
    bitmapData: Buffer;
  }
) => {
  const { pda } = getLargeBitmapPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  // Step 1: Initialize the account with 10KB
  const initializeIx = await initializeLargeBitmap(program, {
    epoch: args.epoch,
    multisigAuthority: args.multisigAuthority,
  }).instruction();

  // Step 2: Calculate number of expansion instructions needed
  const totalExpansionNeeded = TARGET_ACCOUNT_SIZE - INITIAL_ACCOUNT_SIZE;
  const numExpansions = Math.ceil(totalExpansionNeeded / MAX_REALLOC_SIZE);
  
  const expandInstructions = [];
  for (let i = 0; i < numExpansions; i++) {
    const expandIx = await expandBitmap(program, {
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
      const writeIx = await expandAndWriteBitmap(program, {
        epoch: args.epoch,
        multisigAuthority: args.multisigAuthority,
        dataChunk: chunk,
        chunkOffset: offset,
      }).instruction();
      writeInstructions.push(writeIx);
    }
  }

  return {
    initializeInstruction: initializeIx,
    expandInstructions,
    writeInstructions,
    largeBitmapPda: pda,
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
  // Calculate bitmap size needed (full epoch = 54,000 bytes)
  const bitmapSize = FULL_BITMAP_SIZE_BYTES;
  const bitmap = Buffer.alloc(bitmapSize, 0);

  const epochStart = epoch * SLOTS_PER_EPOCH;
  
  for (const slot of slots) {
    // Calculate slot offset within the epoch
    const slotOffset = slot - epochStart;
    
    // Validate slot is within this epoch
    if (slotOffset < 0 || slotOffset >= SLOTS_PER_EPOCH) {
      throw new Error(`Slot ${slot} is not within epoch ${epoch} (slots ${epochStart}-${epochStart + SLOTS_PER_EPOCH - 1})`);
    }
    
    // Set the bit for this slot
    const byteIndex = Math.floor(slotOffset / 8);
    const bitIndex = slotOffset % 8;
    bitmap[byteIndex] |= (1 << bitIndex);
  }
  
  return bitmap;
};

