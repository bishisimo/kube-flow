//! Kube-Flow：Tauri 2 + K8s 资源管理。模块分层：config → credentials → env → kube → commands。

mod config;
mod credentials;
mod debug_log;
mod env;
mod kube;
mod commands;
mod ssh_askpass;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            commands::setup_app_state(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(store) = window.app_handle().try_state::<crate::kube::KubeClientStore>() {
                    store.close_all_tunnels();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::config_commands::app_data_dir,
            commands::config_commands::ensure_app_data_dir,
            commands::config_commands::app_settings_get_ssh_tunnel_mode,
            commands::config_commands::app_settings_set_ssh_tunnel_mode,
            commands::config_commands::app_settings_get_auto_snapshot_enabled,
            commands::config_commands::app_settings_set_auto_snapshot_enabled,
            commands::config_commands::app_settings_get_auto_snapshot_limit_per_resource,
            commands::config_commands::app_settings_set_auto_snapshot_limit_per_resource,
            commands::config_commands::app_settings_get_terminal_instance_cache_limit,
            commands::config_commands::app_settings_set_terminal_instance_cache_limit,
            commands::config_commands::app_settings_get_log_active_stream_limit,
            commands::config_commands::app_settings_set_log_active_stream_limit,
            commands::config_commands::app_settings_get_resource_deploy_strategy,
            commands::config_commands::app_settings_set_resource_deploy_strategy,
            commands::config_commands::app_settings_get_node_resource_usage_enabled,
            commands::config_commands::app_settings_set_node_resource_usage_enabled,
            commands::config_commands::app_settings_get_builtin_gpu_resource_names,
            commands::config_commands::app_settings_get_custom_gpu_resource_rules,
            commands::config_commands::app_settings_set_custom_gpu_resource_rules,
            commands::log_commands::log_get_level,
            commands::log_commands::log_set_level,
            commands::log_commands::log_get_display_settings,
            commands::log_commands::log_set_display_settings,
            commands::log_commands::log_read,
            commands::log_commands::log_clear,
            commands::env_commands::env_list,
            commands::env_commands::env_add,
            commands::env_commands::env_update,
            commands::env_commands::env_delete,
            commands::env_commands::env_touch,
            commands::env_commands::env_list_contexts_from_kubeconfig,
            commands::env_commands::env_set_current_context,
            commands::env_commands::env_list_ssh_tunnels,
            commands::env_commands::env_list_ssh_config_hosts,
            commands::env_commands::env_create_local,
            commands::env_commands::env_create_ssh,
            commands::env_commands::env_create_ssh_with_host,
            commands::env_commands::env_ensure_ssh_tunnel_for_host,
            commands::kube_commands::kube_list_namespaces,
            commands::kube_commands::kube_list_nodes,
            commands::kube_commands::kube_list_pods,
            commands::kube_commands::kube_list_pods_for_workload,
            commands::kube_commands::kube_list_deployments,
            commands::kube_commands::kube_list_services,
            commands::kube_commands::kube_list_stateful_sets,
            commands::kube_commands::kube_list_config_maps,
            commands::kube_commands::kube_list_secrets,
            commands::kube_commands::kube_list_service_accounts,
            commands::kube_commands::kube_list_roles,
            commands::kube_commands::kube_list_role_bindings,
            commands::kube_commands::kube_list_cluster_roles,
            commands::kube_commands::kube_list_cluster_role_bindings,
            commands::kube_commands::kube_list_daemon_sets,
            commands::kube_commands::kube_list_persistent_volume_claims,
            commands::kube_commands::kube_list_persistent_volumes,
            commands::kube_commands::kube_list_storage_classes,
            commands::kube_commands::kube_list_endpoints,
            commands::kube_commands::kube_list_endpoint_slices,
            commands::kube_commands::kube_list_replica_sets,
            commands::kube_commands::kube_list_jobs,
            commands::kube_commands::kube_list_cron_jobs,
            commands::kube_commands::kube_list_ingresses,
            commands::kube_commands::kube_list_ingress_classes,
            commands::kube_commands::kube_list_network_policies,
            commands::kube_commands::kube_list_resource_quotas,
            commands::kube_commands::kube_list_limit_ranges,
            commands::kube_commands::kube_list_priority_classes,
            commands::kube_commands::kube_list_horizontal_pod_autoscalers,
            commands::kube_commands::kube_list_pod_disruption_budgets,
            commands::kube_commands::kube_describe_resource,
            commands::kube_commands::kube_get_resource_graph,
            commands::kube_commands::kube_get_pod_containers,
            commands::kube_commands::kube_pod_log_stream_start,
            commands::kube_commands::kube_pod_log_stream_stop,
            commands::kube_commands::kube_pod_exec_start,
            commands::kube_commands::kube_pod_exec_stdin,
            commands::kube_commands::kube_pod_exec_resize,
            commands::kube_commands::kube_pod_exec_stop,
            commands::terminal_commands::host_shell_start,
            commands::terminal_commands::host_shell_stdin,
            commands::terminal_commands::host_shell_resize,
            commands::terminal_commands::host_shell_stop,
            commands::kube_commands::kube_pod_logs,
            commands::kube_commands::kube_get_resource,
            commands::kube_commands::kube_list_crd_instances,
            commands::kube_commands::kube_get_dynamic_resource,
            commands::kube_commands::kube_describe_dynamic_resource,
            commands::kube_commands::kube_delete_dynamic_resource,
            commands::kube_commands::kube_delete_resource,
            commands::kube_commands::kube_apply_resource,
            commands::kube_commands::kube_deploy_resource,
            commands::kube_commands::kube_patch_container_images,
            commands::kube_commands::kube_get_tunnel_local_port,
            commands::kube_commands::kube_remove_client,
            commands::kube_commands::kube_refresh_resource_aliases,
            commands::kube_commands::kube_resolve_resource_alias,
            commands::kube_commands::kube_search_resource_kinds,
            commands::kube_commands::kube_start_watch,
            commands::kube_commands::kube_stop_watch,
            // 安全设置
            commands::credential_commands::security_get_settings,
            commands::credential_commands::security_set_credential_store,
            commands::credential_commands::security_set_stronghold_path,
            commands::credential_commands::security_set_auto_lock_minutes,
            // 凭证 CRUD
            commands::credential_commands::credential_save,
            commands::credential_commands::credential_delete,
            commands::credential_commands::credential_exists,
            commands::credential_commands::credential_get,
            commands::credential_commands::credential_list,
            commands::credential_commands::credential_cache_only,
            // Stronghold 状态机
            commands::credential_commands::stronghold_get_status,
            commands::credential_commands::stronghold_initialize,
            commands::credential_commands::stronghold_unlock,
            commands::credential_commands::stronghold_lock,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
