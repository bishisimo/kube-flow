//! Pod 文件传输：基于 Kubernetes exec 的单文件流式上传/下载，支持进度与取消。

use crate::kube::file_transfer::{check_cancelled, emit_progress, ProgressEmitter};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams};
use kube::Client;
use std::path::Path;
use tauri::AppHandle;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

const CHUNK_SIZE: usize = 64 * 1024;

async fn read_optional_to_end<R>(reader: Option<R>) -> Result<Vec<u8>, String>
where
    R: AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    reader
        .read_to_end(&mut out)
        .await
        .map_err(|e| e.to_string())?;
    Ok(out)
}

fn ensure_write_allowed(path: &Path, overwrite: bool) -> Result<(), String> {
    if path.exists() && !overwrite {
        return Err(format!("本地文件已存在: {}", path.display()));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn shell_single_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\"'\"'"))
}

/// Quote a remote path for `/bin/sh -c` while preserving leading `~` / `~user` home expansion.
fn shell_quote_remote_path(path: &str) -> String {
    if path == "~" {
        return "$HOME".to_string();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return format!("$HOME/{}", shell_single_quote(rest));
    }
    if let Some(rest) = path.strip_prefix('~') {
        if let Some((user, rem)) = rest.split_once('/') {
            if !user.is_empty()
                && user
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return format!("~{}/{}", user, shell_single_quote(rem));
            }
        } else if !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return format!("~{}", rest);
        }
    }
    shell_single_quote(path)
}

fn build_attach_params(container: Option<&str>, stdin: bool) -> AttachParams {
    let mut params = AttachParams::default()
        .stdin(stdin)
        .stdout(true)
        .stderr(true)
        .tty(false);
    if let Some(container) = container.filter(|v| !v.trim().is_empty()) {
        params = params.container(container.to_string());
    }
    params
}

/// 将本地文件流式写入 Pod 内目标路径，并通过事件报告进度。
pub async fn upload_file_to_pod(
    app: &AppHandle,
    transfer_id: &str,
    cancel_rx: &mut oneshot::Receiver<()>,
    client: Client,
    namespace: &str,
    pod_name: &str,
    container: Option<&str>,
    local_path: &str,
    remote_path: &str,
    overwrite: bool,
) -> Result<(), String> {
    check_cancelled(cancel_rx)?;
    let local_path = Path::new(local_path);
    if !local_path.is_file() {
        return Err(format!("本地文件不存在或不是文件: {}", local_path.display()));
    }
    let remote_path = remote_path.trim();
    if remote_path.is_empty() {
        return Err("远端路径不能为空".to_string());
    }

    let meta = tokio::fs::metadata(local_path)
        .await
        .map_err(|e| format!("读取本地文件失败: {}", e))?;
    let total_bytes = Some(meta.len());
    let mut file = tokio::fs::File::open(local_path)
        .await
        .map_err(|e| format!("打开本地文件失败: {}", e))?;

    let quoted_remote = shell_quote_remote_path(remote_path);
    let script = if overwrite {
        format!("cat > {}", quoted_remote)
    } else {
        format!(
            "[ ! -e {p} ] || {{ echo \"远端文件已存在: {raw}\" >&2; exit 1; }}; cat > {p}",
            p = quoted_remote,
            raw = remote_path.replace('\"', "\\\"")
        )
    };
    let command = vec!["/bin/sh".to_string(), "-c".to_string(), script];
    check_cancelled(cancel_rx)?;
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let mut attached = api
        .exec(pod_name, command, &build_attach_params(container, true))
        .await
        .map_err(|e| e.to_string())?;

    let mut stdin = attached
        .stdin()
        .ok_or_else(|| "无法打开 Pod exec stdin".to_string())?;

    emit_progress(app, transfer_id, 0, total_bytes);
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut transferred: u64 = 0;
    let mut ticker = ProgressEmitter::new();
    loop {
        let n = tokio::select! {
            biased;
            _ = &mut *cancel_rx => {
                return Err("已取消".to_string());
            }
            result = file.read(&mut buf) => {
                result.map_err(|e| format!("读取本地文件失败: {}", e))?
            }
        };
        if n == 0 {
            break;
        }
        stdin
            .write_all(&buf[..n])
            .await
            .map_err(|e| format!("写入容器失败: {}", e))?;
        transferred += n as u64;
        ticker.emit(app, transfer_id, transferred, total_bytes);
    }
    emit_progress(app, transfer_id, transferred, total_bytes);
    stdin
        .shutdown()
        .await
        .map_err(|e| format!("关闭容器写入流失败: {}", e))?;
    drop(stdin);

    let (_, stderr) = tokio::try_join!(
        read_optional_to_end(attached.stdout()),
        read_optional_to_end(attached.stderr())
    )?;
    if !stderr.is_empty() {
        return Err(String::from_utf8_lossy(&stderr).trim().to_string());
    }
    Ok(())
}

/// 从 Pod 流式下载文件到本地，并通过事件报告进度。
pub async fn download_file_from_pod(
    app: &AppHandle,
    transfer_id: &str,
    cancel_rx: &mut oneshot::Receiver<()>,
    client: Client,
    namespace: &str,
    pod_name: &str,
    container: Option<&str>,
    remote_path: &str,
    local_path: &str,
    overwrite: bool,
) -> Result<(), String> {
    check_cancelled(cancel_rx)?;
    let remote_path = remote_path.trim();
    if remote_path.is_empty() {
        return Err("远端路径不能为空".to_string());
    }
    let local_path = Path::new(local_path);
    ensure_write_allowed(local_path, overwrite)?;

    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        format!("cat {}", shell_quote_remote_path(remote_path)),
    ];
    check_cancelled(cancel_rx)?;
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let mut attached = api
        .exec(pod_name, command, &build_attach_params(container, false))
        .await
        .map_err(|e| e.to_string())?;

    let mut stdout = attached
        .stdout()
        .ok_or_else(|| "无法打开 Pod exec stdout".to_string())?;
    let mut file = tokio::fs::File::create(local_path)
        .await
        .map_err(|e| format!("创建本地文件失败: {}", e))?;

    emit_progress(app, transfer_id, 0, None);
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut transferred: u64 = 0;
    let mut ticker = ProgressEmitter::new();
    loop {
        let n = tokio::select! {
            biased;
            _ = &mut *cancel_rx => {
                let _ = tokio::fs::remove_file(local_path).await;
                return Err("已取消".to_string());
            }
            result = stdout.read(&mut buf) => {
                result.map_err(|e| format!("读取容器输出失败: {}", e))?
            }
        };
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .await
            .map_err(|e| format!("写入本地文件失败: {}", e))?;
        transferred += n as u64;
        ticker.emit(app, transfer_id, transferred, None);
    }
    emit_progress(app, transfer_id, transferred, Some(transferred));
    file.flush()
        .await
        .map_err(|e| format!("刷新本地文件失败: {}", e))?;
    drop(file);

    let stderr = read_optional_to_end(attached.stderr()).await?;
    if !stderr.is_empty() {
        let _ = tokio::fs::remove_file(local_path).await;
        return Err(String::from_utf8_lossy(&stderr).trim().to_string());
    }
    Ok(())
}
