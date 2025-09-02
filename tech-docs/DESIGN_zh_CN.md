# 多路径多池原子套利设计

## Prerequisite

1. Merchant Moe 中存在两种池子设计，一种是类似于 Uniswao v2 的 MoeLP 池，即维持最简单的 x*y=z 作为 reserves 的依据，另一种则是类似于 Trade Joe/Uniswap v3 的 tick 设计

## Phase 1 - 单 DEX 的简单三角套利

1. 选择 Merchant Moe 中的 MoeLP 中 volume 最高的 WMNT-MOE-JOE 作为一组三角套利池
2. 设计思路为：
    2.1 链下服务获取三个池子的 reserves 情况（每 2s），如果有变化则开始计算是否存在套利空间
    2.2 通过三分法获取最佳套利方案，在剔除手续费的情况下查看是否存在利润空间
    2.3 构造套利交易，在这里我们需要部署一个合约来实现套利交易的构造，因为合约可以帮助我们在链上再次查询一次 reserves 情况并规避一定风险（即如果该套利机会已经不存在了可以及时 revert）

## Phase 2 - 单 DEX 的多池套利

1. 选择 Merchant Moe 中的 MoeLP 结构池，爬取所有目前存在流动性的 MoeLP 池，构建一个资产池图，将每个币种看做一个顶点，每个交易对就是一条边，问题就转化成了如何在一个有向图里面寻找环状路径的问题
2. 通过图论的方式，来计算，需要注意的是路径每增加一步就要消耗更多 gas，因此需要控制长度(maxHops)，在这里选择的是 3hops 或者 4 hops
3. 提前查询好流动性充足的池子，放在 /data/selected.csv 文件中，通过里面的池子来组建 3 hops 和 4 hops 的套利路径，目前有 6 个 3 hops 路径和 9 个 4 hops 路径。
3. 设计思路为：
    3.1 内置一个 graph 的模块，使用 petgraph 构建无向图，节点是代币，边是池子
    3.2 每 2s 对池状态数据（使用 getReserves() 调用）的并发访问，如果存在 reserve 的变化，需要并行计算所有
    3.3 需要注意的是 3 hops 和 4 hops 所消耗的 gas 是不一样的，大约是 700m/720m gas，gasprice 我们假定为 0.02 gwei，gas的单位为 MNT，MNT 的价格假定为 $1.1，这样方便你进行成本估算
    3.4 需要将 WMNT 作为套利路径的起点和终点
4. 其他
    4.1 系统的模块设计你可以参考这个：https://github.com/cakevm/swap-path，这里有对应的介绍：
        - https://deepwiki.com/cakevm/swap-path/1-overview
        - https://deepwiki.com/cakevm/swap-path/2-architecture-and-dependencies
        - https://deepwiki.com/cakevm/swap-path/3-core-components
            - https://deepwiki.com/cakevm/swap-path/3.1-market-management
            - https://deepwiki.com/cakevm/swap-path/3.2-graph-based-pathfinding
            - https://deepwiki.com/cakevm/swap-path/3.3-pool-abstractions
            - https://deepwiki.com/cakevm/swap-path/3.4-swappath-data-structures

    4.2 我希望你可以构建一个高性能的系统，因为在套利中时效性很重要，因此几个优化点可能是：
        1. 对于 reserves 的情况，直接 batch request 请求所有交易对的最新状态
        2. 想办法优化寻找环的速度，可以优化 dfs，也可以尝试用 bellman-ford 之类的其他图算法，或者有更好的算法也可以使用


## Reference

- [Uniswap Arbitrage Analysis](https://github.com/ccyanxyz/uniswap-arbitrage-analysis/blob/master/readme_zh.md)
- [Swap-path](https://github.com/cakevm/swap-path)
- 