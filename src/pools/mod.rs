//! Pool modules for different DEX protocols
//! 
//! This module provides abstractions and implementations for different
//! DEX protocols that can be used for arbitrage opportunities.

pub mod moe;

use alloy::primitives::Address;
use crate::types::Token;

/// Pool information structure
#[derive(Debug, Clone)]
pub struct PoolInfo {
    pub address: Address,
    pub name: String,
    pub token_a: Token,
    pub token_b: Token,
    pub dex_protocol: String,
    pub fee_tier: f64,
}

impl PoolInfo {
    pub fn new(
        address: Address,
        name: String,
        token_a: Token,
        token_b: Token,
        dex_protocol: String,
        fee_tier: f64,
    ) -> Self {
        Self {
            address,
            name,
            token_a,
            token_b,
            dex_protocol,
            fee_tier,
        }
    }

    /// Get a descriptive name for the pool
    pub fn get_display_name(&self) -> String {
        format!("{}-{} ({})", 
            self.token_a.symbol(), 
            self.token_b.symbol(), 
            self.dex_protocol
        )
    }

    /// Check if this pool contains both tokens
    pub fn contains_tokens(&self, token_a: Token, token_b: Token) -> bool {
        (self.token_a == token_a && self.token_b == token_b) ||
        (self.token_a == token_b && self.token_b == token_a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_info() {
        let wmnt_addr = Address::ZERO;
        let moe_addr = Address::from([1u8; 20]);
        
        let pool = PoolInfo::new(
            Address::from([2u8; 20]),
            "Test Pool".to_string(),
            Token::WMNT(wmnt_addr),
            Token::MOE(moe_addr),
            "TestDEX".to_string(),
            0.003,
        );

        assert_eq!(pool.get_display_name(), "WMNT-MOE (TestDEX)");
        assert!(pool.contains_tokens(Token::WMNT(wmnt_addr), Token::MOE(moe_addr)));
        assert!(pool.contains_tokens(Token::MOE(moe_addr), Token::WMNT(wmnt_addr)));
    }


}
