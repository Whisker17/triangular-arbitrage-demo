use std::collections::HashMap;
use alloy::primitives::Address;
use crate::types::{Token, PoolReserves};
use crate::math::u256_to_f64;

/// Format pool reserves information for logging
pub fn format_pool_reserves(
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

/// Format a single pool's reserves in a readable way
pub fn format_single_pool_reserves(reserves: &PoolReserves) -> String {
    let token_a_symbol = reserves.token_a.symbol();
    let token_b_symbol = reserves.token_b.symbol();
    let reserve_a = u256_to_f64(reserves.reserve_a);
    let reserve_b = u256_to_f64(reserves.reserve_b);
    
    format!("{:.2} {} / {:.2} {}", reserve_a, token_a_symbol, reserve_b, token_b_symbol)
}

/// Format startup banner with configuration info
pub fn print_startup_banner() {
    println!("🚀 Starting triangular arbitrage monitor on Mantle Network");
    println!("📊 Monitoring pools: MOE-WMNT, JOE-MOE, JOE-WMNT");
}

/// Format pool addresses for display
pub fn format_pool_addresses(
    moe_wmnt_addr: Address,
    joe_moe_addr: Address,
    joe_wmnt_addr: Address,
) -> String {
    format!(
        "Pool Addresses:\n   MOE-WMNT: {}\n   JOE-MOE: {}\n   JOE-WMNT: {}",
        moe_wmnt_addr, joe_moe_addr, joe_wmnt_addr
    )
}

/// Format arbitrage path for display
pub fn format_arbitrage_path(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|t| t.symbol())
        .collect::<Vec<_>>()
        .join(" → ")
}

/// Format token amount with symbol
pub fn format_token_amount(amount: f64, token: &Token) -> String {
    format!("{:.6} {}", amount, token.symbol())
}

/// Format percentage with color coding (for future terminal color support)
pub fn format_percentage(percentage: f64) -> String {
    if percentage > 0.0 {
        format!("+{:.2}%", percentage)
    } else {
        format!("{:.2}%", percentage)
    }
}

/// Format duration in human-readable format
pub fn format_duration(duration: std::time::Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1000 {
        format!("{}ms", millis)
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}

/// Format block info
pub fn format_block_info(block_number: u64, timestamp: chrono::DateTime<chrono::Utc>) -> String {
    format!("Block {} ({})", block_number, timestamp.format("%H:%M:%S%.3f"))
}

/// Format error message with emoji
pub fn format_error(message: &str) -> String {
    format!("❌ {}", message)
}

/// Format success message with emoji
pub fn format_success(message: &str) -> String {
    format!("✅ {}", message)
}

/// Format warning message with emoji
pub fn format_warning(message: &str) -> String {
    format!("⚠️ {}", message)
}

/// Format info message with emoji
pub fn format_info(message: &str) -> String {
    format!("ℹ️ {}", message)
}

/// Generic formatter trait for extensibility
pub trait ReservesFormatter {
    fn format_reserves(&self, reserves_map: &HashMap<Address, PoolReserves>) -> String;
    fn format_pool(&self, address: Address, reserves: &PoolReserves) -> String;
}

/// Default reserves formatter
pub struct DefaultReservesFormatter {
    pub known_pools: Vec<(Address, String)>, // (address, name) pairs
}

impl DefaultReservesFormatter {
    pub fn new() -> Self {
        Self {
            known_pools: vec![],
        }
    }

    pub fn with_pools(pools: Vec<(Address, String)>) -> Self {
        Self {
            known_pools: pools,
        }
    }
}

impl ReservesFormatter for DefaultReservesFormatter {
    fn format_reserves(&self, reserves_map: &HashMap<Address, PoolReserves>) -> String {
        let mut output = String::new();
        
        for (address, reserves) in reserves_map {
            let pool_name = self.known_pools
                .iter()
                .find(|(addr, _)| addr == address)
                .map(|(_, name)| name.as_str())
                .unwrap_or("Unknown Pool");
            
            output.push_str(&format!("   📊 {}: {}\n", 
                pool_name, 
                format_single_pool_reserves(reserves)
            ));
        }
        
        // Remove trailing newline
        if output.ends_with('\n') {
            output.pop();
        }
        
        output
    }

    fn format_pool(&self, address: Address, reserves: &PoolReserves) -> String {
        let pool_name = self.known_pools
            .iter()
            .find(|(addr, _)| addr == &address)
            .map(|(_, name)| name.as_str())
            .unwrap_or("Unknown Pool");
        
        format!("📊 {}: {}", pool_name, format_single_pool_reserves(reserves))
    }
}

impl Default for DefaultReservesFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;
    use chrono::Utc;

    fn create_test_reserves() -> PoolReserves {
        PoolReserves {
            token_a: Token::WMNT(Address::ZERO),
            reserve_a: U256::from(1000u128 * 1_000_000_000_000_000_000u128),
            token_b: Token::MOE(Address::from([1u8; 20])),
            reserve_b: U256::from(2000u128 * 1_000_000_000_000_000_000u128),
            block_number: 1,
            timestamp: Utc::now(),
            pool_address: Address::ZERO,
        }
    }

    #[test]
    fn test_format_single_pool_reserves() {
        let reserves = create_test_reserves();
        let formatted = format_single_pool_reserves(&reserves);
        assert!(formatted.contains("1000.00 WMNT"));
        assert!(formatted.contains("2000.00 MOE"));
    }

    #[test]
    fn test_format_arbitrage_path() {
        let tokens = vec![
            Token::WMNT(Address::ZERO),
            Token::MOE(Address::from([1u8; 20])),
            Token::JOE(Address::from([2u8; 20])),
        ];
        let formatted = format_arbitrage_path(&tokens);
        assert_eq!(formatted, "WMNT → MOE → JOE");
    }

    #[test]
    fn test_format_percentage() {
        assert_eq!(format_percentage(5.25), "+5.25%");
        assert_eq!(format_percentage(-2.1), "-2.10%");
        assert_eq!(format_percentage(0.0), "0.00%");
    }

    #[test]
    fn test_default_reserves_formatter() {
        let formatter = DefaultReservesFormatter::new();
        let mut reserves_map = HashMap::new();
        let reserves = create_test_reserves();
        reserves_map.insert(Address::ZERO, reserves);
        
        let formatted = formatter.format_reserves(&reserves_map);
        assert!(formatted.contains("Unknown Pool"));
        assert!(formatted.contains("1000.00 WMNT"));
    }
}
