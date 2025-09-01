//! MOE DEX protocol implementation

use alloy::primitives::Address;
use crate::types::Token;
use crate::constants::*;

/// MOE DEX protocol implementation
pub struct MoeProtocol {
    name: String,
    default_fee: f64,
    known_pools: Vec<(Address, String, Token, Token)>,
}

impl MoeProtocol {
    /// Create a new MOE protocol instance
    pub fn new() -> Self {
        let wmnt_addr: Address = WMNT_ADDRESS.parse().expect("Invalid WMNT address");
        let moe_addr: Address = MOE_ADDRESS.parse().expect("Invalid MOE address");
        let joe_addr: Address = JOE_ADDRESS.parse().expect("Invalid JOE address");

        let known_pools = vec![
            (
                MOE_WMNT_POOL.parse().expect("Invalid MOE-WMNT pool address"),
                "MOE-WMNT".to_string(),
                Token::MOE(moe_addr),
                Token::WMNT(wmnt_addr),
            ),
            (
                JOE_MOE_POOL.parse().expect("Invalid JOE-MOE pool address"),
                "JOE-MOE".to_string(),
                Token::JOE(joe_addr),
                Token::MOE(moe_addr),
            ),
            (
                JOE_WMNT_POOL.parse().expect("Invalid JOE-WMNT pool address"),
                "JOE-WMNT".to_string(),
                Token::JOE(joe_addr),
                Token::WMNT(wmnt_addr),
            ),
        ];

        Self {
            name: "MOE".to_string(),
            default_fee: DEFAULT_DEX_FEE,
            known_pools,
        }
    }

    /// Get the pool addresses as a vector
    pub fn get_pool_addresses(&self) -> Vec<Address> {
        self.known_pools.iter().map(|(addr, _, _, _)| *addr).collect()
    }

    /// Get pool info by address
    pub fn get_pool_info(&self, address: Address) -> Option<&(Address, String, Token, Token)> {
        self.known_pools.iter().find(|(addr, _, _, _)| *addr == address)
    }

    /// Get all triangular arbitrage paths available in this protocol
    pub fn get_triangular_paths(&self) -> Vec<Vec<Token>> {
        // For now, we have one main triangular path: WMNT -> MOE -> JOE -> WMNT
        let wmnt_addr: Address = WMNT_ADDRESS.parse().expect("Invalid WMNT address");
        let moe_addr: Address = MOE_ADDRESS.parse().expect("Invalid MOE address");
        let joe_addr: Address = JOE_ADDRESS.parse().expect("Invalid JOE address");

        vec![vec![
            Token::WMNT(wmnt_addr),
            Token::MOE(moe_addr),
            Token::JOE(joe_addr),
            Token::WMNT(wmnt_addr), // Complete the cycle
        ]]
    }

    /// Check if an address is a known MOE pool
    pub fn is_moe_pool(&self, address: Address) -> bool {
        self.known_pools.iter().any(|(addr, _, _, _)| *addr == address)
    }

    /// Get the protocol name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the default fee for this DEX
    pub fn default_fee(&self) -> f64 {
        self.default_fee
    }

    /// Get all known pool addresses for this DEX
    pub fn get_known_pools(&self) -> Vec<(Address, String, Token, Token)> {
        self.known_pools.clone()
    }

    /// Validate if a pool address belongs to this DEX
    pub fn is_valid_pool(&self, pool_address: Address) -> bool {
        self.is_moe_pool(pool_address)
    }
}



impl Default for MoeProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for MOE protocol specific operations
impl MoeProtocol {
    /// Get the MOE-WMNT pool address
    pub fn moe_wmnt_pool(&self) -> Address {
        MOE_WMNT_POOL.parse().expect("Invalid MOE-WMNT pool address")
    }

    /// Get the JOE-MOE pool address
    pub fn joe_moe_pool(&self) -> Address {
        JOE_MOE_POOL.parse().expect("Invalid JOE-MOE pool address")
    }

    /// Get the JOE-WMNT pool address
    pub fn joe_wmnt_pool(&self) -> Address {
        JOE_WMNT_POOL.parse().expect("Invalid JOE-WMNT pool address")
    }

    /// Get all pool addresses for the main triangular arbitrage
    pub fn get_main_triangular_pools(&self) -> (Address, Address, Address) {
        (
            self.moe_wmnt_pool(),
            self.joe_moe_pool(),
            self.joe_wmnt_pool(),
        )
    }

    /// Validate that we have all required pools for triangular arbitrage
    pub fn validate_triangular_setup(&self) -> Result<(), String> {
        let (moe_wmnt, joe_moe, joe_wmnt) = self.get_main_triangular_pools();
        
        if !self.is_valid_pool(moe_wmnt) {
            return Err("MOE-WMNT pool not found".to_string());
        }
        if !self.is_valid_pool(joe_moe) {
            return Err("JOE-MOE pool not found".to_string());
        }
        if !self.is_valid_pool(joe_wmnt) {
            return Err("JOE-WMNT pool not found".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moe_protocol_creation() {
        let protocol = MoeProtocol::new();
        assert_eq!(protocol.name(), "MOE");
        assert_eq!(protocol.default_fee(), DEFAULT_DEX_FEE);
        assert_eq!(protocol.get_known_pools().len(), 3);
    }

    #[test]
    fn test_pool_addresses() {
        let protocol = MoeProtocol::new();
        let addresses = protocol.get_pool_addresses();
        assert_eq!(addresses.len(), 3);
        
        let (moe_wmnt, joe_moe, joe_wmnt) = protocol.get_main_triangular_pools();
        assert!(addresses.contains(&moe_wmnt));
        assert!(addresses.contains(&joe_moe));
        assert!(addresses.contains(&joe_wmnt));
    }

    #[test]
    fn test_triangular_paths() {
        let protocol = MoeProtocol::new();
        let paths = protocol.get_triangular_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 4); // WMNT -> MOE -> JOE -> WMNT
    }

    #[test]
    fn test_validate_triangular_setup() {
        let protocol = MoeProtocol::new();
        let result = protocol.validate_triangular_setup();
        assert!(result.is_ok());
    }

    #[test]
    fn test_pool_validation() {
        let protocol = MoeProtocol::new();
        let moe_wmnt = protocol.moe_wmnt_pool();
        assert!(protocol.is_valid_pool(moe_wmnt));
        
        // Test with invalid address
        let invalid_addr = Address::ZERO;
        assert!(!protocol.is_valid_pool(invalid_addr));
    }
}
