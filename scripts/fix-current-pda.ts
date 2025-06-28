import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  // expandSandwichValidators, // REMOVED: Function was removed from contract and SDK
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

async function main() {
  console.log("=== Fix Current Epoch PDA Bitmap Length ===\n");

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

    // Get current epoch
    const currentSlot = await connection.getSlot();
    const currentEpoch = Math.floor(currentSlot / SLOTS_PER_EPOCH);
    
    console.log(`Current epoch: ${currentEpoch}`);

    // Check current PDA state
    const { pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
    const beforeInfo = await connection.getAccountInfo(pda);
    
    if (beforeInfo) {
      const beforeBitmapLen = beforeInfo.data.readUInt32LE(10);
      console.log(`Before fix - Account size: ${beforeInfo.data.length} bytes, Bitmap length: ${beforeBitmapLen} bytes`);
      
      // REMOVED: expandSandwichValidators was removed from the contract
      // The bitmap length is now handled correctly at PDA creation time
      console.log("NOTE: expandSandwichValidators was removed - bitmap length is fixed at creation");
      
      // await expandSandwichValidators(program, {
      //   epoch: currentEpoch,
      //   multisigAuthority: wallet.publicKey,
      // })
      //   .signers([wallet.payer])
      //   .rpc();

      // Check current state (no expansion possible)
      const currentInfo = await connection.getAccountInfo(pda);
      if (currentInfo) {
        const currentBitmapLen = currentInfo.data.readUInt32LE(10);
        console.log(`Current - Account size: ${currentInfo.data.length} bytes, Bitmap length: ${currentBitmapLen} bytes`);
        console.log(`Max trackable slots: ${currentBitmapLen * 8}`);
        console.log(`NOTE: If this PDA was created with the old contract, the bitmap size is fixed.`);
      }
    } else {
      console.log("No PDA exists for current epoch");
    }

  } catch (error) {
    console.error(`Error: ${error}`);
  }
}

main().catch(console.error);