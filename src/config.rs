use std::env;
use std::error::Error;
use dotenv::dotenv;
use crate::constants::*;

/// Configuration structure for runtime settings
#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub gas_price_gwei: f64,
    pub block_time_seconds: u64,
    pub max_retries: u32,
    pub csv_file_path: String,
    pub dex_fee: f64,
    pub ternary_search_iterations: usize,
}

impl Config {
    /// Load configuration from environment variables with fallback to defaults
    pub fn load() -> Result<Self, Box<dyn Error>> {
        // Load .env file if it exists
        let _ = dotenv();

        let rpc_url = env::var("RPC_URL")
            .or_else(|_| env::var("MANTLE_RPC_URL"))
            .map_err(|_| "RPC_URL environment variable is required. Please set RPC_URL=your_rpc_endpoint")?;

        let gas_price_gwei = env::var("GAS_PRICE_GWEI")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_GAS_PRICE_GWEI);

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
            gas_price_gwei,
            block_time_seconds,
            max_retries,
            csv_file_path,
            dex_fee,
            ternary_search_iterations,
        })
    }

    /// Print configuration summary
    pub fn print_summary(&self) {
        println!("🔍 Algorithm: Ternary search optimization ({} iterations)", self.ternary_search_iterations);
        println!("🌐 RPC URL: {}", self.rpc_url);
        println!("⛽ Gas Price: {:.3} gwei", self.gas_price_gwei);
        println!("💸 Gas Cost (3-hops): {:.6} MNT", self.calculate_gas_cost(GAS_UNITS_3_HOPS));
        println!("💸 Gas Cost (4-hops): {:.6} MNT", self.calculate_gas_cost(GAS_UNITS_4_HOPS));
        println!("💹 DEX fee: {}%", self.dex_fee * 100.0);
        println!("⏰ Block time: {} seconds", self.block_time_seconds);
        println!("📝 Logging: Only when reserves change (not every block)");
        println!("📋 Reserves info: Included in each update");
    }

    /// Calculate precise gas cost based on gas units (result in MNT)
    pub fn calculate_gas_cost(&self, gas_units: u64) -> f64 {
        gas_units as f64 * self.gas_price_gwei * GWEI_TO_MNT_MULTIPLIER
    }
}
