import { invoke } from "@tauri-apps/api/core";

export interface StorageFileItem {
  name: string;
  bytes: number;
}

export interface StorageDirectoryUsage {
  path: string;
  totalBytes: number;
  fileCount: number;
  files: StorageFileItem[];
}

export interface StorageDiskUsage {
  appDataDir: string;
  debugLogs: StorageDirectoryUsage;
  appFiles: StorageFileItem[];
  sshBackups: StorageDirectoryUsage;
}

export async function storageGetDiskUsage(): Promise<StorageDiskUsage> {
  return invoke<StorageDiskUsage>("storage_get_disk_usage");
}

export async function storageDeleteSshBackups(): Promise<number> {
  return invoke<number>("storage_delete_ssh_backups");
}

/** 将文本写入用户选定的绝对路径。 */
export async function storageWriteTextFile(path: string, content: string): Promise<void> {
  await invoke("storage_write_text_file", { path, content });
}
