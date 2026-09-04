# 工程电子制造 MCP

这是一个只读优先的工程电子制造 MCP。它以独立 Rust 二进制运行，通过 stdio 接入任意支持本地 MCP 子进程的 Agent，不包含 Tauri 或前端代码。

## 使用

```bash
electronics-manufacturing-mcp doctor --config config/default.toml
electronics-manufacturing-mcp serve --config config/default.toml
```

未设置 `filesystem.allowed_roots` 时，服务只能读取启动时的当前工作目录。只有调用 `pcb_validate_release` 并显式传入报告目录时才会写入 JSON/Markdown 报告；源设计和制造文件始终只读。

侧载包中的 `manifest.json` 是宿主无关的清单。宿主应从包目录解析命令和配置路径，以用户工作区作为子进程工作目录，并通过 stdin/stdout 交换 MCP 消息。运行期不需要网络；`kicad-cli` 是可选依赖。

## 首版边界

支持目录或 ZIP 中的 BOM、CPL、Gerber、Excellon 和 IPC-2581 文件，需求追溯支持 JSON/CSV/Markdown，另提供 BOM 风险和 SPICE 网表静态检查，内置通用及 JLCPCB 字段规则。检查结果不能替代制造商 CAM 审核、原生 ERC/DRC、法规认证、实时供应商查询、仿真器或人工发布批准。

当前 MCP 工具分为四组：

- PCB/EDA：`kicad_inspect_design`、`kicad_semantic_review`、`kicad_design_review`、`kicad_connectivity_review`、`kicad_schematic_pcb_consistency`、`kicad_power_tree_review`、`kicad_trace_signal`、`kicad_compare_revisions`、`pcb_inspect_package`、`pcb_validate_bom`、`pcb_compare_bom_cpl`、`pcb_validate_gerber_set`、`pcb_validate_ipc2581`、`pcb_validate_release`、`pcb_run_kicad_checks`。
- 需求与追溯：`requirements_ingest`、`requirements_quality_review`、`requirements_traceability`、`requirements_change_impact`。
- 物料风险：`bom_review_risk`，只基于 BOM 中的生命周期、制造商、供应商和替代料字段，不联网查询库存。
- 仿真准备：`spice_validate_netlist`，检查网表结构，不启动 ngspice/LTspice。

需求文件可以是 JSON：

```json
{"requirements":[{"id":"REQ-001","statement":"Input shall tolerate 24 V","status":"approved","verification_method":"test"}]}
```

追溯链接可以是 CSV：

```csv
requirement_id,target,relation,evidence
REQ-001,test:T-001,verified_by,results/T-001.json
```

`requirements_traceability` 会返回覆盖/未覆盖需求、未知 ID、追溯目标和证据缺口；`requirements_change_impact` 会按需求 ID返回受影响对象及同标签相关需求。

构建侧载包前需安装 Rust 目标 `x86_64-unknown-linux-musl`、`x86_64-pc-windows-gnu` 及 MinGW 工具链，然后运行 `packaging/package.sh`。

## GitHub Actions

仓库内的 `.github/workflows/ci.yml` 会在 Pull Request、`main`/`master` 推送、`v*.*.*` 标签和手动触发时运行：

1. 在 GitHub 新建空仓库，将本目录提交并推送到 `main` 或 `master`。不要提交 `target/`；本地 `dist/` 归档也可以不提交，Actions 会重新生成。
2. 打开仓库的 **Actions** 页面，选择 **CI**。第一次运行时如果 GitHub 要求启用 Actions，选择允许工作流运行。
3. `Quality and tests` job 会使用 stable Rust 执行 `cargo fmt`、Clippy、Rust 测试、Skills frontmatter/catalog 检查和 UTF-8 无 BOM 检查；项目的最低 Rust 版本仍由 `Cargo.toml` 的 `rust-version = 1.88` 声明。
4. `Build side-load packages` 只有在质量 job 通过后才执行。它会在 Ubuntu runner 安装 musl/MinGW，构建 Linux 静态二进制和 Windows x64 `.exe`，校验归档内外 SHA-256，并上传 artifact。
5. 在成功的工作流运行页面底部下载 `electronics-manufacturing-mcp-<commit>` artifact。里面包含两个侧载包和 `SHA256SUMS.txt`；解压后按包内 `manifest.json` 注册到你的通用 Agent。
6. 发布正式版本时，先确认 `Cargo.toml` 版本号，再创建并推送对应标签。质量和打包 Job 成功后，`Publish GitHub Release` 会自动创建 Release 并上传两个侧载包和校验文件：

```bash
git tag v0.1.0
git push origin v0.1.0
```

标签必须符合 `v主版本.次版本.修订版本`，例如 `v0.1.0`。同一个标签重复运行会因为 Release 已存在而失败；修改后请使用新的版本号和标签。

本地可先复现质量 job：

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

本地可先复现打包 job（需要 `jq`、`zip`、`musl-tools`、`mingw-w64` 和两个 Rust target）：

```bash
packaging/package.sh
(cd dist && sha256sum -c SHA256SUMS.txt)
```

如果只想验证代码而不构建跨平台包，可以在 Actions 页面使用 job 旁的日志查看具体失败步骤。真实 KiCad ERC/DRC 不在 CI 中执行，因为 runner 没有安装 KiCad 和你的工程；`pcb_run_kicad_checks` 会在用户侧载 MCP 的工作站上按配置检测 `kicad-cli`。
