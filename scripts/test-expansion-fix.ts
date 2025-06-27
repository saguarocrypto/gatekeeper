import * as anchor from "@coral-xyz/anchor";
import { BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  setSandwichValidators,
  expandSandwichValidators,
  updateSandwichValidator,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

// ANSI color codes
const colors = {
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
  reset: '\x1b[0m',
};

async function testExpansionFix() {
  console.log(`${colors.cyan}=== Testing Expansion Fix ===${colors.reset}`);
  
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  
  const program = anchor.workspace.SaguaroGatekeeper as anchor.Program<SaguaroGatekeeper>;
  const connection = provider.connection;
  
  // Load wallet
  const walletKeypair = anchor.web3.Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(readFileSync(provider.wallet.publicKey.toString().includes('FdgwEjyGYkbDSzQ1Qch6XPjUJRsqGyp7DzfzYtJ8Pn5h') ? 
      '/Users/christorreslugo/.config/solana/id.json' : 
      '/Users/christorreslugo/.config/solana/id.json', 'utf-8')))
  );
  
  const wallet = {
    publicKey: walletKeypair.publicKey,
    payer: walletKeypair,
  };
  
  console.log(`Program ID: ${program.programId.toString()}`);
  console.log(`Wallet: ${wallet.publicKey.toString()}`);
  
  // Use a unique test epoch to avoid conflicts
  const testEpoch = 8888;
  
  try {
    // Step 1: Create a fresh PDA with the new program
    console.log(`\n${colors.cyan}Step 1: Creating fresh PDA with fixed program...${colors.reset}`);
    const epochStartSlot = testEpoch * SLOTS_PER_EPOCH;
    const testSlots = [new BN(epochStartSlot + 1000)];
    
    const createSig = await setSandwichValidators(program, {
      epoch: testEpoch,
      slots: testSlots,
      multisigAuthority: wallet.publicKey,
    })
      .signers([wallet.payer])
      .rpc();
    
    console.log(`${colors.green}✓ Fresh PDA created${colors.reset}`);
    
    // Step 2: Check initial bitmap length
    const { pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(testEpoch), program.programId);
    const initialAccount = await connection.getAccountInfo(pda);
    
    if (initialAccount?.data) {
      const initialBitmapLen = initialAccount.data.readUInt32LE(10);
      console.log(`${colors.yellow}Initial: Account size: ${initialAccount.data.length}, Bitmap length field: ${initialBitmapLen}${colors.reset}`);
    }
    
    // Step 3: Expand the PDA
    console.log(`\n${colors.cyan}Step 2: Expanding PDA to full capacity...${colors.reset}`);
    
    const FULL_BITMAP_SIZE_BYTES = 54000;
    const METADATA_SIZE = 15;
    const targetSize = FULL_BITMAP_SIZE_BYTES + METADATA_SIZE;
    
    let currentSize = initialAccount?.data.length || 0;
    let expansionCount = 0;
    
    while (currentSize < targetSize && expansionCount < 10) {
      expansionCount++;
      console.log(`${colors.yellow}Expansion ${expansionCount}: Current size ${currentSize}, target ${targetSize}${colors.reset}`);
      
      await expandSandwichValidators(program, {
        epoch: testEpoch,
        multisigAuthority: wallet.publicKey,
      })
        .signers([wallet.payer])
        .rpc();
      
      const updatedAccount = await connection.getAccountInfo(pda);
      currentSize = updatedAccount?.data.length || 0;
      
      console.log(`${colors.green}✓ Expansion ${expansionCount}: New size ${currentSize}${colors.reset}`);
    }
    
    // Step 4: Check final bitmap length
    const finalAccount = await connection.getAccountInfo(pda);
    if (finalAccount?.data) {
      const finalBitmapLen = finalAccount.data.readUInt32LE(10);
      const maxTrackableSlots = finalBitmapLen * 8;
      console.log(`${colors.yellow}Final: Account size: ${finalAccount.data.length}, Bitmap length field: ${finalBitmapLen}${colors.reset}`);
      console.log(`${colors.yellow}Max trackable slots: ${maxTrackableSlots}${colors.reset}`);
      
      if (finalBitmapLen === FULL_BITMAP_SIZE_BYTES) {
        console.log(`${colors.green}✅ SUCCESS: Bitmap length field correctly updated!${colors.reset}`);
      } else {
        console.log(`${colors.red}❌ FAILED: Bitmap length field not updated correctly${colors.reset}`);
      }
    }
    
    // Step 5: Test updating with a slot that would have been out of range
    console.log(`\n${colors.cyan}Step 3: Testing slot update with large offset...${colors.reset}`);
    const testSlotOffset = 200000; // This should now work
    const testSlot = epochStartSlot + testSlotOffset;
    
    try {
      await updateSandwichValidator(program, {
        epoch: testEpoch,
        newSlots: [new BN(testSlot)],
        removeSlots: [],
        multisigAuthority: wallet.publicKey,
      })
        .signers([wallet.payer])
        .rpc();
      
      console.log(`${colors.green}✅ SUCCESS: Large slot offset ${testSlotOffset} accepted!${colors.reset}`);
    } catch (error) {
      console.log(`${colors.red}❌ FAILED: Large slot offset rejected: ${error}${colors.reset}`);
    }
    
  } catch (error) {
    console.log(`${colors.red}Test failed: ${error}${colors.reset}`);
  }
}

testExpansionFix();