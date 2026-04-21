/// SSH_ASKPASS 临时脚本守卫：构造时写盘，Drop 时自动删除。
/// 仅 Unix 平台可用；脚本内容为向 stdout 输出密码，供 ssh 子进程调用。

#[cfg(unix)]
pub struct SshAskpassGuard {
    path: std::path::PathBuf,
}

#[cfg(not(unix))]
pub struct SshAskpassGuard;

#[cfg(unix)]
impl SshAskpassGuard {
    pub fn new(password: &str) -> Result<Self, std::io::Error> {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir()
            .join(format!("kf-askpass-{}.sh", uuid::Uuid::new_v4()));
        // 单引号转义：' → '\''
        let escaped = password.replace('\'', "'\\''");
        let content = format!("#!/bin/sh\nprintf '%s' '{}'\n", escaped);
        std::fs::write(&path, &content)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self { path })
    }

    pub fn path_str(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }
}

#[cfg(unix)]
impl Drop for SshAskpassGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
