# Helios Mantle Support Audit Report (2026-02-23)

## 审计范围
- 目标仓库: `helios`
- 评审对象: Claude 生成的 Mantle 支持改动（`mantle/`、`cli` 接入、`helios-ts` 接入、workspace 接入）
- 协议对照:
  - `mantle-v2`（OP Node 侧）
  - `op-geth`（Execution/ChainConfig 侧）
- 本次仅审计，不修改实现代码。

## Helios 在 Mantle 场景下的工作原理（实现视角）
1. 共识层输入:
- `mantle/src/consensus.rs` 周期轮询 `consensus_rpc/latest`，获取 `SequencerCommitment`。
- 客户端先验签，再解码成 `ExecutionPayload`，再转换为 `Block<Transaction>` 发给 `HeliosClient`。

2. 执行层验证:
- `mantle/src/builder.rs` 组装执行 provider。
- 二选一:
  - `execution_rpc`
  - `verifiable_api`
- 之后由 `HeliosClient<Mantle>` 统一提供 RPC 行为。

3. EVM 规则选择:
- `mantle/src/evm.rs::get_spec_id_for_block_timestamp` 根据 `ForkSchedule` 选择 `OpSpecId`。
- 此处直接决定 `eth_call`、`estimate_gas`、`create_access_list` 等模拟执行语义。

4. 不安全签名者（可选）校验:
- 若 `verify_unsafe_signer = true`，`mantle/src/consensus.rs` 会通过 L1 证明更新 unsafe signer。
- 依赖 `consensus_rpc/unsafe_signer_proof/{l1_block_hash}` 返回 EIP-1186 证明。

## Mantle 协议对照要点
- L2 chain id:
  - mainnet `5000` (`mantle-v2/.../deploy-config/mantle-mainnet.json:3`)
  - sepolia `5003` (`mantle-v2/.../deploy-config/mantle-sepolia.json:3`)
- `p2pSequencerAddress`:
  - mainnet `0xaac979...66dc` (`mantle-v2/.../mantle-mainnet.json:8`)
  - sepolia `0xff17e3...1d31` (`mantle-v2/.../mantle-sepolia.json:8`)
- mainnet `SystemConfigProxy`:
  - `0x427Ea0...6cAf` (`mantle-v2/.../deployments/mantle-mainnet/SystemConfigProxy.json:2`)
- unsafe signer slot:
  - `keccak256("systemconfig.unsafeblocksigner")`
  - 见 `SystemConfig.sol` 常量定义 (`mantle-v2/.../SystemConfig.sol:51`)

## 发现的问题

### [High] Mantle 分叉对齐与 Go 实现不一致，且 Limb 升级在 Helios EVM 选择器中未生效
证据:
- Helios Mantle 配置把 Canyon..Isthmus 映射到 Skadi，把 Jovian 映射到 Limb:
  - `mantle/src/config.rs:343`
  - `mantle/src/config.rs:352`
- Helios Mantle EVM 选择器仅到 `ISTHMUS`，没有基于 `jovian_timestamp` 的分支:
  - `mantle/src/evm.rs:205`
  - `mantle/src/evm.rs:224`
- Mantle 官方 Go 路径中，OP forks 对齐到 `MantleArsiaTime`:
  - `mantle-v2/op-node/rollup/mantle_types.go:169`
  - `mantle-v2/op-node/rollup/mantle_types.go:176`
  - `op-geth/core/genesis.go:351`
  - `op-geth/core/genesis.go:357`
- Go 路径把 EVM 标准分叉单独对齐:
  - `Shanghai/Cancun/Prague -> Skadi` (`op-geth/core/genesis.go:362`)
  - `Osaka -> Limb` (`op-geth/core/genesis.go:366`)

影响:
- Helios 当前实现把 OP 级别分叉提前到 Skadi 时点，与 Mantle Go 侧“OP 对齐到 Arsia”的规则不一致。
- 由于 EVM 选择器没有 `jovian -> OSAKA` 分支，Limb 时间点（mainnet 配置是 2026-01-14 07:00:00 UTC）不会触发新的 spec 切换。
- 当前日期为 2026-02-23，mainnet Limb 时间已过，需重点评估与链上真实执行规则的偏差风险。

结论:
- 这是 correctness 风险，不是纯文档问题。

### [Medium] `helios-ts` 公共 API 尚未真正暴露 Mantle
证据:
- Rust wasm 侧已新增 `mantle` 模块:
  - `helios-ts/src/lib.rs:8`
  - `helios-ts/src/mantle.rs:26`
- 但 TS 封装仍只导入 `EthereumClient/OpStackClient/LineaClient`:
  - `helios-ts/lib.ts:3`
- `NetworkKind` 未包含 `mantle`:
  - `helios-ts/lib.ts:22`
- `Network` 联合类型未包含 `mantle/mantle-sepolia`:
  - `helios-ts/lib.ts:469`

影响:
- NPM 使用者无法通过 `createHeliosProvider` 选择 Mantle，即使 Rust 绑定已存在。

### [Medium] Mantle CLI 的 `--execution-verifiable-api` 与配置字段未对齐，可能导致运行时 panic
证据:
- CLI 把参数写入 `execution_verifiable_api`:
  - `cli/src/main.rs:404`
- Mantle Config 字段名是 `verifiable_api`:
  - `mantle/src/config.rs:22`
- 当 `verifiable_api` 为空时，Builder 直接 `unwrap(execution_rpc)`:
  - `mantle/src/builder.rs:113`

影响:
- 用户只传 `--execution-verifiable-api` 而不传 `--execution-rpc` 时，配置可能未正确落到 `verifiable_api`，进而触发 panic。
- 该行为是可用性和稳定性风险。

备注:
- 该模式与 `opstack` 现状一致，属于继承问题，但已影响 Mantle 入口。

### [Low/Operational] Mantle 没有默认 `consensus_rpc`，生产部署需要自建或外部托管兼容服务
证据:
- 默认 `consensus_rpc: None`:
  - `mantle/src/config.rs:81`
  - `mantle/src/config.rs:101`
- 客户端强依赖 `GET /latest`:
  - `mantle/src/consensus.rs:136`
- 若开启 signer 校验，还依赖 `GET /unsafe_signer_proof/{hash}`:
  - `mantle/src/consensus.rs:220`

影响:
- 无法像部分 OP 网络一样“开箱即用”，需要额外基础设施与运维约束。

## 非阻塞观察与待确认项
1. `/latest` 负载格式需与 Helios 解码假设完全一致（`mantle/src/lib.rs:57`）。
2. `mantle-sepolia` 的 `system_config_contract` 地址来源于文档注释，仓库内未找到同级部署清单可直接交叉校验。

## 验证记录
- 已执行: `cargo test -p helios-mantle`
  - 结果: 76 passed, 0 failed
- 未完成: `cargo check -p helios-cli`
  - 原因:
    - 默认 cargo cache 权限错误（`Operation not permitted`）
    - 切换 `CARGO_HOME=/tmp/cargo-home` 后 crates.io 拉取反复超时

