// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 与 GUI 共用同一可执行文件：`kube-flow mcp` 进入 stdio MCP 模式。
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("mcp") {
        if let Err(e) = kube_flow_lib::run_mcp_stdio() {
            eprintln!("kube-flow mcp error: {e}");
            std::process::exit(1);
        }
        return;
    }
    kube_flow_lib::run()
}
