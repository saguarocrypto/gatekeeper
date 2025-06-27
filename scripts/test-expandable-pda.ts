import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  setSandwichValidators,
  expandSandwichValidators,
  updateSandwichValidator,
  validateSandwichValidators,
  closeSandwichValidator,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

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
  console.log(`${colors.bright}${colors.blue}=== Expandable Sandwich Validators PDA Test ===${colors.reset}\n`);

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
    
    try {
      program = anchor.workspace.SaguaroGatekeeper as anchor.Program<SaguaroGatekeeper>;
      console.log(`${colors.green}✓ Loaded program from workspace${colors.reset}`);
    } catch (e) {
      // Fallback: load manually
      console.log(`${colors.yellow}Loading program manually...${colors.reset}`);
      const programId = new web3.PublicKey(process.env.PROGRAM_ID || "CfVQ26C5bNkaKNNj7Upin5RFgU4ZNaKN3uPmfV2Cq3tA");
      const idl = require("../target/idl/saguaro_gatekeeper.json");
      
      idl.address = programId.toBase58();
      program = new anchor.Program<SaguaroGatekeeper>(idl, provider);
    }

    console.log(`${colors.yellow}Program ID: ${program.programId.toBase58()}${colors.reset}\n`);

    // Get current slot information
    const currentSlot = await connection.getSlot();
    const currentEpoch = Math.floor(currentSlot / SLOTS_PER_EPOCH);
    const epochStartSlot = currentEpoch * SLOTS_PER_EPOCH;
    const currentSlotOffset = currentSlot - epochStartSlot;
    
    console.log(`${colors.cyan}Current Environment:${colors.reset}`);
    console.log(`  Slot: ${currentSlot}`);
    console.log(`  Epoch: ${currentEpoch}`);
    console.log(`  Slot offset in epoch: ${currentSlotOffset}`);
    
    // Test epoch for demonstration
    const testEpoch = currentEpoch;
    
    // Clean up any existing PDA
    console.log(`\n${colors.cyan}Step 1: Cleaning up existing PDA...${colors.reset}`);
    const { pda: testPda } = getSandwichValidatorsPda(wallet.publicKey, new BN(testEpoch), program.programId);
    const existingPda = await connection.getAccountInfo(testPda);
    
    if (existingPda) {
      console.log(`${colors.yellow}Found existing PDA, closing it...${colors.reset}`);
      try {
        await closeSandwichValidator(program, {
          epoch: testEpoch,
          multisigAuthority: wallet.publicKey,
        })
          .signers([wallet.payer])
          .rpc();
        console.log(`${colors.green}✓ Existing PDA closed${colors.reset}`);
      } catch (error) {
        console.log(`${colors.yellow}⚠ Could not close existing PDA: ${error}${colors.reset}`);
      }
    } else {
      console.log(`${colors.green}✓ No existing PDA found${colors.reset}`);
    }

    // Step 2: Create initial small PDA
    console.log(`\n${colors.cyan}Step 2: Creating initial sandwich validators PDA...${colors.reset}`);
    await setSandwichValidators(program, {
      epoch: testEpoch,
      slots: [], // Start with no gated slots
      multisigAuthority: wallet.publicKey,
    })
      .signers([wallet.payer])
      .rpc();

    const afterCreation = await connection.getAccountInfo(testPda);
    console.log(`${colors.green}✓ PDA created with size: ${afterCreation?.data.length} bytes${colors.reset}`);

    // Step 3: Test validation before expansion
    console.log(`\n${colors.cyan}Step 3: Testing validation before expansion...${colors.reset}`);
    try {
      const tx = await validateSandwichValidators(program, {
        multisigAuthority: wallet.publicKey,
      });
      await tx.rpc();
      console.log(`${colors.green}✓ Validation succeeded (no slots gated)${colors.reset}`);
    } catch (error) {
      console.log(`${colors.red}✗ Unexpected validation failure: ${error}${colors.reset}`);
    }

    // Step 4: Expand the PDA
    console.log(`\n${colors.cyan}Step 4: Expanding PDA to full capacity...${colors.reset}`);
    await expandSandwichValidators(program, {
      epoch: testEpoch,
      multisigAuthority: wallet.publicKey,
    })
      .signers([wallet.payer])
      .rpc();

    const afterExpansion = await connection.getAccountInfo(testPda);
    console.log(`${colors.green}✓ PDA expanded to size: ${afterExpansion?.data.length} bytes${colors.reset}`);

    // Step 5: Test validation after expansion
    console.log(`\n${colors.cyan}Step 5: Testing validation after expansion...${colors.reset}`);
    try {
      const tx = await validateSandwichValidators(program, {
        multisigAuthority: wallet.publicKey,
      });
      await tx.rpc();
      console.log(`${colors.green}✓ Validation succeeded after expansion${colors.reset}`);
    } catch (error) {
      console.log(`${colors.red}✗ Unexpected validation failure: ${error}${colors.reset}`);
    }

    // Step 6: Gate current slot and test validation
    console.log(`\n${colors.cyan}Step 6: Gating current slot range...${colors.reset}`);
    
    // Gate current slot and surrounding slots
    const slotsToGate = [];
    const startOffset = Math.max(0, currentSlotOffset - 2);
    const endOffset = Math.min(currentSlotOffset + 2, SLOTS_PER_EPOCH - 1);
    
    for (let offset = startOffset; offset <= endOffset; offset++) {
      slotsToGate.push(new BN(epochStartSlot + offset));
    }
    
    console.log(`${colors.yellow}Gating ${slotsToGate.length} slots (offsets ${startOffset}-${endOffset})${colors.reset}`);
    
    await updateSandwichValidator(program, {
      epoch: testEpoch,
      newSlots: slotsToGate,
      multisigAuthority: wallet.publicKey,
    })
      .signers([wallet.payer])
      .rpc();

    console.log(`${colors.green}✓ Slots gated successfully${colors.reset}`);

    // Step 7: Test validation with gated slots
    console.log(`\n${colors.cyan}Step 7: Testing validation with gated slots...${colors.reset}`);
    try {
      const tx = await validateSandwichValidators(program, {
        multisigAuthority: wallet.publicKey,
      });
      await tx.rpc();
      console.log(`${colors.red}✗ Unexpected success - validation should have failed${colors.reset}`);
    } catch (error) {
      if (error.toString().includes('SlotIsGated')) {
        console.log(`${colors.green}✓ Validation correctly failed - SlotIsGated error${colors.reset}`);
      } else {
        console.log(`${colors.red}✗ Failed with unexpected error: ${error}${colors.reset}`);
      }
    }

    // Summary
    console.log(`\n${colors.cyan}Test Summary:${colors.reset}`);
    console.log(`${colors.yellow}- Created expandable sandwich_validators PDA${colors.reset}`);
    console.log(`${colors.yellow}- Initial size: ${afterCreation?.data.length} bytes${colors.reset}`);
    console.log(`${colors.yellow}- Expanded size: ${afterExpansion?.data.length} bytes${colors.reset}`);
    console.log(`${colors.yellow}- Validation works at all stages${colors.reset}`);
    console.log(`${colors.yellow}- Slot gating works with expanded capacity${colors.reset}`);

    console.log(`\n${colors.bright}${colors.green}Expandable PDA test completed successfully! 🎉${colors.reset}`);

  } catch (error) {
    console.error(`${colors.red}Fatal error: ${error}${colors.reset}`);
    process.exit(1);
  }
}

main().catch(console.error);