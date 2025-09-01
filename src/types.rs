use alloy::primitives::{Address, U256};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Token enum for identification across different DEX protocols
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Token {
    WMNT(Address),
    MOE(Address), 
    JOE(Address),
}

impl Token {
    /// Get the address of the token
    pub fn address(&self) -> Address {
        match self {
            Token::WMNT(addr) | Token::MOE(addr) | Token::JOE(addr) => *addr,
        }
    }

    /// Get the symbol of the token
    pub fn symbol(&self) -> &'static str {
        match self {
            Token::WMNT(_) => "WMNT",
            Token::MOE(_) => "MOE", 
            Token::JOE(_) => "JOE",
        }
    }

    /// Create a token from address string (used for parsing from contracts)
    pub fn from_address(addr: Address) -> Option<Self> {
        let addr_str = addr.to_checksum(None).to_lowercase();
        match addr_str.as_str() {
            crate::constants::WMNT_ADDRESS => Some(Token::WMNT(addr)),
            crate::constants::MOE_ADDRESS => Some(Token::MOE(addr)),
            crate::constants::JOE_ADDRESS => Some(Token::JOE(addr)),
            _ => None,
        }
    }
}

/// Struct to hold reserves with token mapping for any DEX pool
#[derive(Debug, Clone, PartialEq)]
pub struct PoolReserves {
    pub token_a: Token,
    pub reserve_a: U256,
    pub token_b: Token,
    pub reserve_b: U256,
    pub block_number: u64,
    pub timestamp: DateTime<Utc>,
    pub pool_address: Address,
}

impl PoolReserves {
    /// Create new pool reserves
    pub fn new(
        token_a: Token,
        reserve_a: U256,
        token_b: Token,
        reserve_b: U256,
        block_number: u64,
        pool_address: Address,
    ) -> Self {
        Self {
            token_a,
            reserve_a,
            token_b,
            reserve_b,
            block_number,
            timestamp: Utc::now(),
            pool_address,
        }
    }

    /// Get reserves for a specific token pair (returns None if tokens don't match)
    pub fn get_reserves_for_pair(&self, token_in: Token, token_out: Token) -> Option<(U256, U256)> {
        if self.token_a == token_in && self.token_b == token_out {
            Some((self.reserve_a, self.reserve_b))
        } else if self.token_a == token_out && self.token_b == token_in {
            Some((self.reserve_b, self.reserve_a))
        } else {
            None
        }
    }
}

/// CSV record structure for arbitrage opportunities
#[derive(Debug, Serialize)]
pub struct ArbitrageRecord {
    pub timestamp: String,
    pub block_number: u64,
    pub optimal_input_wmnt: f64,
    pub final_output_wmnt: f64,
    pub gross_profit_wmnt: f64,
    pub net_profit_wmnt: f64,
    pub profit_percentage: f64,
    pub gas_cost_mnt: f64,
    pub search_method: String,
    pub moe_wmnt_reserve0: String,
    pub moe_wmnt_reserve1: String,
    pub joe_moe_reserve0: String,
    pub joe_moe_reserve1: String,
    pub joe_wmnt_reserve0: String,
    pub joe_wmnt_reserve1: String,
    pub fetch_time_ms: u64,
}

/// Arbitrage opportunity result
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub optimal_input: f64,
    pub final_output: f64,
    pub gross_profit: f64,
    pub net_profit: f64,
    pub profit_percentage: f64,
    pub search_method: String,
}

impl ArbitrageOpportunity {
    /// Check if the opportunity is profitable
    pub fn is_profitable(&self) -> bool {
        self.net_profit > 0.0
    }
}
