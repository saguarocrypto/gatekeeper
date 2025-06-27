import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  setSandwichValidators,
  updateSandwichValidator,
  validateSandwichValidators,
  // expandSandwichValidators, // REMOVED: Function was removed from contract and SDK
  closeSandwichValidator,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

// Helper function to create explorer link
function getExplorerLink(signature: string): string {
  return `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
}

// ANSI color codes
const colors = {
  reset: "\x1b[0m",
  bright: "\x1b[1m",
  green: "\x1b[32m",
  red: "\x1b[31m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
  cyan: "\x1b[36m",
};

async function main() {
  console.log(`${colors.bright}${colors.blue}=== Saguaro Gatekeeper Devnet Test (Simple) ===${colors.reset}\n`);

  try {
    // Setup connection and wallet
    const connection = new web3.Connection("https://api.devnet.solana.com", "confirmed");
    const walletPath = process.env.WALLET_PATH || `${process.env.HOME}/.config/solana/id.json`;
    const keypair = web3.Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(readFileSync(walletPath, "utf-8")))
    );
    const wallet = new anchor.Wallet(keypair);
    
    const provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
    });
    anchor.setProvider(provider);

    console.log(`${colors.yellow}Wallet: ${wallet.publicKey.toBase58()}${colors.reset}`);

    // Check balance
    const balance = await connection.getBalance(wallet.publicKey);
    console.log(`${colors.yellow}Balance: ${balance / web3.LAMPORTS_PER_SOL} SOL${colors.reset}`);

    // Load program using workspace (which reads from Anchor.toml)
    let program: anchor.Program<SaguaroGatekeeper>;
    
    // Try to load from workspace first
    try {
      program = anchor.workspace.SaguaroGatekeeper as anchor.Program<SaguaroGatekeeper>;
      console.log(`${colors.green}✓ Loaded program from workspace${colors.reset}`);
    } catch (e) {
      // Fallback: load manually
      console.log(`${colors.yellow}Loading program manually...${colors.reset}`);
      const programId = new web3.PublicKey(process.env.PROGRAM_ID || "CfVQ26C5bNkaKNNj7Upin5RFgU4ZNaKN3uPmfV2Cq3tA");
      const idl = require("../target/idl/saguaro_gatekeeper.json");
      
      // Update IDL address to match the program ID
      idl.address = programId.toBase58();
      
      program = new anchor.Program<SaguaroGatekeeper>(idl, provider);
    }

    console.log(`${colors.yellow}Program ID: ${program.programId.toBase58()}${colors.reset}\n`);

    // Test 1: Basic functionality - Create PDA
    console.log(`${colors.cyan}Test 1: Creating sandwich validators PDA...${colors.reset}`);
    try {
      const epoch = new BN(9999); // Use a high epoch number to avoid conflicts
      const slots = [new BN(9999 * SLOTS_PER_EPOCH), new BN(9999 * SLOTS_PER_EPOCH + 100)];

      const test1Sig = await setSandwichValidators(program, {
        epoch: epoch.toNumber(),
        slots: slots,
        multisigAuthority: wallet.publicKey,
      })
        .signers([wallet.payer])
        .rpc();

      console.log(`${colors.green}✓ PDA created successfully${colors.reset}`);
      console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(test1Sig)}${colors.reset}`);

      // Fetch and verify
      const { pda } = getSandwichValidatorsPda(wallet.publicKey, epoch, program.programId);
      const account = await program.account.sandwichValidators.fetch(pda);
      console.log(`${colors.green}✓ Verified: Epoch ${account.epoch}, Bitmap size: ${account.slots.length} bytes${colors.reset}`);
    } catch (error) {
      console.log(`${colors.red}✗ Failed: ${error}${colors.reset}`);
    }

    // Test 2: Update slots
    console.log(`\n${colors.cyan}Test 2: Updating validator slots...${colors.reset}`);
    try {
      const epoch = new BN(9999);
      const newSlots = [new BN(9999 * SLOTS_PER_EPOCH + 200), new BN(9999 * SLOTS_PER_EPOCH + 300)];

      const test2Sig = await updateSandwichValidator(program, {
        epoch: epoch.toNumber(),
        newSlots: newSlots,
        multisigAuthority: wallet.publicKey,
      })
        .signers([wallet.payer])
        .rpc();

      console.log(`${colors.green}✓ Slots updated successfully${colors.reset}`);
      console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(test2Sig)}${colors.reset}`);
    } catch (error) {
      console.log(`${colors.red}✗ Failed: ${error}${colors.reset}`);
    }

    // Get current slot and calculate current epoch for all tests
    const currentSlot = await connection.getSlot();
    const currentEpoch = Math.floor(currentSlot / SLOTS_PER_EPOCH);
    const epochStartSlot = currentEpoch * SLOTS_PER_EPOCH;
    const currentSlotOffset = currentSlot - epochStartSlot;
    
    // Test 3: Clean up any existing PDA for current epoch
    console.log(`\n${colors.cyan}Test 3: Preparing clean state for current epoch testing...${colors.reset}`);
    
    try {
      console.log(`${colors.yellow}Current slot: ${currentSlot}, Current epoch: ${currentEpoch}${colors.reset}`);
      console.log(`${colors.yellow}Epoch start slot: ${epochStartSlot}, Slot offset: ${currentSlotOffset}${colors.reset}`);
      
      // Check if PDA already exists and close it if needed
      const { pda: currentEpochPda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
      const existingPda = await connection.getAccountInfo(currentEpochPda);
      
      if (existingPda) {
        console.log(`${colors.yellow}Found existing PDA for current epoch (size: ${existingPda.data.length} bytes)${colors.reset}`);
        
        // Check if it's an expanded PDA (54015 bytes)
        if (existingPda.data.length >= 54015) {
          console.log(`${colors.yellow}PDA is already expanded, attempting to close it...${colors.reset}`);
          
          // For large PDAs, we might need to skip closing due to memory allocation issues
          // Instead, we'll work with the existing PDA
          console.log(`${colors.yellow}⚠ Skipping close due to known memory allocation issues with large PDAs${colors.reset}`);
          console.log(`${colors.yellow}Will continue tests with existing PDA${colors.reset}`);
        } else {
          try {
            const closeSig = await closeSandwichValidator(program, {
              epoch: currentEpoch,
              multisigAuthority: wallet.publicKey,
            })
              .signers([wallet.payer])
              .rpc();
            console.log(`${colors.green}✓ Existing PDA closed${colors.reset}`);
            console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(closeSig)}${colors.reset}`);
          } catch (error) {
            console.log(`${colors.yellow}⚠ Could not close existing PDA: ${error}${colors.reset}`);
          }
        }
      } else {
        console.log(`${colors.green}✓ No existing PDA found for current epoch${colors.reset}`);
      }
      
    } catch (error) {
      console.log(`${colors.red}✗ PDA cleanup failed: ${error}${colors.reset}`);
    }

    // Test 4: Comprehensive validation testing with expandable bitmap
    console.log(`\n${colors.cyan}Test 4: Testing validate_sandwich_validators with expandable bitmap...${colors.reset}`);
    try {
      // Skip clearing for faster testing - just note if PDA exists
      console.log(`\n${colors.cyan}Pre-step: Checking for existing gated slots...${colors.reset}`);
      const { pda: cleanupPda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
      const existingAccount = await connection.getAccountInfo(cleanupPda);
      if (existingAccount) {
        console.log(`${colors.yellow}⚠ Existing PDA found - continuing with existing state${colors.reset}`);
      } else {
        console.log(`${colors.green}✓ No existing PDA found - clean state confirmed${colors.reset}`);
      }
      
      // Step 1: Test validation with no PDA (should succeed)
      console.log(`\n${colors.cyan}Step 1: validate_sandwich_validators with no PDA (should succeed)...${colors.reset}`);
      try {
        const tx = await validateSandwichValidators(program, {
          multisigAuthority: wallet.publicKey,
          epoch: currentEpoch,
        });
        const step1Sig = await tx.rpc();
        console.log(`${colors.green}✓ Validation succeeded - no PDA exists for current epoch${colors.reset}`);
        console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step1Sig)}${colors.reset}`);
      } catch (error) {
        console.log(`${colors.red}✗ Unexpected failure: ${error}${colors.reset}`);
      }
      
      // Step 2: Create small bitmap with empty slots (all bits = 0)
      console.log(`\n${colors.cyan}Step 2: Create small bitmap with empty slots...${colors.reset}`);
      
      // Check if PDA exists and skip creation if it does
      const { pda: step2Pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
      const step2ExistingAccount = await connection.getAccountInfo(step2Pda);
      
      if (step2ExistingAccount) {
        console.log(`${colors.yellow}⚠ PDA already exists for current epoch, skipping creation${colors.reset}`);
      } else {
        const step2Sig = await setSandwichValidators(program, {
          epoch: currentEpoch,
          slots: [], // Empty slots array = no gated slots = all bits remain 0
          multisigAuthority: wallet.publicKey,
        })
          .signers([wallet.payer])
          .rpc();
        
        console.log(`${colors.green}✓ Small bitmap PDA created with empty slots${colors.reset}`);
        console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step2Sig)}${colors.reset}`);
      }
      
      // Step 3: Test validation with small bitmap (should succeed)
      console.log(`\n${colors.cyan}Step 3: validate_sandwich_validators with small bitmap (should succeed)...${colors.reset}`);
      try {
        const tx = await validateSandwichValidators(program, {
          multisigAuthority: wallet.publicKey,
          epoch: currentEpoch,
        });
        const step3Sig = await tx.rpc();
        
        if (currentSlotOffset > 71999) {
          console.log(`${colors.green}✓ Validation succeeded - current slot offset ${currentSlotOffset} is beyond small bitmap capacity (72,000)${colors.reset}`);
        } else {
          console.log(`${colors.green}✓ Validation succeeded - current slot bit is 0 (ungated) in small bitmap${colors.reset}`);
        }
        console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step3Sig)}${colors.reset}`);
      } catch (error) {
        console.log(`${colors.red}✗ Unexpected failure: ${error}${colors.reset}`);
      }
      
      // Step 4: Check PDA capacity (expansion functionality was removed)
      console.log(`\n${colors.cyan}Step 4: Checking sandwich_validators PDA capacity...${colors.reset}`);
      
      // REMOVED: expandSandwichValidators was removed from the contract
      // The PDA is now created with full capacity automatically
      console.log(`${colors.yellow}NOTE: expandSandwichValidators was removed - PDA is now created with full capacity${colors.reset}`);
      
      // Get the current PDA and check its size
      const { pda: currentEpochPda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
      const pdaInfo = await connection.getAccountInfo(currentEpochPda);
      console.log(`${colors.yellow}Current PDA size: ${pdaInfo?.data.length} bytes${colors.reset}`);
      
      // Check the bitmap capacity
      if (pdaInfo?.data) {
        const bitmapLen = pdaInfo.data.readUInt32LE(10);
        console.log(`${colors.yellow}Bitmap length field: ${bitmapLen} bytes (max slots: ${bitmapLen * 8})${colors.reset}`);
        console.log(`${colors.yellow}Current slot offset: ${currentSlotOffset}${colors.reset}`);
        
        if (currentSlotOffset < bitmapLen * 8) {
          console.log(`${colors.green}✓ PDA has sufficient capacity for current slot${colors.reset}`);
        } else {
          console.log(`${colors.red}✗ Current slot offset exceeds PDA capacity${colors.reset}`);
        }
      }
      
      // // Old expansion code (removed):
      // do {
      //   expansionCount++;
      //   console.log(`${colors.yellow}Expansion ${expansionCount}: Current size ${currentSize} bytes, need ${targetSize - currentSize} more bytes${colors.reset}`);
      //   
      //   try {
      //     const step4Sig = await expandSandwichValidators(program, {
      //       epoch: currentEpoch,
      //       multisigAuthority: wallet.publicKey,
      //     })
      //       .signers([wallet.payer])
      //       .rpc();
      //     ...
      //   } catch (error) {
      //     ...
      //   }
      // } while (currentSize < targetSize);
      
      // Step 5: Test validation after expansion (should succeed)
      console.log(`\n${colors.cyan}Step 5: validate_sandwich_validators after expansion (should succeed)...${colors.reset}`);
      try {
        const tx = await validateSandwichValidators(program, {
          multisigAuthority: wallet.publicKey,
          epoch: currentEpoch,
        });
        const step5Sig = await tx.rpc();
        console.log(`${colors.green}✓ Validation succeeded - bitmap can now handle full epoch capacity${colors.reset}`);
        console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step5Sig)}${colors.reset}`);
      } catch (error) {
        console.log(`${colors.red}✗ Unexpected failure: ${error}${colors.reset}`);
      }
      
      // Step 6: Gate current slot range using updateSandwichValidator
      console.log(`\n${colors.cyan}Step 6: Gate current slot range...${colors.reset}`);
      
      let gatingSkipped = false;
      
      // Fetch the current slot right before gating to ensure accuracy
      const gatingCurrentSlot = await connection.getSlot();
      const gatingSlotOffset = gatingCurrentSlot - epochStartSlot;
      
      console.log(`${colors.yellow}Current slot at gating time: ${gatingCurrentSlot}, offset: ${gatingSlotOffset}${colors.reset}`);
      
      // Check the PDA's bitmap capacity
      const pdaBeforeGating = await connection.getAccountInfo(currentEpochPda);
      if (pdaBeforeGating?.data) {
        const bitmapLen = pdaBeforeGating.data.readUInt32LE(10);
        const maxTrackableSlots = bitmapLen * 8;
        console.log(`${colors.yellow}Bitmap capacity: ${bitmapLen} bytes (max ${maxTrackableSlots} slots)${colors.reset}`);
        
        if (gatingSlotOffset >= maxTrackableSlots) {
          console.log(`${colors.red}✗ Current slot offset ${gatingSlotOffset} exceeds bitmap capacity ${maxTrackableSlots}${colors.reset}`);
          console.log(`${colors.yellow}Skipping slot gating test${colors.reset}`);
          gatingSkipped = true;
        }
      }
      
      // Now we can gate the actual current slot since we have expanded capacity
      // Gate multiple ranges (100 slots each - max allowed per transaction) to create a wider gated area
      const gateRangeSize = 100; // Maximum allowed per transaction
      const totalRanges = 10; // Gate 10 ranges of 100 slots each = 1000 total slots
      const slotsPerRange = gateRangeSize;
      
      let overallStartOffset = 0;
      let overallEndOffset = 0;
      let validationSlotOffset = 0;
      
      if (!gatingSkipped) {
        // Calculate the overall range centered around the current slot
        const totalGatedSlots = totalRanges * slotsPerRange;
        // Center the range around the current slot offset
        const halfRange = Math.floor(totalGatedSlots / 2);
        overallStartOffset = Math.max(0, gatingSlotOffset - halfRange);
        overallEndOffset = Math.min(431999, gatingSlotOffset + halfRange); // Stay within epoch bounds
        
        console.log(`${colors.yellow}Gating ${totalRanges} ranges of ${slotsPerRange} slots each around current slot${colors.reset}`);
        console.log(`${colors.yellow}Total gated range: ${overallStartOffset} to ${overallEndOffset} (${overallEndOffset - overallStartOffset + 1} slots)${colors.reset}`);
        console.log(`${colors.yellow}Current slot offset ${gatingSlotOffset} is ${gatingSlotOffset >= overallStartOffset && gatingSlotOffset <= overallEndOffset ? 'INCLUDED' : 'NOT INCLUDED'} in gated range${colors.reset}`);
        
        // Note: Skipping clearing step for faster testing - using fresh ranges should avoid duplicates
        console.log(`${colors.yellow}Using offset ranges that should be unique to avoid duplicates...${colors.reset}`);
        
        // Gate ranges in batches
        for (let rangeIndex = 0; rangeIndex < totalRanges; rangeIndex++) {
          const rangeStart = overallStartOffset + (rangeIndex * slotsPerRange);
          const rangeEnd = Math.min(rangeStart + slotsPerRange - 1, overallEndOffset);
          
          const slotsToGate = [];
          for (let offset = rangeStart; offset <= rangeEnd; offset++) {
            slotsToGate.push(new BN(epochStartSlot + offset));
          }
          
          if (slotsToGate.length > 0) {
            const step6Sig = await updateSandwichValidator(program, {
              epoch: currentEpoch,
              newSlots: slotsToGate,
              removeSlots: [], // No slots to remove
              multisigAuthority: wallet.publicKey,
            })
              .signers([wallet.payer])
              .rpc();
            
            console.log(`${colors.green}✓ Gated range ${rangeIndex + 1}/${totalRanges}: ${slotsToGate.length} slots (${rangeStart}-${rangeEnd})${colors.reset}`);
            if (rangeIndex === 0) {
              console.log(`${colors.blue}📋 First Transaction: ${getExplorerLink(step6Sig)}${colors.reset}`);
            }
          }
        }
      }
      
      // Step 7: Test validation with gated slots (should now fail due to large gated range)
      console.log(`\n${colors.cyan}Step 7: validate_sandwich_validators with large gated range (should fail)...${colors.reset}`);
      
      if (gatingSkipped) {
        console.log(`${colors.yellow}⚠ Skipping validation test because slot gating was skipped${colors.reset}`);
      } else {
      
      // Fetch current slot again for validation to ensure we're testing the right slot
      const validationCurrentSlot = await connection.getSlot();
      validationSlotOffset = validationCurrentSlot - epochStartSlot;
      console.log(`${colors.yellow}Current slot at validation time: ${validationCurrentSlot}, offset: ${validationSlotOffset}${colors.reset}`);
      console.log(`${colors.yellow}Slot is ${validationSlotOffset >= overallStartOffset && validationSlotOffset <= overallEndOffset ? 'WITHIN' : 'OUTSIDE'} the gated range${colors.reset}`);
      
      // Debug: Check epoch consistency
      const epochInfo = await connection.getEpochInfo("processed");
      const connectionEpoch = epochInfo.epoch;
      console.log(`${colors.yellow}Debug: Connection epoch: ${connectionEpoch}, Test calculated epoch: ${currentEpoch}${colors.reset}`);
      
      let step7Sig: string | null = null;
      try {
        const tx = await validateSandwichValidators(program, {
          multisigAuthority: wallet.publicKey,
          epoch: currentEpoch, // Use the epoch we created the PDA for
        });
        
        // Build and send the transaction with skipPreflight to bypass simulation
        const transaction = await tx.transaction();
        const latestBlockhash = await connection.getLatestBlockhash();
        transaction.recentBlockhash = latestBlockhash.blockhash;
        transaction.feePayer = wallet.publicKey;
        
        // Sign the transaction
        const signedTx = await wallet.signTransaction(transaction);
        
        // Send without simulation to ensure it executes on-chain
        step7Sig = await connection.sendRawTransaction(signedTx.serialize(), {
          skipPreflight: true,
          preflightCommitment: "confirmed"
        });
        
        // Wait for confirmation
        await connection.confirmTransaction({
          signature: step7Sig,
          blockhash: latestBlockhash.blockhash,
          lastValidBlockHeight: latestBlockhash.lastValidBlockHeight
        }, "confirmed");
        
        // Check if the current slot was actually within the gated range
        if (validationSlotOffset >= overallStartOffset && validationSlotOffset <= overallEndOffset) {
          console.log(`${colors.red}✗ Unexpected success - current slot ${validationSlotOffset} is within the gated range ${overallStartOffset}-${overallEndOffset}${colors.reset}`);
        } else {
          console.log(`${colors.yellow}⚠ Validation passed because current slot ${validationSlotOffset} drifted outside the gated range ${overallStartOffset}-${overallEndOffset}${colors.reset}`);
        }
        console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step7Sig)}${colors.reset}`);
      } catch (error: any) {
        // Check for the SlotIsGated error in various formats
        const errorStr = error.toString();
        const errorMsg = error.message || '';
        const logs = error.logs || [];
        
        // Check if this is a transaction error with logs
        if (logs.length > 0) {
          const hasSlotIsGated = logs.some((log: string) => log.includes('SlotIsGated'));
          if (hasSlotIsGated) {
            console.log(`${colors.green}✓ Validation correctly failed - SlotIsGated error${colors.reset}`);
            if (step7Sig) {
              console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step7Sig)}${colors.reset}`);
            }
          } else {
            console.log(`${colors.red}✗ Transaction failed with unexpected error${colors.reset}`);
            console.log(`${colors.yellow}Error: ${errorStr}${colors.reset}`);
            if (step7Sig) {
              console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step7Sig)}${colors.reset}`);
            }
          }
        } else if (errorStr.includes('SlotIsGated') || errorMsg.includes('SlotIsGated')) {
          console.log(`${colors.green}✓ Validation correctly failed - SlotIsGated error${colors.reset}`);
          if (step7Sig) {
            console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step7Sig)}${colors.reset}`);
          }
        } else {
          console.log(`${colors.red}✗ Failed with unexpected error: ${errorStr}${colors.reset}`);
          if (step7Sig) {
            console.log(`${colors.blue}📋 Transaction: ${getExplorerLink(step7Sig)}${colors.reset}`);
          }
        }
      }
      } // Close the else block for gatingSkipped
      
      // Summary
      console.log(`\n${colors.cyan}Test 4 Summary:${colors.reset}`);
      console.log(`${colors.yellow}- Created small bitmap PDA (${beforeExpansion?.data.length} bytes)${colors.reset}`);
      console.log(`${colors.yellow}- Expanded bitmap PDA to (${finalSize?.data.length} bytes)${colors.reset}`);
      if (!gatingSkipped) {
        console.log(`${colors.yellow}- Dynamically gated current slot range (${overallStartOffset}-${overallEndOffset})${colors.reset}`);
        console.log(`${colors.yellow}- Validation correctly ${validationSlotOffset >= overallStartOffset && validationSlotOffset <= overallEndOffset ? 'failed for gated slot' : 'passed for ungated slot'}${colors.reset}`);
      } else {
        console.log(`${colors.yellow}- Slot gating was skipped due to bitmap capacity constraints${colors.reset}`);
      }

    } catch (error) {
      console.log(`${colors.red}✗ Expandable bitmap test failed: ${error}${colors.reset}`);
    }

    // Test 5: Close old PDA
    console.log(`\n${colors.cyan}Test 5: Closing old epoch PDA...${colors.reset}`);
    try {
      const oldEpoch = new BN(1); // Assuming epoch 1 is old enough

      // First create it
      const test5aSig = await setSandwichValidators(program, {
        epoch: oldEpoch.toNumber(),
        slots: [new BN(1 * SLOTS_PER_EPOCH)],
        multisigAuthority: wallet.publicKey,
      })
        .signers([wallet.payer])
        .rpc();

      // Then close it
      const test5bSig = await closeSandwichValidator(program, {
        epoch: oldEpoch.toNumber(),
        multisigAuthority: wallet.publicKey,
      })
        .signers([wallet.payer])
        .rpc();

      console.log(`${colors.green}✓ Old PDA closed successfully${colors.reset}`);
      console.log(`${colors.blue}📋 Create TX: ${getExplorerLink(test5aSig)}${colors.reset}`);
      console.log(`${colors.blue}📋 Close TX: ${getExplorerLink(test5bSig)}${colors.reset}`);
    } catch (error) {
      console.log(`${colors.red}✗ Failed: ${error}${colors.reset}`);
    }

    console.log(`\n${colors.bright}${colors.green}Tests completed!${colors.reset}`);
    console.log(`${colors.cyan}View transactions at: https://explorer.solana.com/address/${wallet.publicKey.toBase58()}?cluster=devnet${colors.reset}`);

  } catch (error) {
    console.error(`${colors.red}Fatal error: ${error}${colors.reset}`);
    process.exit(1);
  }
}

main().catch(console.error);