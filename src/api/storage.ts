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
