//! 终端 I/O 编解码：base64 载荷与 TTY 尺寸归一化。

use base64::Engine;

/// 交互式 shell 启动时注入的终端环境（TERM / COLORTERM / locale）。
pub const SHELL_ENV_EXPORTS: &str = "export TERM=\"xterm-256color\"; \
export COLORTERM=\"truecolor\"; \
export LANG=\"${LANG:-C.UTF-8}\"; \
export LC_CTYPE=\"${LC_CTYPE:-$LANG}\";";

/// 将二进制 chunk 编码为 STANDARD base64，供 Tauri 事件传输。
pub fn encode_chunk_b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// 解码前端 stdin 的 base64 载荷。
pub fn decode_stdin_b64(data_b64: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .map_err(|e| format!("invalid base64 stdin: {e}"))
}

/// 归一化 PTY / exec 初始尺寸；缺省或非法值回落到可用默认。
pub fn normalize_tty_size(cols: Option<u16>, rows: Option<u16>) -> (u16, u16) {
    let cols = cols.unwrap_or(120).clamp(2, 1000);
    let rows = rows.unwrap_or(36).clamp(1, 500);
    (cols, rows)
}

/// 输出背压高水位（未 ACK 字节数超过后暂停读）。
pub const OUTPUT_FLOW_HIGH: usize = 512 * 1024;
