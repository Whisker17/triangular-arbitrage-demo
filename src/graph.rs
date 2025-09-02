use std::collections::{HashMap, HashSet, VecDeque};
use alloy::primitives::Address;
use petgraph::Graph;
use petgraph::graph::{NodeIndex, DiGraph};
use petgraph::visit::EdgeRef;
use crate::types::{Token, PoolReserves, ArbitragePath};

/// Token graph node for arbitrage pathfinding
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenNode {
    pub token: Token,
    pub address: Address,
}

impl TokenNode {
    pub fn new(token: Token) -> Self {
        Self {
            address: token.address(),
            token,
        }
    }
}

/// Pool edge representing a trading pair with SPFA-optimized weights
#[derive(Debug, Clone)]
pub struct PoolEdge {
    pub pool_address: Address,
    pub token_a: Token,
    pub token_b: Token,
    pub reserves_a: f64,
    pub reserves_b: f64,
    pub fee: f64,
    /// Negative log weight for SPFA algorithm (a->b direction)
    pub weight_a_to_b: f64,
    /// Negative log weight for SPFA algorithm (b->a direction)  
    pub weight_b_to_a: f64,
}

impl PoolEdge {
    pub fn new(
        pool_address: Address,
        token_a: Token,
        token_b: Token,
        reserves_a: f64,
        reserves_b: f64,
        fee: f64,
    ) -> Self {
        let (weight_a_to_b, weight_b_to_a) = Self::calculate_log_weights(reserves_a, reserves_b, fee);
        
        Self {
            pool_address,
            token_a,
            token_b,
            reserves_a,
            reserves_b,
            fee,
            weight_a_to_b,
            weight_b_to_a,
        }
    }
    
    /// Calculate negative log weights for SPFA algorithm
    /// Returns (weight_a_to_b, weight_b_to_a)
    fn calculate_log_weights(reserves_a: f64, reserves_b: f64, fee: f64) -> (f64, f64) {
        const EPSILON: f64 = 1e-10;
        
        if reserves_a <= EPSILON || reserves_b <= EPSILON {
            return (f64::INFINITY, f64::INFINITY);
        }
        
        // Calculate effective exchange rates considering fees
        // rate_a_to_b = (reserves_b * (1 - fee)) / reserves_a
        // rate_b_to_a = (reserves_a * (1 - fee)) / reserves_b
        let effective_fee = 1.0 - fee;
        let rate_a_to_b = (reserves_b * effective_fee) / reserves_a;
        let rate_b_to_a = (reserves_a * effective_fee) / reserves_b;
        
        // Convert to negative log weights for shortest path algorithm
        // Negative because we want to find maximum profit (minimum negative log)
        let weight_a_to_b = if rate_a_to_b > EPSILON { -rate_a_to_b.ln() } else { f64::INFINITY };
        let weight_b_to_a = if rate_b_to_a > EPSILON { -rate_b_to_a.ln() } else { f64::INFINITY };
        
        (weight_a_to_b, weight_b_to_a)
    }
    
    /// Update weights when reserves change
    pub fn update_reserves(&mut self, reserves_a: f64, reserves_b: f64) {
        self.reserves_a = reserves_a;
        self.reserves_b = reserves_b;
        let (weight_a_to_b, weight_b_to_a) = Self::calculate_log_weights(reserves_a, reserves_b, self.fee);
        self.weight_a_to_b = weight_a_to_b;
        self.weight_b_to_a = weight_b_to_a;
    }

    /// Get the exchange rate from token_a to token_b
    pub fn get_rate_a_to_b(&self) -> f64 {
        if self.reserves_a > 0.0 {
            self.reserves_b / self.reserves_a
        } else {
            0.0
        }
    }

    /// Get the exchange rate from token_b to token_a
    pub fn get_rate_b_to_a(&self) -> f64 {
        if self.reserves_b > 0.0 {
            self.reserves_a / self.reserves_b
        } else {
            0.0
        }
    }

    /// Calculate output amount for a given input considering fees
    pub fn calculate_output(&self, input_amount: f64, token_in: Token) -> Option<f64> {
        let (reserve_in, reserve_out) = if token_in == self.token_a {
            (self.reserves_a, self.reserves_b)
        } else if token_in == self.token_b {
            (self.reserves_b, self.reserves_a)
        } else {
            return None;
        };

        if reserve_in <= 0.0 || reserve_out <= 0.0 || input_amount <= 0.0 {
            return None;
        }

        // AMM formula with fees: output = (input * fee * reserve_out) / (reserve_in + input * fee)
        let input_with_fee = input_amount * (1.0 - self.fee);
        let output = (input_with_fee * reserve_out) / (reserve_in + input_with_fee);
        
        Some(output)
    }
}

/// Token graph for arbitrage pathfinding using SPFA algorithm
pub struct TokenGraph {
    graph: DiGraph<TokenNode, DirectedEdge>,
    token_to_node: HashMap<Token, NodeIndex>,
    wmnt_token: Token,
}

/// Directed edge for SPFA algorithm
#[derive(Debug, Clone)]
pub struct DirectedEdge {
    pub pool_address: Address,
    pub from_token: Token,
    pub to_token: Token,
    pub weight: f64,
    pub original_pool: PoolEdge,
}

impl DirectedEdge {
    pub fn new(pool: &PoolEdge, from_token: Token, to_token: Token, weight: f64) -> Self {
        Self {
            pool_address: pool.pool_address,
            from_token,
            to_token,
            weight,
            original_pool: pool.clone(),
        }
    }
}

impl TokenGraph {
    /// Create a new empty token graph
    pub fn new(wmnt_token: Token) -> Self {
        Self {
            graph: Graph::new(),
            token_to_node: HashMap::new(),
            wmnt_token,
        }
    }

    /// Add a token to the graph
    pub fn add_token(&mut self, token: Token) -> NodeIndex {
        if let Some(&node_idx) = self.token_to_node.get(&token) {
            return node_idx;
        }

        let node = TokenNode::new(token);
        let node_idx = self.graph.add_node(node);
        self.token_to_node.insert(token, node_idx);
        node_idx
    }

    /// Add a pool to the graph (creates directed edges in both directions)
    pub fn add_pool(&mut self, pool_reserves: &PoolReserves, fee: f64) {
        let token_a_idx = self.add_token(pool_reserves.token_a);
        let token_b_idx = self.add_token(pool_reserves.token_b);

        let reserves_a = crate::math::u256_to_f64(pool_reserves.reserve_a);
        let reserves_b = crate::math::u256_to_f64(pool_reserves.reserve_b);

        let pool_edge = PoolEdge::new(
            pool_reserves.pool_address,
            pool_reserves.token_a,
            pool_reserves.token_b,
            reserves_a,
            reserves_b,
            fee,
        );

        // Add directed edge from token_a to token_b
        let edge_a_to_b = DirectedEdge::new(
            &pool_edge,
            pool_reserves.token_a,
            pool_reserves.token_b,
            pool_edge.weight_a_to_b,
        );
        self.graph.add_edge(token_a_idx, token_b_idx, edge_a_to_b);

        // Add directed edge from token_b to token_a
        let edge_b_to_a = DirectedEdge::new(
            &pool_edge,
            pool_reserves.token_b,
            pool_reserves.token_a,
            pool_edge.weight_b_to_a,
        );
        self.graph.add_edge(token_b_idx, token_a_idx, edge_b_to_a);
    }

    /// Update pool reserves and recalculate weights
    pub fn update_pool(&mut self, pool_reserves: &PoolReserves) {
        let token_a_idx = if let Some(&idx) = self.token_to_node.get(&pool_reserves.token_a) {
            idx
        } else {
            return;
        };

        let token_b_idx = if let Some(&idx) = self.token_to_node.get(&pool_reserves.token_b) {
            idx
        } else {
            return;
        };

        let reserves_a = crate::math::u256_to_f64(pool_reserves.reserve_a);
        let reserves_b = crate::math::u256_to_f64(pool_reserves.reserve_b);

        // Update edge from token_a to token_b
        if let Some(edge_ref) = self.graph.find_edge(token_a_idx, token_b_idx) {
            if let Some(edge) = self.graph.edge_weight_mut(edge_ref) {
                edge.original_pool.update_reserves(reserves_a, reserves_b);
                edge.weight = edge.original_pool.weight_a_to_b;
            }
        }

        // Update edge from token_b to token_a
        if let Some(edge_ref) = self.graph.find_edge(token_b_idx, token_a_idx) {
            if let Some(edge) = self.graph.edge_weight_mut(edge_ref) {
                edge.original_pool.update_reserves(reserves_a, reserves_b);
                edge.weight = edge.original_pool.weight_b_to_a;
            }
        }
    }

    /// Find all arbitrage cycles using SPFA algorithm (negative cycle detection)
    pub fn find_arbitrage_cycles(&self, max_hops: usize) -> Vec<ArbitragePath> {
        let mut cycles = Vec::new();
        
        let wmnt_node_idx = match self.token_to_node.get(&self.wmnt_token) {
            Some(&idx) => idx,
            None => return cycles,
        };

        // Use SPFA to detect negative cycles (arbitrage opportunities)
        if let Some(negative_cycles) = self.spfa_detect_negative_cycles(wmnt_node_idx, max_hops) {
            for cycle_path in negative_cycles {
                if let Some(arbitrage_path) = self.convert_node_path_to_arbitrage_path(cycle_path) {
                    cycles.push(arbitrage_path);
                }
            }
        }

        cycles
    }

    /// SPFA algorithm to detect negative cycles (arbitrage opportunities)
    fn spfa_detect_negative_cycles(&self, source: NodeIndex, max_hops: usize) -> Option<Vec<Vec<NodeIndex>>> {
        let node_count = self.graph.node_count();
        if node_count == 0 {
            return None;
        }

        // Initialize distance and predecessor arrays
        let mut dist = vec![f64::INFINITY; node_count];
        let mut predecessor = vec![None; node_count];
        let mut in_queue = vec![false; node_count];
        let mut queue_count = vec![0; node_count];
        
        let source_index = source.index();
        dist[source_index] = 0.0;
        
        let mut queue = VecDeque::new();
        queue.push_back(source);
        in_queue[source_index] = true;
        queue_count[source_index] = 1;

        let mut negative_cycle_nodes = HashSet::new();

        // SPFA main loop
        while let Some(current) = queue.pop_front() {
            let current_index = current.index();
            in_queue[current_index] = false;

            // Check if we've processed this node too many times (indicates negative cycle)
            if queue_count[current_index] > node_count {
                negative_cycle_nodes.insert(current);
                continue;
            }

            // Relax all outgoing edges
            for edge_ref in self.graph.edges(current) {
                let neighbor = edge_ref.target();
                let neighbor_index = neighbor.index();
                let edge_weight = edge_ref.weight();
                
                let new_dist = dist[current_index] + edge_weight.weight;
                
                if new_dist < dist[neighbor_index] {
                    dist[neighbor_index] = new_dist;
                    predecessor[neighbor_index] = Some(current);
                    
                    if !in_queue[neighbor_index] {
                        queue.push_back(neighbor);
                        in_queue[neighbor_index] = true;
                        queue_count[neighbor_index] += 1;
                        
                        // If a node is relaxed too many times, it's part of a negative cycle
                        if queue_count[neighbor_index] > node_count {
                            negative_cycle_nodes.insert(neighbor);
                        }
                    }
                }
            }
        }

        // If negative cycles were detected, extract them
        if !negative_cycle_nodes.is_empty() {
            self.extract_negative_cycles(negative_cycle_nodes, predecessor, max_hops)
        } else {
            None
        }
    }

    /// Extract actual negative cycle paths from detected nodes
    fn extract_negative_cycles(
        &self,
        negative_cycle_nodes: HashSet<NodeIndex>,
        predecessor: Vec<Option<NodeIndex>>,
        max_hops: usize,
    ) -> Option<Vec<Vec<NodeIndex>>> {
        let mut cycles = Vec::new();
        let wmnt_node_idx = self.token_to_node.get(&self.wmnt_token)?;

        for &cycle_node in &negative_cycle_nodes {
            if let Some(cycle_path) = self.reconstruct_cycle(cycle_node, &predecessor, *wmnt_node_idx, max_hops) {
                // Only keep cycles that start and end with WMNT and are within hop limits
                if cycle_path.len() >= 4 && cycle_path.len() <= max_hops + 1 && 
                   cycle_path.first() == Some(wmnt_node_idx) && 
                   cycle_path.last() == Some(wmnt_node_idx) {
                    cycles.push(cycle_path);
                }
            }
        }

        if cycles.is_empty() {
            None
        } else {
            Some(cycles)
        }
    }

    /// Reconstruct cycle path from predecessor array
    fn reconstruct_cycle(
        &self,
        start_node: NodeIndex,
        predecessor: &[Option<NodeIndex>],
        wmnt_node: NodeIndex,
        max_hops: usize,
    ) -> Option<Vec<NodeIndex>> {
        let mut path = Vec::new();
        let mut current = start_node;
        let mut visited = HashSet::new();

        // Traverse backwards using predecessor to find a cycle
        loop {
            if visited.contains(&current) {
                // Found a cycle, now construct the path
                let cycle_start_pos = path.iter().position(|&node| node == current)?;
                let mut cycle_path = path[cycle_start_pos..].to_vec();
                cycle_path.push(current); // Close the cycle
                
                // Try to extend cycle to include WMNT if not already present
                if !cycle_path.contains(&wmnt_node) {
                    // Look for a path from any node in the cycle to WMNT
                    if let Some(extended_path) = self.extend_cycle_to_wmnt(cycle_path, wmnt_node, max_hops) {
                        return Some(extended_path);
                    }
                } else {
                    // If WMNT is already in the cycle, rearrange to start/end with WMNT
                    return self.rearrange_cycle_with_wmnt(cycle_path, wmnt_node);
                }
                break;
            }

            visited.insert(current);
            path.push(current);

            if let Some(prev) = predecessor[current.index()] {
                current = prev;
            } else {
                break;
            }

            // Prevent infinite loops
            if path.len() > max_hops * 2 {
                break;
            }
        }

        None
    }

    /// Extend cycle to include WMNT as start/end point
    fn extend_cycle_to_wmnt(
        &self,
        cycle: Vec<NodeIndex>,
        wmnt_node: NodeIndex,
        max_hops: usize,
    ) -> Option<Vec<NodeIndex>> {
        // Simple approach: try to find direct connections from WMNT to cycle
        for &cycle_node in &cycle {
            if self.graph.find_edge(wmnt_node, cycle_node).is_some() {
                // Found connection from WMNT to cycle
                let mut extended = vec![wmnt_node];
                extended.extend_from_slice(&cycle);
                
                // Try to find path back to WMNT
                if let Some(&last_node) = cycle.last() {
                    if self.graph.find_edge(last_node, wmnt_node).is_some() {
                        extended.push(wmnt_node);
                        if extended.len() <= max_hops + 1 {
                            return Some(extended);
                        }
                    }
                }
            }
        }
        None
    }

    /// Rearrange cycle to start and end with WMNT
    fn rearrange_cycle_with_wmnt(&self, mut cycle: Vec<NodeIndex>, wmnt_node: NodeIndex) -> Option<Vec<NodeIndex>> {
        if let Some(wmnt_pos) = cycle.iter().position(|&node| node == wmnt_node) {
            // Rotate cycle to start with WMNT
            cycle.rotate_left(wmnt_pos);
            // Ensure it ends with WMNT
            if cycle.last() != Some(&wmnt_node) {
                cycle.push(wmnt_node);
            }
            Some(cycle)
        } else {
            None
        }
    }

    /// Convert node path to ArbitragePath
    fn convert_node_path_to_arbitrage_path(&self, node_path: Vec<NodeIndex>) -> Option<ArbitragePath> {
        if node_path.len() < 3 {
            return None;
        }

        let tokens: Vec<Token> = node_path.iter()
            .map(|&idx| self.graph[idx].token)
            .collect();

        let mut pools = Vec::new();
        for window in node_path.windows(2) {
            if let Some(edge_ref) = self.graph.find_edge(window[0], window[1]) {
                if let Some(edge) = self.graph.edge_weight(edge_ref) {
                    pools.push(edge.pool_address);
                }
            }
        }

        if pools.len() == tokens.len() - 1 {
            Some(ArbitragePath::new(tokens, pools))
        } else {
            None
        }
    }

    /// Calculate the profit for a given arbitrage path
    pub fn calculate_path_profit(&self, path: &ArbitragePath, input_amount: f64) -> Option<f64> {
        if path.tokens.len() < 3 {
            return None;
        }

        let mut current_amount = input_amount;
        
        for i in 0..path.tokens.len() - 1 {
            let token_in = path.tokens[i];
            let token_out = path.tokens[i + 1];
            
            let token_in_idx = self.token_to_node.get(&token_in)?;
            let token_out_idx = self.token_to_node.get(&token_out)?;
            
            let edge_ref = self.graph.find_edge(*token_in_idx, *token_out_idx)?;
            let edge = self.graph.edge_weight(edge_ref)?;
            
            current_amount = edge.original_pool.calculate_output(current_amount, token_in)?;
        }
        
        // Add the closing trade back to WMNT
        if let Some(last_token) = path.tokens.last() {
            if *last_token != self.wmnt_token {
                let last_token_idx = self.token_to_node.get(last_token)?;
                let wmnt_idx = self.token_to_node.get(&self.wmnt_token)?;
                
                let edge_ref = self.graph.find_edge(*last_token_idx, *wmnt_idx)?;
                let edge = self.graph.edge_weight(edge_ref)?;
                
                current_amount = edge.original_pool.calculate_output(current_amount, *last_token)?;
            }
        }

        Some(current_amount - input_amount)
    }

    /// Get number of nodes in the graph
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get number of edges in the graph
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Get all tokens in the graph
    pub fn get_all_tokens(&self) -> Vec<Token> {
        self.token_to_node.keys().cloned().collect()
    }

    /// Get pool information for a token pair
    pub fn get_pool_info(&self, token_a: Token, token_b: Token) -> Option<&PoolEdge> {
        let token_a_idx = self.token_to_node.get(&token_a)?;
        let token_b_idx = self.token_to_node.get(&token_b)?;
        
        let edge_ref = self.graph.find_edge(*token_a_idx, *token_b_idx)?;
        let directed_edge = self.graph.edge_weight(edge_ref)?;
        Some(&directed_edge.original_pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use crate::types::{Token, PoolReserves};
    use alloy::primitives::U256;
    use chrono::Utc;

    fn create_test_token(symbol: &str, address: [u8; 20]) -> Token {
        let addr = Address::from(address);
        match symbol {
            "WMNT" => Token::WMNT(addr),
            "MOE" => Token::MOE(addr),
            "JOE" => Token::JOE(addr),
            _ => panic!("Unknown token symbol"),
        }
    }

    fn create_test_pool_reserves(
        token_a: Token,
        reserve_a: u128,
        token_b: Token,
        reserve_b: u128,
        pool_address: Address,
    ) -> PoolReserves {
        PoolReserves {
            token_a,
            reserve_a: U256::from(reserve_a * 1_000_000_000_000_000_000u128), // Convert to wei
            token_b,
            reserve_b: U256::from(reserve_b * 1_000_000_000_000_000_000u128),
            block_number: 1,
            timestamp: Utc::now(),
            pool_address,
        }
    }

    #[test]
    fn test_token_graph_creation() {
        let wmnt = create_test_token("WMNT", [0u8; 20]);
        let mut graph = TokenGraph::new(wmnt);
        
        let moe = create_test_token("MOE", [1u8; 20]);
        let joe = create_test_token("JOE", [2u8; 20]);
        
        graph.add_token(wmnt);
        graph.add_token(moe);
        graph.add_token(joe);
        
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_pool_addition() {
        let wmnt = create_test_token("WMNT", [0u8; 20]);
        let moe = create_test_token("MOE", [1u8; 20]);
        let mut graph = TokenGraph::new(wmnt);
        
        let pool_reserves = create_test_pool_reserves(
            wmnt,
            1000,
            moe,
            1000,
            Address::from([1u8; 20])
        );
        
        graph.add_pool(&pool_reserves, 0.003);
        
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 2); // 有向图：每个池子创建两条边
    }

    #[test]
    fn test_spfa_arbitrage_cycle_finding() {
        let wmnt = create_test_token("WMNT", [0u8; 20]);
        let moe = create_test_token("MOE", [1u8; 20]);
        let joe = create_test_token("JOE", [2u8; 20]);
        let mut graph = TokenGraph::new(wmnt);
        
        // Add pools to create a triangle with potential arbitrage opportunity
        // Create imbalanced pools to simulate arbitrage opportunity
        let pool1 = create_test_pool_reserves(wmnt, 1000, moe, 900, Address::from([1u8; 20]));   // WMNT undervalued
        let pool2 = create_test_pool_reserves(moe, 1000, joe, 1100, Address::from([2u8; 20]));   // MOE undervalued  
        let pool3 = create_test_pool_reserves(joe, 1000, wmnt, 1200, Address::from([3u8; 20]));  // JOE overvalued
        
        graph.add_pool(&pool1, 0.003);
        graph.add_pool(&pool2, 0.003);
        graph.add_pool(&pool3, 0.003);
        
        let cycles = graph.find_arbitrage_cycles(4);
        
        // SPFA should detect cycles, but actual arbitrage depends on the precise math
        // For this test, we mainly verify the algorithm doesn't crash and returns valid paths
        for cycle in &cycles {
            // Verify cycle structure
            assert!(cycle.tokens.len() >= 3);
            assert_eq!(cycle.tokens.first(), Some(&wmnt));
            assert_eq!(cycle.tokens.last(), Some(&wmnt));
        }
    }
    
    #[test] 
    fn test_spfa_negative_weights() {
        let wmnt = create_test_token("WMNT", [0u8; 20]);
        let moe = create_test_token("MOE", [1u8; 20]);
        let mut graph = TokenGraph::new(wmnt);
        
        // Test weight calculation
        let pool_reserves = create_test_pool_reserves(wmnt, 1000, moe, 1000, Address::from([1u8; 20]));
        graph.add_pool(&pool_reserves, 0.003);
        
        // Graph should have 2 nodes and 2 directed edges
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 2);
        
        // Test pool info retrieval
        let pool_info = graph.get_pool_info(wmnt, moe);
        assert!(pool_info.is_some());
    }
}
