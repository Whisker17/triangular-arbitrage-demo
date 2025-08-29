# Triangular Arbitrage Monitor

## 🚀 Optimized Features

This is an efficient triangular arbitrage monitoring service for Mantle Network with the following key optimizations:

### 💰 Transaction Cost Integration
- ✅ Integrated 0.02 MNT transaction cost estimation
- ✅ Reports arbitrage opportunities only when net profit is positive
- ✅ Real-time profitability calculation excluding transaction fees

### ⚡ Block-Based Smart Monitoring
- ✅ Optimized polling strategy based on Mantle's 2-second block time
- ✅ Queries pool states only when new blocks are produced
- ✅ Avoids unnecessary API calls, reducing resource consumption

### 🔄 High-Efficiency Data Retrieval
- ✅ **Parallel fetching** of reserves data from three pools
- ✅ **Caching mechanism** that only recalculates when data changes
- ✅ **Retry mechanism** for improved network fault tolerance

### 🛡️ Enhanced Error Handling
- ✅ Up to 3 retries for improved stability
- ✅ Graceful error recovery mechanisms
- ✅ Detailed error logging

### 📊 Smart Logging System
- ✅ **Smart triggering**: Prints logs only when reserves actually change, avoiding invalid output
- ✅ **Detailed reserves info**: Shows real-time reserve status of three pools
- ✅ **Real-time logs with timestamps**: Time recording accurate to milliseconds
- ✅ **Colored emoji indicators**: Intuitive distinction for different event types
- ✅ **Performance metrics**: Data retrieval and analysis time statistics

### 📝 CSV Data Recording
- ✅ **Automatic CSV recording**: All arbitrage opportunities automatically saved to CSV files
- ✅ **Detailed fields**: Timestamps, block numbers, profit rates, pool reserves, etc.
- ✅ **Structured data**: Convenient for subsequent analysis and visualization

### 🎯 Ternary Search Algorithm Optimization
- ✅ **Mathematical optimal solution**: Uses ternary search algorithm to find mathematically optimal input amounts
- ✅ **Constant product formula**: Precise AMM swap calculation (x*y=k)
- ✅ **100 iterations**: High-precision search ensures finding the true optimal point
- ✅ **Automatic range control**: Search range up to 99.9% of pool reserves

## 📈 Monitored Arbitrage Path

**WMNT → MOE → JOE → WMNT**

Monitoring the following three pools:
- `MOE-WMNT Pool`: `0x763868612858358f62b05691dB82Ad35a9b3E110`
- `JOE-MOE Pool`: `0xb670D2B452D0Ecc468cccFD532482d45dDdDe2a1`
- `JOE-WMNT Pool`: `0xEFC38C1B0d60725B824EBeE8D431aBFBF12BC953`

## 🔧 Setup and Usage

### Environment Configuration

**Required**: Set the RPC_URL environment variable:

```bash
# Option 1: Set environment variable directly
export RPC_URL=https://rpc.mantle.xyz

# Option 2: Create a .env file
echo "RPC_URL=https://rpc.mantle.xyz" > .env

# Option 3: Use a custom RPC endpoint
export RPC_URL=https://rpc-moon.mantle.xyz/v1/YOUR_API_KEY
```

**Recommended**: Create a `.env` file in the project root:

```env
# Mantle Network Configuration
# Copy this content to a .env file and configure your settings

# Required: Mantle RPC URL
RPC_URL=https://rpc.mantle.xyz

# Alternative RPC endpoints:
# RPC_URL=https://rpc-moon.mantle.xyz/v1/YOUR_API_KEY
# RPC_URL=https://mantle-mainnet.public.blastapi.io

# Optional: Override default configuration
# TRANSACTION_COST_MNT=0.02
# DEX_FEE=0.003
# TERNARY_SEARCH_ITERATIONS=100
# BLOCK_TIME_SECONDS=2
# MAX_RETRIES=3
# CSV_FILE_PATH=arbitrage_opportunities.csv
```

### Build and Run

```bash
# Build the project
cargo build --release

# Run the monitoring service
cargo run --release
```

## 📋 Configuration Parameters

```rust
// Default values (can be overridden by environment variables)
const DEFAULT_TRANSACTION_COST_MNT: f64 = 0.02;    // Transaction cost
const DEFAULT_BLOCK_TIME_SECONDS: u64 = 2;         // Mantle block time
const DEFAULT_MAX_RETRIES: u32 = 3;                // Maximum retries
const DEFAULT_CSV_FILE_PATH: &str = "arbitrage_opportunities.csv";  // CSV file path
const DEFAULT_DEX_FEE: f64 = 0.003;                // DEX trading fee (0.3%)
const DEFAULT_TERNARY_SEARCH_ITERATIONS: usize = 100;  // Ternary search iterations
```

### Environment Variables

All configuration can be overridden with environment variables:

```bash
# Core configuration
export RPC_URL=https://your-rpc-endpoint.com
export TRANSACTION_COST_MNT=0.02
export DEX_FEE=0.003

# Performance tuning
export TERNARY_SEARCH_ITERATIONS=100
export BLOCK_TIME_SECONDS=2
export MAX_RETRIES=3

# Output configuration
export CSV_FILE_PATH=arbitrage_opportunities.csv
```

## 📋 CSV Field Description

The CSV file contains the following fields:
- `timestamp`: Timestamp when arbitrage opportunity was discovered
- `block_number`: Block number
- `optimal_input_wmnt`: Optimal input amount found by ternary search (WMNT)
- `final_output_wmnt`: Final output amount (WMNT)  
- `gross_profit_wmnt`: Total profit before transaction costs (WMNT)
- `net_profit_wmnt`: Net profit after deducting transaction costs (WMNT)
- `profit_percentage`: Profit rate (%)
- `gas_cost_mnt`: Transaction cost (MNT)
- `search_method`: Search method ("ternary_search")
- `*_reserve0/1`: Reserve amounts of each pool
- `fetch_time_ms`: Data retrieval time (milliseconds)

## 📖 Output Example

```
📝 CSV logging initialized: arbitrage_opportunities.csv
🚀 Starting triangular arbitrage monitor on Mantle Network
📊 Monitoring pools: MOE-WMNT, JOE-MOE, JOE-WMNT
🔍 Algorithm: Ternary search optimization (100 iterations)
🌐 RPC URL: https://rpc.mantle.xyz
⛽ Transaction cost: 0.02 MNT
💹 DEX fee: 0.3%
⏰ Block time: 2 seconds
📝 Logging: Only when reserves change (not every block)
📋 Reserves info: Included in each update

🔄 Reserves changed at block 12345678 (14:30:25.123)
   📊 MOE-WMNT: 15234.56 WMNT / 8567.89 MOE
   📊 JOE-MOE: 4231.12 MOE / 9876.54 JOE
   📊 JOE-WMNT: 6543.21 JOE / 12890.34 WMNT
💎 OPTIMAL ARBITRAGE OPPORTUNITY FOUND!
   🎯 Optimal Input: 0.750000 WMNT (via ternary search)
   📈 Final Output: 0.772500 WMNT
   💰 Gross Profit: 0.022500 WMNT
   🎯 Net Profit: 0.002500 WMNT (0.33%)
   ⛽ After 0.02 MNT tx cost
   🔍 Search iterations: 100
   ⚡ Analysis time: 245ms
   ✅ Logged to CSV: arbitrage_opportunities.csv

🔄 Reserves changed at block 12345892 (14:35:47.456)
   📊 MOE-WMNT: 15120.33 WMNT / 8598.77 MOE
   📊 JOE-MOE: 4251.88 MOE / 9854.12 JOE
   📊 JOE-WMNT: 6567.45 JOE / 12865.78 WMNT
   📊 No profitable opportunity after costs. Gross: 0.015000, Net: -0.005000 WMNT, Time: 189ms

```

## 🏗️ Architecture Optimizations

### Parallel Data Retrieval
Uses `futures::join_all` to fetch all pool data in parallel, providing 3-5x speed improvement over serial fetching.

### Smart Caching and Logging
- **Block-level caching**: Only fetches data on new blocks, reducing API calls
- **Reserves change detection**: Compares actual reserve changes, avoiding meaningless calculations
- **Smart log triggering**: Only outputs logs on meaningful changes, keeping terminal clean

### Ternary Search Optimization Algorithm
Uses mathematical optimization methods to find theoretically optimal input amounts:
- **Constant product formula**: Precise AMM swap calculation `x*y=k`
- **Ternary search**: Finds global maximum of profit function in continuous space
- **Adaptive boundaries**: Search range from 0 to 99.9% of pool reserves
- **High precision**: 100 iterations ensure convergence to optimal solution

### CSV Data Recording
Automatically records all arbitrage opportunities to structured CSV files, including complete search results and pool states.

### Error Recovery
Implements comprehensive retry mechanisms to ensure temporary network issues don't interrupt monitoring.

## 📚 Dependencies

- `alloy`: Ethereum interaction library
- `tokio`: Async runtime
- `futures`: Concurrent processing
- `chrono`: Time handling
- `csv`: CSV file read/write
- `serde`: Data serialization
- `dotenv`: Environment variable management

## 💡 Ternary Search Algorithm Principles

### Mathematical Optimization Method
The system uses ternary search algorithm to find mathematically optimal input amounts:

1. **Objective function**: Arbitrage profit function `f(x) = output(x) - x - transaction_cost`
2. **Search range**: `[0, pool_reserves * 0.999]`
3. **Convergence condition**: Optimal solution after 100 iterations
4. **Precision guarantee**: Numerically stable ternary search ensures global optimum

### Algorithm Advantages
```rust
// Ternary search automatically finds optimal amounts:
// ✅ No need for preset fixed amount ranges
// ✅ Mathematical guarantee to find global optimal solution  
// ✅ Adaptive to different liquidity environments
// ✅ Considers trading fees and slippage effects
```

### Key Formulas
- **AMM swap**: `dy = (y * dx * (1-fee)) / (x + dx * (1-fee))`
- **Profit calculation**: `profit = final_output - initial_input - tx_cost`
- **Ternary search**: Recursively narrow range in `[left, right]` interval to optimal point

## ⚠️ Disclaimer

- This code is for educational and research purposes only
- Conduct thorough testing before actual trading
- Consider slippage and MEV factors
- Ensure sufficient funds and gas fees
- CSV files will grow continuously, clean up or backup regularly
- Always set the RPC_URL environment variable before running