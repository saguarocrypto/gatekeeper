import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

async function main() {
  console.log("=== Check Specific Gated Range ===\n");

  try {
    // Setup
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

    const program = anchor.workspace.SaguaroGatekeeper as anchor.Program<SaguaroGatekeeper>;

    // Get current slot info
    const currentSlot = await connection.getSlot();
    const currentEpoch = Math.floor(currentSlot / SLOTS_PER_EPOCH);
    
    console.log(`Current slot: ${currentSlot}`);
    console.log(`Current epoch: ${currentEpoch}`);

    // Check current PDA state
    const { pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
    const accountInfo = await connection.getAccountInfo(pda);
    
    if (accountInfo) {
      const bitmapLen = accountInfo.data.readUInt32LE(10);
      console.log(`\nBitmap length: ${bitmapLen} bytes`);
      
      // Check the specific range that should have been gated: 274094-274104
      console.log(`\nChecking range 274094-274104 (the range that was supposed to be gated):`);
      
      for (let offset = 274094; offset <= 274104; offset++) {
        const byteIndex = Math.floor(offset / 8);
        const bitIndex = offset % 8;
        const bytePos = 15 + byteIndex; // header size = 15
        
        if (bytePos < accountInfo.data.length) {
          const byte = accountInfo.data[bytePos];
          const bitSet = (byte >> bitIndex) & 1;
          console.log(`  Offset ${offset}: byte ${byteIndex}, bit ${bitIndex} = ${bitSet ? 'GATED' : 'not gated'} (byte=0x${byte.toString(16).padStart(2, '0')})`);
        } else {
          console.log(`  Offset ${offset}: beyond account data`);
        }
      }
      
      // Let's also scan for ANY set bits in a wider range
      console.log(`\nScanning for any set bits in range 274000-274200:`);
      let foundGatedSlots = false;
      for (let offset = 274000; offset < 274200; offset++) {
        const byteIndex = Math.floor(offset / 8);
        const bitIndex = offset % 8;
        const bytePos = 15 + byteIndex;
        
        if (bytePos < accountInfo.data.length) {
          const byte = accountInfo.data[bytePos];
          const bitSet = (byte >> bitIndex) & 1;
          if (bitSet) {
            console.log(`  Found gated slot at offset ${offset}: byte ${byteIndex}, bit ${bitIndex}`);
            foundGatedSlots = true;
          }
        }
      }
      
      if (!foundGatedSlots) {
        console.log("  No gated slots found in this range");
      }
      
    } else {
      console.log("No PDA exists for current epoch");
    }

  } catch (error) {
    console.error(`Error: ${error}`);
  }
}

main().catch(console.error);