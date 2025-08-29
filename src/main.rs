use alloy::providers::{Provider, ProviderBuilder};
use alloy::primitives::{Address, U256};
use std::collections::HashMap;
use std::error::Error;
use std::fs::OpenOptions;
use std::path::Path;
use std::env;

use tokio::runtime::Runtime;
use tokio::time::{sleep, Duration, Instant};
use chrono::{DateTime, Utc};
use csv::Writer;
use serde::Serialize;
use dotenv::dotenv;

// Define the MoePair interface using alloy's sol! macro
alloy::sol!(
    #[sol(rpc)]
    interface IMoePair {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }
);

// Constants for pool addresses
const MOE_WMNT_POOL: &str = "0x763868612858358f62b05691dB82Ad35a9b3E110";
const JOE_MOE_POOL: &str = "0xb670D2B452D0Ecc468cccFD532482d45dDdDe2a1";
const JOE_WMNT_POOL: &str = "0xEFC38C1B0d60725B824EBeE8D431aBFBF12BC953";

// Token addresses on Mantle
const WMNT_ADDRESS: &str = "0x78c1b0c915c4faa5fffa6cabf0219da63d7f4cb8";
const MOE_ADDRESS: &str = "0x4515a45337f461a11ff0fe8abf3c606ae5dc00c9";
const JOE_ADDRESS: &str = "0x371c7ec6d8039ff7933a2aa28eb827ffe1f52f07";

// Default configuration constants (can be overridden by environment variables)
const DEFAULT_TRANSACTION_COST_MNT: f64 = 0.02; // Transaction cost in MNT
const DEFAULT_BLOCK_TIME_SECONDS: u64 = 2; // Mantle block time
const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_CSV_FILE_PATH: &str = "arbitrage_opportunities.csv";
const DEFAULT_DEX_FEE: f64 = 0.003; // 0.3% fee for most DEXes
const DEFAULT_TERNARY_SEARCH_ITERATIONS: usize = 100; // Iterations for ternary search

// Configuration structure for runtime settings
#[derive(Debug, Clone)]
struct Config {
    rpc_url: String,
    transaction_cost_mnt: f64,
    block_time_seconds: u64,
    max_retries: u32,
    csv_file_path: String,
    dex_fee: f64,
    ternary_search_iterations: usize,
}

impl Config {
    fn load() -> Result<Self, Box<dyn Error>> {
        // Load .env file if it exists
        let _ = dotenv();

        let rpc_url = env::var("RPC_URL")
            .or_else(|_| env::var("MANTLE_RPC_URL"))
            .map_err(|_| "RPC_URL environment variable is required. Please set RPC_URL=your_rpc_endpoint")?;

        let transaction_cost_mnt = env::var("TRANSACTION_COST_MNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TRANSACTION_COST_MNT);

        let block_time_seconds = env::var("BLOCK_TIME_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_BLOCK_TIME_SECONDS);

        let max_retries = env::var("MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_RETRIES);

        let csv_file_path = env::var("CSV_FILE_PATH")
            .unwrap_or_else(|_| DEFAULT_CSV_FILE_PATH.to_string());

        let dex_fee = env::var("DEX_FEE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_DEX_FEE);

        let ternary_search_iterations = env::var("TERNARY_SEARCH_ITERATIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TERNARY_SEARCH_ITERATIONS);

        Ok(Config {
            rpc_url,
            transaction_cost_mnt,
            block_time_seconds,
            max_retries,
            csv_file_path,
            dex_fee,
            ternary_search_iterations,
        })
    }
}

// Token enum for identification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Token {
    WMNT(Address),
    MOE(Address),
    JOE(Address),
}

// CSV record structure for arbitrage opportunities
#[derive(Debug, Serialize)]
struct ArbitrageRecord {
    timestamp: String,
    block_number: u64,
    optimal_input_wmnt: f64,      // Optimal input found by ternary search
    final_output_wmnt: f64,       // Final output amount
    gross_profit_wmnt: f64,       // Gross profit before transaction costs
    net_profit_wmnt: f64,         // Net profit after transaction costs
    profit_percentage: f64,       // Profit percentage
    gas_cost_mnt: f64,           // Transaction cost
    search_method: String,        // "ternary_search" or "fixed_amount"
    moe_wmnt_reserve0: String,
    moe_wmnt_reserve1: String,
    joe_moe_reserve0: String,
    joe_moe_reserve1: String,
    joe_wmnt_reserve0: String,
    joe_wmnt_reserve1: String,
    fetch_time_ms: u64,
}

// More accurate swap function using constant product formula (x*y=k)
fn swap(x_reserve: f64, y_reserve: f64, dx: f64, fee: f64) -> f64 {
    if dx <= 0.0 || x_reserve <= 0.0 || y_reserve <= 0.0 {
        return 0.0;
    }
    let dx_after_fee = dx * (1.0 - fee);
    (y_reserve * dx_after_fee) / (x_reserve + dx_after_fee)
}

// Calculate arbitrage profit for a given input amount
fn arbitrage_profit(
    dx: f64,
    pools: &[(f64, f64)], // [(x,y), (x,y), (x,y)] - 3 pools in the arbitrage path
    fee: f64,
) -> f64 {
    if pools.len() != 3 {
        return -1.0; // Invalid input
    }
    
    // pool1: token0 -> token1
    let dy1 = swap(pools[0].0, pools[0].1, dx, fee);
    // pool2: token1 -> token2  
    let dy2 = swap(pools[1].0, pools[1].1, dy1, fee);
    // pool3: token2 -> token0
    let dy3 = swap(pools[2].0, pools[2].1, dy2, fee);

    dy3 - dx // profit (can be negative)
}

// Find optimal input amount using ternary search
fn find_best_input(
    pools: &[(f64, f64)], // 3 pools
    fee: f64,
    iterations: usize,
) -> (f64, f64) {
    let mut left = 0.0;
    let mut right = pools[0].0 * 0.999; // Upper limit close to pool's total token0 reserves
    
    // Ternary search for maximum profit
    for _ in 0..iterations {
        let m1 = left + (right - left) / 3.0;
        let m2 = right - (right - left) / 3.0;
        let p1 = arbitrage_profit(m1, pools, fee);
        let p2 = arbitrage_profit(m2, pools, fee);
        
        if p1 < p2 {
            left = m1;
        } else {
            right = m2;
        }
    }
    
    let best_input = (left + right) / 2.0;
    let best_profit = arbitrage_profit(best_input, pools, fee);
    (best_input, best_profit)
}

// Helper function to convert U256 to f64 (in token units, not wei)
fn u256_to_f64(value: U256) -> f64 {
    value.to_string().parse::<f64>().unwrap_or(0.0) / 1e18
}

// Helper function to convert f64 to U256 (from token units to wei)
#[allow(dead_code)]
fn f64_to_u256(value: f64) -> U256 {
    U256::from((value * 1e18) as u128)
}

// Legacy function for compatibility (kept for potential future use)
#[allow(dead_code)]
fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
    if amount_in == U256::ZERO || reserve_in == U256::ZERO || reserve_out == U256::ZERO {
        return U256::ZERO;
    }
    let amount_in_with_fee = amount_in * U256::from(997u64);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * U256::from(1000u64) + amount_in_with_fee;
    numerator / denominator
}

// Struct to hold reserves with token mapping
#[derive(Debug, Clone, PartialEq)]
struct PoolReserves {
    token_a: Token,
    reserve_a: U256,
    token_b: Token,
    reserve_b: U256,
    block_number: u64,
    timestamp: DateTime<Utc>,
}

// Cache structure for reserves
#[derive(Debug, Clone)]
struct ReservesCache {
    data: HashMap<Address, PoolReserves>,
    last_block: u64,
}

impl ReservesCache {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
            last_block: 0,
        }
    }

    #[allow(dead_code)]
    fn get(&self, address: &Address) -> Option<&PoolReserves> {
        self.data.get(address)
    }

    fn update(&mut self, address: Address, reserves: PoolReserves) {
        self.last_block = reserves.block_number;
        self.data.insert(address, reserves);
    }

    fn has_changed(&self, block_number: u64) -> bool {
        block_number > self.last_block
    }

    // Check if any reserves have actually changed compared to cache
    fn reserves_changed(&self, new_reserves: &HashMap<Address, PoolReserves>) -> bool {
        if self.data.len() != new_reserves.len() {
            return true;
        }
        
        for (addr, new_reserve) in new_reserves {
            match self.data.get(addr) {
                Some(cached_reserve) => {
                    if cached_reserve.reserve_a != new_reserve.reserve_a 
                        || cached_reserve.reserve_b != new_reserve.reserve_b {
                        return true;
                    }
                }
                None => return true,
            }
        }
        false
    }
}

async fn fetch_pool_reserves<P: Provider + Clone>(
    provider: P,
    pool_address: Address,
    block_number: u64,
) -> Result<PoolReserves, Box<dyn Error>> {
    let contract = IMoePair::new(pool_address, provider.clone());

    // Fetch token0 and token1
    let token0_addr = contract.token0().call().await?;
    let token1_addr = contract.token1().call().await?;

    // Map to our Token enum
    let token0 = match token0_addr.to_checksum(None).to_lowercase().as_str() {
        WMNT_ADDRESS => Token::WMNT(token0_addr),
        MOE_ADDRESS => Token::MOE(token0_addr),
        JOE_ADDRESS => Token::JOE(token0_addr),
        _ => return Err("Unknown token".into()),
    };
    let token1 = match token1_addr.to_checksum(None).to_lowercase().as_str() {
        WMNT_ADDRESS => Token::WMNT(token1_addr),
        MOE_ADDRESS => Token::MOE(token1_addr),
        JOE_ADDRESS => Token::JOE(token1_addr),
        _ => return Err("Unknown token".into()),
    };

    // Fetch reserves
    let reserves = contract.getReserves().call().await?;
    let reserve0 = U256::from(reserves.reserve0);
    let reserve1 = U256::from(reserves.reserve1);

    Ok(PoolReserves {
        token_a: token0,
        reserve_a: reserve0,
        token_b: token1,
        reserve_b: reserve1,
        block_number,
        timestamp: Utc::now(),
    })
}

// Parallel fetch all pool reserves with retry mechanism
async fn fetch_all_reserves_with_retry<P: Provider + Clone>(
    provider: P,
    pool_addresses: &[Address],
    block_number: u64,
    max_retries: u32,
) -> Result<HashMap<Address, PoolReserves>, Box<dyn Error>> {
    let mut attempts = 0;
    
    while attempts < max_retries {
        let futures: Vec<_> = pool_addresses.iter().map(|&addr| {
            let provider_clone = provider.clone();
            async move {
                (addr, fetch_pool_reserves(provider_clone, addr, block_number).await)
            }
        }).collect();

        let results = futures::future::join_all(futures).await;
        let mut reserves_map = HashMap::new();
        let mut all_succeeded = true;

        for (addr, result) in results {
            match result {
                Ok(reserves) => {
                    reserves_map.insert(addr, reserves);
                }
                Err(e) => {
                    println!("Error fetching reserves for pool {}: {}", addr, e);
                    all_succeeded = false;
                    break;
                }
            }
        }

        if all_succeeded {
            return Ok(reserves_map);
        }

        attempts += 1;
        if attempts < max_retries {
            println!("Retrying... attempt {} of {}", attempts + 1, max_retries);
            sleep(Duration::from_secs(1)).await;
        }
    }

    Err("Failed to fetch reserves after maximum retries".into())
}

// Extract and normalize pool reserves for ternary search algorithm
fn prepare_pools_for_search(
    moe_wmnt: &PoolReserves,
    joe_moe: &PoolReserves,
    joe_wmnt: &PoolReserves,
) -> Option<Vec<(f64, f64)>> {
    // Pool 1: WMNT -> MOE in MOE-WMNT pool
    let (wmnt_reserve1, moe_reserve1) = match (moe_wmnt.token_a, moe_wmnt.token_b) {
        (Token::WMNT(_), Token::MOE(_)) => (u256_to_f64(moe_wmnt.reserve_a), u256_to_f64(moe_wmnt.reserve_b)),
        (Token::MOE(_), Token::WMNT(_)) => (u256_to_f64(moe_wmnt.reserve_b), u256_to_f64(moe_wmnt.reserve_a)),
        _ => return None,
    };

    // Pool 2: MOE -> JOE in JOE-MOE pool
    let (moe_reserve2, joe_reserve2) = match (joe_moe.token_a, joe_moe.token_b) {
        (Token::MOE(_), Token::JOE(_)) => (u256_to_f64(joe_moe.reserve_a), u256_to_f64(joe_moe.reserve_b)),
        (Token::JOE(_), Token::MOE(_)) => (u256_to_f64(joe_moe.reserve_b), u256_to_f64(joe_moe.reserve_a)),
        _ => return None,
    };

    // Pool 3: JOE -> WMNT in JOE-WMNT pool
    let (joe_reserve3, wmnt_reserve3) = match (joe_wmnt.token_a, joe_wmnt.token_b) {
        (Token::JOE(_), Token::WMNT(_)) => (u256_to_f64(joe_wmnt.reserve_a), u256_to_f64(joe_wmnt.reserve_b)),
        (Token::WMNT(_), Token::JOE(_)) => (u256_to_f64(joe_wmnt.reserve_b), u256_to_f64(joe_wmnt.reserve_a)),
        _ => return None,
    };

    Some(vec![
        (wmnt_reserve1, moe_reserve1),    // Pool 1: WMNT -> MOE
        (moe_reserve2, joe_reserve2),     // Pool 2: MOE -> JOE
        (joe_reserve3, wmnt_reserve3),    // Pool 3: JOE -> WMNT
    ])
}

// New optimized arbitrage function using ternary search
fn find_optimal_arbitrage(
    moe_wmnt: &PoolReserves,
    joe_moe: &PoolReserves,
    joe_wmnt: &PoolReserves,
    config: &Config,
) -> Option<(f64, f64, f64)> { // (optimal_input, final_output, gross_profit)
    let pools = prepare_pools_for_search(moe_wmnt, joe_moe, joe_wmnt)?;
    
    let (best_input, gross_profit) = find_best_input(&pools, config.dex_fee, config.ternary_search_iterations);
    
    // Calculate final output amount
    let final_output = best_input + gross_profit;
    
    Some((best_input, final_output, gross_profit))
}

// Legacy arbitrage function (kept for potential comparison/debugging)
#[allow(dead_code)]
fn check_arbitrage(
    moe_wmnt: &PoolReserves,
    joe_moe: &PoolReserves,
    joe_wmnt: &PoolReserves,
    start_amount: U256,
) -> (bool, U256, U256) {
    // Path: WMNT -> MOE -> JOE -> WMNT

    // Step 1: WMNT -> MOE in MOE-WMNT pool
    let (wmnt_reserve, moe_reserve) = match (moe_wmnt.token_a, moe_wmnt.token_b) {
        (Token::WMNT(_), Token::MOE(_)) => (moe_wmnt.reserve_a, moe_wmnt.reserve_b),
        (Token::MOE(_), Token::WMNT(_)) => (moe_wmnt.reserve_b, moe_wmnt.reserve_a),
        _ => return (false, U256::ZERO, U256::ZERO),
    };
    let moe_out = get_amount_out(start_amount, wmnt_reserve, moe_reserve);

    // Step 2: MOE -> JOE in JOE-MOE pool
    let (moe_reserve, joe_reserve) = match (joe_moe.token_a, joe_moe.token_b) {
        (Token::MOE(_), Token::JOE(_)) => (joe_moe.reserve_a, joe_moe.reserve_b),
        (Token::JOE(_), Token::MOE(_)) => (joe_moe.reserve_b, joe_moe.reserve_a),
        _ => return (false, U256::ZERO, U256::ZERO),
    };
    let joe_out = get_amount_out(moe_out, moe_reserve, joe_reserve);

    // Step 3: JOE -> WMNT in JOE-WMNT pool
    let (joe_reserve, wmnt_reserve) = match (joe_wmnt.token_a, joe_wmnt.token_b) {
        (Token::JOE(_), Token::WMNT(_)) => (joe_wmnt.reserve_a, joe_wmnt.reserve_b),
        (Token::WMNT(_), Token::JOE(_)) => (joe_wmnt.reserve_b, joe_wmnt.reserve_a),
        _ => return (false, U256::ZERO, U256::ZERO),
    };
    let wmnt_out = get_amount_out(joe_out, joe_reserve, wmnt_reserve);

    // Calculate transaction cost in wei (0.02 MNT = 0.02 * 10^18 wei)
    let tx_cost = U256::from((DEFAULT_TRANSACTION_COST_MNT * 1e18) as u128);
    
    // Calculate net profit after transaction costs
    let net_profit = if wmnt_out > start_amount + tx_cost {
        wmnt_out - start_amount - tx_cost
    } else {
        U256::ZERO
    };
    
    // Check if profitable after considering transaction costs
    let profitable = net_profit > U256::ZERO;
    (profitable, wmnt_out, net_profit)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Load configuration from environment variables
    let config = Config::load().map_err(|e| {
        eprintln!("Configuration Error: {}", e);
        eprintln!("Please set the RPC_URL environment variable.");
        eprintln!("Example: export RPC_URL=https://rpc.mantle.xyz");
        eprintln!("Or create a .env file with: RPC_URL=your_rpc_endpoint");
        e
    })?;

    let rt = Runtime::new()?;

    rt.block_on(async {
        // Set up provider
        let provider = ProviderBuilder::new().connect_http(config.rpc_url.parse()?);

        // Parse pool addresses
        let moe_wmnt_addr: Address = MOE_WMNT_POOL.parse()?;
        let joe_moe_addr: Address = JOE_MOE_POOL.parse()?;
        let joe_wmnt_addr: Address = JOE_WMNT_POOL.parse()?;
        let pool_addresses = vec![moe_wmnt_addr, joe_moe_addr, joe_wmnt_addr];

        // Initialize CSV file
        if let Err(e) = init_csv_file(&config.csv_file_path) {
            println!("⚠️ Warning: Failed to initialize CSV file: {}", e);
        } else {
            println!("📝 CSV logging initialized: {}", config.csv_file_path);
        }

        // Initialize cache
        let mut cache = ReservesCache::new();
        
        println!("🚀 Starting triangular arbitrage monitor on Mantle Network");
        println!("📊 Monitoring pools: MOE-WMNT, JOE-MOE, JOE-WMNT");
        println!("🔍 Algorithm: Ternary search optimization ({} iterations)", config.ternary_search_iterations);
        println!("🌐 RPC URL: {}", config.rpc_url);
        println!("⛽ Transaction cost: {} MNT", config.transaction_cost_mnt);
        println!("💹 DEX fee: {}%", config.dex_fee * 100.0);
        println!("⏰ Block time: {} seconds", config.block_time_seconds);
        println!("📝 Logging: Only when reserves change (not every block)");
        println!("📋 Reserves info: Included in each update\n");

        // Block-based monitoring loop
        loop {
            let start_time = Instant::now();
            
            // Get current block number
            let current_block = match provider.get_block_number().await {
                Ok(block) => block,
                Err(e) => {
                    println!("❌ Error getting block number: {}", e);
                    sleep(Duration::from_secs(config.block_time_seconds)).await;
                    continue;
                }
            };

                        // Only fetch and process if block has changed
            if cache.has_changed(current_block) {
                // Fetch all reserves in parallel
                match fetch_all_reserves_with_retry(&provider, &pool_addresses, current_block, config.max_retries).await {
                    Ok(reserves_map) => {
                        // Check if reserves have actually changed
                        if cache.reserves_changed(&reserves_map) {
                            let timestamp = Utc::now();
                            let fetch_duration = start_time.elapsed();
                            
                            // Print log with reserves information only when reserves change
                            println!("🔄 Reserves changed at block {} ({})", current_block, timestamp.format("%H:%M:%S%.3f"));
                            println!("{}", format_pool_reserves(moe_wmnt_addr, joe_moe_addr, joe_wmnt_addr, &reserves_map));

                            // Update cache
                            for (addr, reserves) in &reserves_map {
                                cache.update(*addr, reserves.clone());
                            }

                            // Use ternary search to find optimal arbitrage opportunity
                            let moe_wmnt_reserves = &reserves_map[&moe_wmnt_addr];
                            let joe_moe_reserves = &reserves_map[&joe_moe_addr];
                            let joe_wmnt_reserves = &reserves_map[&joe_wmnt_addr];

                            // Find optimal arbitrage using ternary search
                            match find_optimal_arbitrage(moe_wmnt_reserves, joe_moe_reserves, joe_wmnt_reserves, &config) {
                                Some((optimal_input, final_output, gross_profit)) => {
                                    // Calculate net profit after transaction costs
                                    let net_profit = gross_profit - config.transaction_cost_mnt;
                                    
                                    if net_profit > 0.0 {
                                        let profit_percentage = if optimal_input > 0.0 { (net_profit / optimal_input) * 100.0 } else { 0.0 };

                                        println!("💎 OPTIMAL ARBITRAGE OPPORTUNITY FOUND!");
                                        println!("   🎯 Optimal Input: {:.6} WMNT (via ternary search)", optimal_input);
                                        println!("   📈 Final Output: {:.6} WMNT", final_output);
                                        println!("   💰 Gross Profit: {:.6} WMNT", gross_profit);
                                        println!("   🎯 Net Profit: {:.6} WMNT ({:.2}%)", net_profit, profit_percentage);
                                        println!("   ⛽ After {} MNT tx cost", config.transaction_cost_mnt);
                                        println!("   🔍 Search iterations: {}", config.ternary_search_iterations);
                                        println!("   ⚡ Analysis time: {:?}", fetch_duration);

                                        // Write to CSV
                                        if let Err(e) = write_arbitrage_to_csv(
                                            timestamp,
                                            current_block,
                                            optimal_input,
                                            final_output,
                                            gross_profit,
                                            net_profit,
                                            "ternary_search",
                                            moe_wmnt_reserves,
                                            joe_moe_reserves,
                                            joe_wmnt_reserves,
                                            fetch_duration.as_millis() as u64,
                                            &config,
                                        ) {
                                            println!("   ⚠️ Failed to write to CSV: {}", e);
                                        } else {
                                            println!("   ✅ Logged to CSV: {}", config.csv_file_path);
                                        }
                                    } else {
                                        println!("   📊 No profitable opportunity after costs. Gross: {:.6}, Net: {:.6} WMNT, Time: {:?}", 
                                            gross_profit, net_profit, fetch_duration);
                                    }
                                }
                                None => {
                                    println!("   ❌ Failed to analyze pools. Analysis time: {:?}", fetch_duration);
                                }
                            }
                            println!(); // Add blank line for readability
                        } else {
                            // Update cache block number even if reserves didn't change
                            cache.last_block = current_block;
                        }
                    }
                Err(e) => {
                        println!("❌ Block {}: Failed to fetch reserves: {}", current_block, e);
                    }
                }
            }

            // Sleep until next expected block (with a small buffer)
            sleep(Duration::from_millis((config.block_time_seconds * 1000) - 200)).await;
        }
    })
}

// Function to initialize CSV file with headers
fn init_csv_file(csv_file_path: &str) -> Result<(), Box<dyn Error>> {
    if !Path::new(csv_file_path).exists() {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(csv_file_path)?;
        
        let mut writer = Writer::from_writer(file);
        
        // Write header if file is new
        writer.write_record(&[
            "timestamp", "block_number", "optimal_input_wmnt", "final_output_wmnt", 
            "gross_profit_wmnt", "net_profit_wmnt", "profit_percentage", "gas_cost_mnt", "search_method",
            "moe_wmnt_reserve0", "moe_wmnt_reserve1", 
            "joe_moe_reserve0", "joe_moe_reserve1",
            "joe_wmnt_reserve0", "joe_wmnt_reserve1", "fetch_time_ms"
        ])?;
        writer.flush()?;
    }
    Ok(())
}

// Function to write arbitrage opportunity to CSV
fn write_arbitrage_to_csv(
    timestamp: DateTime<Utc>,
    block_number: u64,
    optimal_input: f64,
    final_output: f64,
    gross_profit: f64,
    net_profit: f64,
    search_method: &str,
    moe_wmnt_reserves: &PoolReserves,
    joe_moe_reserves: &PoolReserves,
    joe_wmnt_reserves: &PoolReserves,
    fetch_time_ms: u64,
    config: &Config,
) -> Result<(), Box<dyn Error>> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&config.csv_file_path)?;
    
    let mut writer = Writer::from_writer(file);
    
    let profit_percentage = if optimal_input > 0.0 { (net_profit / optimal_input) * 100.0 } else { 0.0 };
    
    let record = ArbitrageRecord {
        timestamp: timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string(),
        block_number,
        optimal_input_wmnt: optimal_input,
        final_output_wmnt: final_output,
        gross_profit_wmnt: gross_profit,
        net_profit_wmnt: net_profit,
        profit_percentage,
        gas_cost_mnt: config.transaction_cost_mnt,
        search_method: search_method.to_string(),
        moe_wmnt_reserve0: moe_wmnt_reserves.reserve_a.to_string(),
        moe_wmnt_reserve1: moe_wmnt_reserves.reserve_b.to_string(),
        joe_moe_reserve0: joe_moe_reserves.reserve_a.to_string(),
        joe_moe_reserve1: joe_moe_reserves.reserve_b.to_string(),
        joe_wmnt_reserve0: joe_wmnt_reserves.reserve_a.to_string(),
        joe_wmnt_reserve1: joe_wmnt_reserves.reserve_b.to_string(),
        fetch_time_ms,
    };
    
    writer.serialize(&record)?;
    writer.flush()?;
    Ok(())
}

// Function to format reserves information for logging
fn format_pool_reserves(
    moe_wmnt_addr: Address,
    joe_moe_addr: Address, 
    joe_wmnt_addr: Address,
    reserves_map: &HashMap<Address, PoolReserves>
) -> String {
    let mut output = String::new();
    
    // MOE-WMNT Pool
    if let Some(reserves) = reserves_map.get(&moe_wmnt_addr) {
        let (wmnt_reserve, moe_reserve, pool_name) = match (reserves.token_a, reserves.token_b) {
            (Token::WMNT(_), Token::MOE(_)) => (u256_to_f64(reserves.reserve_a), u256_to_f64(reserves.reserve_b), "MOE-WMNT"),
            (Token::MOE(_), Token::WMNT(_)) => (u256_to_f64(reserves.reserve_b), u256_to_f64(reserves.reserve_a), "MOE-WMNT"),
            _ => (0.0, 0.0, "MOE-WMNT(?)"),
        };
        output.push_str(&format!("   📊 {}: {:.2} WMNT / {:.2} MOE\n", pool_name, wmnt_reserve, moe_reserve));
    }
    
    // JOE-MOE Pool
    if let Some(reserves) = reserves_map.get(&joe_moe_addr) {
        let (moe_reserve, joe_reserve, pool_name) = match (reserves.token_a, reserves.token_b) {
            (Token::MOE(_), Token::JOE(_)) => (u256_to_f64(reserves.reserve_a), u256_to_f64(reserves.reserve_b), "JOE-MOE"),
            (Token::JOE(_), Token::MOE(_)) => (u256_to_f64(reserves.reserve_b), u256_to_f64(reserves.reserve_a), "JOE-MOE"),
            _ => (0.0, 0.0, "JOE-MOE(?)"),
        };
        output.push_str(&format!("   📊 {}: {:.2} MOE / {:.2} JOE\n", pool_name, moe_reserve, joe_reserve));
    }
    
    // JOE-WMNT Pool
    if let Some(reserves) = reserves_map.get(&joe_wmnt_addr) {
        let (joe_reserve, wmnt_reserve, pool_name) = match (reserves.token_a, reserves.token_b) {
            (Token::JOE(_), Token::WMNT(_)) => (u256_to_f64(reserves.reserve_a), u256_to_f64(reserves.reserve_b), "JOE-WMNT"),
            (Token::WMNT(_), Token::JOE(_)) => (u256_to_f64(reserves.reserve_b), u256_to_f64(reserves.reserve_a), "JOE-WMNT"),
            _ => (0.0, 0.0, "JOE-WMNT(?)"),
        };
        output.push_str(&format!("   📊 {}: {:.2} JOE / {:.2} WMNT", pool_name, joe_reserve, wmnt_reserve));
    }
    
    output
}

// Note: Optimal amounts are now calculated automatically using ternary search