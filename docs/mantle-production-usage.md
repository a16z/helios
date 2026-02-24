# Helios 在 Mantle 生产环境使用指南

## 1. 生产拓扑（最小可用）
1. Helios 节点（本项目 `helios mantle`）
2. L2 执行端点（二选一）
- `execution_rpc`（标准 Execution RPC，需支持 `eth_getProof`）
- `verifiable_api`（Helios Verifiable API）
3. Mantle 共识承诺服务（必须）
- 提供 `GET /latest`
4. 可选安全增强
- 若开启 `verify_unsafe_signer = true`，共识服务还必须提供 `GET /unsafe_signer_proof/{l1_block_hash}`。

## 2. 必须先明确的网络常量
- Mantle Mainnet:
  - chain id: `5000`
  - 默认 unsafe signer: `0xaac979cbee00c75c35de9a2635d8b75940f466dc`
  - system config contract: `0x427Ea0710FA5252057F0D88274f7aeb308386cAf`
- Mantle Sepolia:
  - chain id: `5003`
  - 默认 unsafe signer: `0xff17e3dd9c0e0378ab219111f5325c83ebd01d31`
  - system config contract: `0x04B34526C91424e955D13C7226Bc4385e57E6706`

以上常量来自 `mantle/src/config.rs`，其中 `consensus_rpc` 默认是空值，生产必须显式配置。

## 3. 共识服务接口契约
1. `GET /latest`
- 返回最新 `SequencerCommitment` JSON（Helios 可直接反序列化）。
- Helios 每秒轮询一次该接口。

2. `GET /unsafe_signer_proof/{l1_block_hash}`（仅在 signer 校验开启时）
- 返回 EIP-1186 `eth_getProof` 风格响应。
- 证明目标是 L1 `SystemConfig` 合约中的 unsafe signer slot。

3. `GET /chain_id`
- Mantle 客户端本身不直接依赖该接口，但建议提供，便于副本一致性校验与运维排障。

## 4. 推荐配置文件（生产）
文件: `~/.helios/helios.toml`

```toml
[mantle]
consensus_rpc = "https://YOUR-MANTLE-CONSENSUS-SERVICE"
execution_rpc = "https://YOUR-MANTLE-EXECUTION-RPC"
rpc_bind_ip = "127.0.0.1"
rpc_port = 8545

# 可选: 启用 unsafe signer 的 L1 证明校验
verify_unsafe_signer = true

# 可选: 给 signer 校验流程提供 L1 弱主观检查点
# checkpoint = "0x..."
# load_external_fallback = true

[mantle-sepolia]
consensus_rpc = "https://YOUR-MANTLE-SEPOLIA-CONSENSUS-SERVICE"
execution_rpc = "https://YOUR-MANTLE-SEPOLIA-EXECUTION-RPC"
rpc_bind_ip = "127.0.0.1"
rpc_port = 8546
verify_unsafe_signer = true
```

## 5. 启动方式
1. Mainnet:

```bash
helios mantle \
  --network mantle \
  --consensus-rpc https://YOUR-MANTLE-CONSENSUS-SERVICE \
  --execution-rpc https://YOUR-MANTLE-EXECUTION-RPC
```

2. Sepolia:

```bash
helios mantle \
  --network mantle-sepolia \
  --consensus-rpc https://YOUR-MANTLE-SEPOLIA-CONSENSUS-SERVICE \
  --execution-rpc https://YOUR-MANTLE-SEPOLIA-EXECUTION-RPC
```

## 6. 上线前检查清单
1. 共识服务健康检查:
- `curl -s https://YOUR-MANTLE-CONSENSUS-SERVICE/latest`
- 返回非空 JSON，且持续更新。

2. Helios 本地 RPC 检查:
- `eth_chainId`
- `eth_blockNumber`
- `eth_getProof`（验证执行端可用）

3. signer 校验链路检查（开启时）:
- `curl -s https://YOUR-MANTLE-CONSENSUS-SERVICE/unsafe_signer_proof/<l1_block_hash>`
- 返回可被 EIP-1186 反序列化的结构。

4. 执行端点主备:
- 至少两条可切换的执行 RPC（同链同高度策略）。

5. 日志与告警:
- `/latest` 连续超时
- block age 持续变大
- signer proof 校验失败

## 7. 生产注意事项
1. 当前实现不提供 Mantle 默认 `consensus_rpc`，请把共识服务视为必需依赖。
2. 若使用 `verifiable_api`，建议优先在 `helios.toml` 显式设置 `verifiable_api` 字段。
3. 当前 `helios-ts` 公共 TS API 尚未完整暴露 Mantle（Rust 绑定已存在，TS 封装未全接通）。
4. Mantle 升级节奏与 OP 标准链不同，建议在每次网络升级前回归 `eth_call/estimateGas` 关键路径。

