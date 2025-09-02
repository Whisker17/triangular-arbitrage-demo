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

/// Enhanced CSV record for multi-path arbitrage
#[derive(Debug, Serialize)]
pub struct MultiPathArbitrageRecord {
    pub timestamp: String,
    pub block_number: u64,
    pub optimal_input_wmnt: f64,
    pub final_output_wmnt: f64,
    pub gross_profit_wmnt: f64,
    pub net_profit_wmnt: f64,
    pub profit_percentage: f64,
    pub search_method: String,
    pub path_type: String,
    pub path_description: String,
    pub gas_units: u64,
    pub fetch_time_ms: u64,
    pub analysis_time_ms: u64,
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
    pub path: Option<ArbitragePath>,
}

impl ArbitrageOpportunity {
    /// Check if the opportunity is profitable
    pub fn is_profitable(&self) -> bool {
        self.net_profit > 0.0
    }

    /// Get the number of hops in the arbitrage path
    pub fn hop_count(&self) -> usize {
        self.path.as_ref()
            .map(|p| p.tokens.len().saturating_sub(1))
            .unwrap_or(0)
    }

    /// Get precise gas cost based on hop count and current gas price (result in MNT)
    pub fn gas_cost(&self, gas_price_gwei: f64) -> f64 {
        use crate::constants::{GAS_UNITS_3_HOPS, GAS_UNITS_4_HOPS, GWEI_TO_MNT_MULTIPLIER};
        
        let hop_count = self.hop_count();
        let gas_units = match hop_count {
            3 => GAS_UNITS_3_HOPS,
            4 => GAS_UNITS_4_HOPS,
            _ => GAS_UNITS_3_HOPS, // Default to 3-hops gas
        };
        
        // Direct calculation: gas_units * gas_price_gwei * gwei_to_mnt_multiplier
        gas_units as f64 * gas_price_gwei * GWEI_TO_MNT_MULTIPLIER
    }
}

/// Arbitrage path representation
#[derive(Debug, Clone, PartialEq)]
pub struct ArbitragePath {
    pub tokens: Vec<Token>,
    pub pools: Vec<Address>,
    pub path_type: PathType,
}

impl ArbitragePath {
    /// Create a new arbitrage path
    pub fn new(tokens: Vec<Token>, pools: Vec<Address>) -> Self {
        let path_type = match tokens.len() {
            3 => PathType::ThreeHop,
            4 => PathType::FourHop,
            _ => PathType::Custom(tokens.len()),
        };
        
        Self {
            tokens,
            pools,
            path_type,
        }
    }

    /// Get path description as string
    pub fn description(&self) -> String {
        self.tokens
            .iter()
            .map(|t| t.symbol())
            .collect::<Vec<_>>()
            .join(" -> ")
    }

    /// Check if path starts and ends with the same token (cycle)
    pub fn is_cycle(&self) -> bool {
        self.tokens.first() == self.tokens.last()
    }

    /// Get expected gas cost for this path type
    pub fn expected_gas_units(&self) -> u64 {
        use crate::constants::{GAS_UNITS_3_HOPS, GAS_UNITS_4_HOPS};
        
        match self.path_type {
            PathType::ThreeHop => GAS_UNITS_3_HOPS,
            PathType::FourHop => GAS_UNITS_4_HOPS,
            PathType::Custom(_) => GAS_UNITS_3_HOPS + (self.tokens.len() as u64 * 10_000),
        }
    }
}

/// Path type classification
#[derive(Debug, Clone, PartialEq)]
pub enum PathType {
    ThreeHop,
    FourHop,
    Custom(usize),
}

/// Multi-path arbitrage opportunity result
#[derive(Debug, Clone)]
pub struct MultiPathOpportunity {
    pub opportunities: Vec<ArbitrageOpportunity>,
    pub best_opportunity: Option<ArbitrageOpportunity>,
    pub total_profit: f64,
    pub analysis_time_ms: u64,
}

impl MultiPathOpportunity {
    /// Create new multi-path opportunity
    pub fn new(opportunities: Vec<ArbitrageOpportunity>, analysis_time_ms: u64) -> Self {
        let best_opportunity = opportunities
            .iter()
            .max_by(|a, b| a.net_profit.partial_cmp(&b.net_profit).unwrap_or(std::cmp::Ordering::Equal))
            .cloned();
        
        let total_profit = opportunities
            .iter()
            .filter(|opp| opp.is_profitable())
            .map(|opp| opp.net_profit)
            .sum();

        Self {
            opportunities,
            best_opportunity,
            total_profit,
            analysis_time_ms,
        }
    }

    /// Get profitable opportunities only
    pub fn profitable_opportunities(&self) -> Vec<&ArbitrageOpportunity> {
        self.opportunities
            .iter()
            .filter(|opp| opp.is_profitable())
            .collect()
    }

    /// Get number of profitable opportunities
    pub fn profitable_count(&self) -> usize {
        self.profitable_opportunities().len()
    }

    /// Check if any opportunity is profitable
    pub fn has_profitable_opportunities(&self) -> bool {
        self.profitable_count() > 0
    }
}
