/**
 * 编排中心共享类型定义。
 */

export interface OrchestratorResourceRef {
  kind: string;
  name: string;
  namespace: string | null;
}

export interface OrchestratorImportResourceInput {
  component: string;
  kind: string;
  name: string;
  namespace: string | null;
  yaml: string;
  source_file_name?: string | null;
  source_doc_index?: number | null;
}

export interface ManifestHistoryItem {
  id: string;
  at: string;
  action: "sync" | "save" | "apply" | "restore";
  yaml: string;
}

export interface OrchestratorManifest {
  id: string;
  env_id: string;
  env_name: string;
  component: string;
  resource_kind: string;
  resource_name: string;
  resource_namespace: string | null;
  yaml: string;
  created_at: string;
  updated_at: string;
  history: ManifestHistoryItem[];
  source_type?: "manual" | "import_file" | "import_text" | "sync_from_workbench" | "package_sync";
  source_batch_id?: string | null;
  source_file_name?: string | null;
  source_doc_index?: number | null;
}

export interface OrchestratorImportBatch {
  id: string;
  env_id: string;
  env_name: string;
  name: string;
  source_kind: "file" | "text";
  file_count: number;
  document_count: number;
  resource_count: number;
  error_count: number;
  warning_count: number;
  created_at: string;
  strategy_snapshot: {
    component: string;
    overwrite: boolean;
  };
  summary: string;
}

export interface OrchestratorPackageResourceSnapshot {
  id: string;
  source_manifest_id: string;
  component: string;
  resource_kind: string;
  resource_name: string;
  resource_namespace: string | null;
  yaml: string;
}

export interface OrchestratorPackageVersion {
  id: string;
  label: string;
  tag: string | null;
  source_env_id: string;
  source_env_name: string;
  component_names: string[];
  created_at: string;
  resources: OrchestratorPackageResourceSnapshot[];
}

export interface OrchestratorPackageDeploymentRecord {
  id: string;
  at: string;
  package_id: string;
  package_name: string;
  version_id: string;
  version_label: string;
  target_env_id: string;
  target_env_name: string;
  mode: "sync" | "apply";
  total: number;
  success: number;
  failed: number;
  errors: string[];
}

export interface OrchestratorPackage {
  id: string;
  name: string;
  description: string;
  created_at: string;
  updated_at: string;
  versions: OrchestratorPackageVersion[];
  deployments: OrchestratorPackageDeploymentRecord[];
}

export interface OrchestratorFocusTarget {
  env_id: string;
  component: string;
  manifest_id?: string | null;
  resource_kind?: string | null;
  resource_name?: string | null;
  resource_namespace?: string | null;
}
