#![allow(unused)]
use bitcoin::{
    address::{NetworkChecked, NetworkUnchecked},
    hex::DisplayHex,
    Address,
};
use bitcoincore_rpc::bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;

// Node access params
const RPC_URL: &str = "http://127.0.0.1:18443"; // Default regtest RPC port
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";

// You can use calls not provided in RPC lib API using the generic `call` function.
// An example of using the `send` RPC call, which doesn't have exposed API.
// You can also use serde_json `Deserialize` derivation to capture the returned json result.
fn send(rpc: &Client, addr: &str) -> bitcoincore_rpc::Result<String> {
    let args = [
        json!([{addr : 100 }]), // recipient address
        json!(null),            // conf target
        json!(null),            // estimate mode
        json!(null),            // fee rate in sats/vb
        json!(null),            // Empty option object
    ];

    #[derive(Deserialize)]
    struct SendResult {
        complete: bool,
        txid: String,
    }
    let send_result = rpc.call::<SendResult>("send", &args)?;
    assert!(send_result.complete);
    Ok(send_result.txid)
}

fn main() -> bitcoincore_rpc::Result<()> {
    // Connect to Bitcoin Core RPC
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Get blockchain info
    let blockchain_info = rpc.get_blockchain_info();
    println!("Blockchain Info: {:?}", blockchain_info);

    fn unload_all_wallets(rpc: &Client) {
        let wallets = rpc.list_wallets().unwrap();
        for wallet in wallets {
            rpc.unload_wallet(Some(&wallet)).unwrap();
        }
    }

    // Create/Load the wallets, named 'Miner' and 'Trader'. Have logic to optionally create/load them if they do not exist or not loaded already.
    unload_all_wallets(&rpc);
    let wallets = ["Miner", "Trader"];
    for wallet in wallets {
        // rpc.unload_wallet(Some(wallet));
        match rpc.load_wallet(wallet) {
            Ok(_) => println!("Wallet {} loaded successfully", wallet),
            Err(_) => {
                rpc.create_wallet(wallet, None, None, None, None)?;
            }
        }
    }

    // Generate spendable balances in the Miner wallet. How many blocks needs to be mined?
    // need to mine 101 blocks in order to spend the mining rewards

    // get a random address from the Miner wallet
    unload_all_wallets(&rpc);
    rpc.load_wallet("Miner")?;
    let miner_address = rpc.get_new_address(None, None)?;
    let balance = rpc.get_balance(None, None)?;
    if (balance < Amount::from_int_btc(25)) {
        rpc.generate_to_address(101, &miner_address.assume_checked())?;
    }

    // Load Trader wallet and generate a new address
    unload_all_wallets(&rpc);
    rpc.load_wallet("Trader")?;
    let trader_address = rpc.get_new_address(None, None)?;

    // Send 20 BTC from Miner to Trader
    unload_all_wallets(&rpc);
    rpc.load_wallet("Miner")?;
    let txid = rpc.send_to_address(
        &trader_address.clone().assume_checked(),
        Amount::from_int_btc(20),
        None,
        None,
        Some(false),
        Some(false),
        None,
        None,
    )?;

    // Check transaction in mempool
    let mempool_txs = rpc.get_raw_mempool()?;
    if !mempool_txs.contains(&txid) {
        return Ok(());
    }

    // Mine 1 block to confirm the transaction
    let other_miner_address = rpc.get_new_address(None, None)?;
    rpc.generate_to_address(1, &other_miner_address.assume_checked())?;

    // Extract all required transaction details
    let tx = rpc.get_transaction(&txid, None)?;

    // Write the data to ../out.txt in the specified format given in readme.md
    let mut file = File::create("out.txt")?;

    let blockhash = tx.info.blockhash.unwrap();

    let mut miner_change_address: Option<Address<NetworkUnchecked>> = None;
    let mut miner_change_amount = Amount::ZERO;
    let mut miner_input_address: Option<Address<NetworkUnchecked>> = None;
    let mut miner_input_amount = Amount::ZERO;
    let mut trader_output_address: Option<Address<NetworkUnchecked>> = None;
    let mut trader_output_amount = Amount::ZERO;

    let mut raw_tx = rpc.get_raw_transaction_info(&txid, Some(&blockhash))?;
    for vin in raw_tx.vin {
        let prev_vout = vin.vout.unwrap() as usize;
        if let Some(prev_txid) = vin.txid {
            let prev_tx = rpc.get_raw_transaction_info(&prev_txid, None)?;
            miner_input_amount = prev_tx.vout[prev_vout].value;
            miner_input_address = prev_tx.vout[prev_vout].script_pub_key.address.clone();
        }
    }

    for vout in raw_tx.vout {
        let output_amount = vout.value;
        let output_address = vout.script_pub_key.address.clone();
        let output_address_string = output_address.clone().unwrap().assume_checked().to_string();
        if output_address == Some(trader_address.clone()) {
            trader_output_address = output_address;
            trader_output_amount = output_amount;
        } else {
            miner_change_address = output_address;
            miner_change_amount = output_amount;
        }
    }

    writeln!(file, "{}", tx.info.txid);
    writeln!(
        file,
        "{:?}",
        miner_input_address.unwrap().assume_checked().to_string()
    );
    writeln!(file, "{}", miner_input_amount.to_btc());
    writeln!(
        file,
        "{:?}",
        trader_output_address.unwrap().assume_checked().to_string()
    );
    writeln!(file, "{}", trader_output_amount.to_btc());
    writeln!(
        file,
        "{:?}",
        miner_change_address.unwrap().assume_checked().to_string()
    );
    writeln!(file, "{}", miner_change_amount.to_btc());
    writeln!(file, "{}", tx.fee.unwrap().to_btc());
    writeln!(file, "{}", tx.info.blockheight.unwrap());
    writeln!(file, "{}", blockhash);

    Ok(())
}
