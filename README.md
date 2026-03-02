# Kube-Flow

Kube-Flow 是一款基于 Tauri 2 与 Vue 3 的 Kubernetes 桌面管理工具，提供多集群环境集中管理、经 SSH 隧道连接远程集群、在工作台中浏览 Namespace/Pod 等资源、查看 Pod 日志与进入容器 Shell，以及 ConfigMap/Secret 的查看与编辑；在同一窗口内切换上下文、进行日常运维与排障。

## 应用特性

- **多环境管理**：从本地 kubeconfig 导入 context，或通过 SSH 隧道关联远程集群；支持显示名、默认 namespace、标签、收藏与排序，配置以 TOML 持久化，支持版本管理。
- **SSH 隧道**：经跳板机访问内网集群，隧道配置与 `~/.ssh/config` 集成，连接状态与进度在界面中可见。
- **工作台**：可折叠的已打开环境栏、快速切换/关闭环境，按 namespace 浏览 Pod 等资源，集中展示资源详情与拓扑关联。
- **运维能力**：Pod 日志流式查看、进入容器 Shell（xterm.js 终端）、工作负载日志聚合；ConfigMap/Secret 的查看与键值编辑。
- **资源关联**：从资源详情出发的拓扑与关联导航，可追踪 Deployment → ReplicaSet → Pod 等关系。

## 技术栈

- **后端**：Rust (Tauri 2)、kube-rs、TOML 配置
- **前端**：Vue 3、TypeScript、Vite、Vue Router
- **包管理**：**Bun**

## 如何使用

**请使用 Bun** 安装依赖与运行脚本：

```bash
bun install
bun run dev        # 仅前端
bun run tauri dev  # Tauri 开发（前端 + Rust）
```

构建：

```bash
bun run build      # 前端
bun run tauri build  # 完整应用
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
