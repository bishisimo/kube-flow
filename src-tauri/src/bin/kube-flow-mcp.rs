//! 兼容入口：等同于主程序 `kube-flow mcp`。

fn main() {
    if let Err(e) = kube_flow_lib::run_mcp_stdio() {
        eprintln!("kube-flow-mcp error: {e}");
        std::process::exit(1);
    }
}
