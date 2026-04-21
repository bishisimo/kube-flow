import type { Ref } from "vue";
import type {
  ClusterRoleBindingItem,
  ClusterRoleItem,
  ConfigMapItem,
  CronJobItem,
  DaemonSetItem,
  DeploymentItem,
  EndpointSliceItem,
  EndpointsItem,
  HorizontalPodAutoscalerItem,
  IngressClassItem,
  IngressItem,
  JobItem,
  LimitRangeItem,
  NamespaceItem,
  NetworkPolicyItem,
  NodeItem,
  PersistentVolumeClaimItem,
  PersistentVolumeItem,
  PodDisruptionBudgetItem,
  PodItem,
  PriorityClassItem,
  ReplicaSetItem,
  ResourceQuotaItem,
  RoleBindingItem,
  RoleItem,
  SecretItem,
  ServiceAccountItem,
  ServiceItem,
  StatefulSetItem,
  StorageClassItem,
} from "../../../api/kube";
import type { ResourceKind } from "../../../constants/resourceKinds";

export type WorkbenchTableColumn = { key: string; label: string };

export type NodeAllocSnapshot = Record<string, { cpuRequests: string; memoryRequests: string; gpuRequests: string }>;

export type WorkbenchTableDescriptorContext = {
  nodeResourceUsageEnabled: Ref<boolean>;
  nodeAllocations: Ref<NodeAllocSnapshot>;
  namespaceOptions: Ref<NamespaceItem[]>;
  pods: Ref<PodItem[]>;
  deployments: Ref<DeploymentItem[]>;
  services: Ref<ServiceItem[]>;
  statefulSets: Ref<StatefulSetItem[]>;
  configMaps: Ref<ConfigMapItem[]>;
  secrets: Ref<SecretItem[]>;
  serviceAccounts: Ref<ServiceAccountItem[]>;
  roles: Ref<RoleItem[]>;
  roleBindings: Ref<RoleBindingItem[]>;
  clusterRoles: Ref<ClusterRoleItem[]>;
  clusterRoleBindings: Ref<ClusterRoleBindingItem[]>;
  daemonSets: Ref<DaemonSetItem[]>;
  nodes: Ref<NodeItem[]>;
  persistentVolumeClaims: Ref<PersistentVolumeClaimItem[]>;
  persistentVolumes: Ref<PersistentVolumeItem[]>;
  storageClasses: Ref<StorageClassItem[]>;
  endpoints: Ref<EndpointsItem[]>;
  endpointSlices: Ref<EndpointSliceItem[]>;
  replicaSets: Ref<ReplicaSetItem[]>;
  jobs: Ref<JobItem[]>;
  cronJobs: Ref<CronJobItem[]>;
  ingresses: Ref<IngressItem[]>;
  ingressClasses: Ref<IngressClassItem[]>;
  networkPolicies: Ref<NetworkPolicyItem[]>;
  resourceQuotas: Ref<ResourceQuotaItem[]>;
  limitRanges: Ref<LimitRangeItem[]>;
  priorityClasses: Ref<PriorityClassItem[]>;
  horizontalPodAutoscalers: Ref<HorizontalPodAutoscalerItem[]>;
  podDisruptionBudgets: Ref<PodDisruptionBudgetItem[]>;
};

const ROW_BUILDERS: Record<ResourceKind, (ctx: WorkbenchTableDescriptorContext) => Record<string, unknown>[]> = {
  "namespaces": (ctx) => {
    return ctx.namespaceOptions.value.map((n) => ({ name: n.name, creationTime: n.creation_time ?? "-" }));
  },
  "nodes": (ctx) => {
    return ctx.nodes.value.map((n) => ({
      name: n.name,
      status: n.status ?? "-",
      taints: typeof n.taint_count === "number" ? (n.taint_count > 0 ? `${n.taint_count}` : "无") : "-",
      internalIp: n.internal_ip ?? "-",
      cpuTotal: n.cpu_total ?? "-",
      memoryTotal: n.memory_total ?? "-",
      gpuTotal: n.gpu_total ?? "-",
      cpuRequests: ctx.nodeResourceUsageEnabled.value
        ? (ctx.nodeAllocations.value[n.name]?.cpuRequests ?? n.cpu_requests ?? "-")
        : "-",
      memoryRequests: ctx.nodeResourceUsageEnabled.value
        ? (ctx.nodeAllocations.value[n.name]?.memoryRequests ?? n.memory_requests ?? "-")
        : "-",
      gpuRequests: ctx.nodeResourceUsageEnabled.value
        ? (ctx.nodeAllocations.value[n.name]?.gpuRequests ?? n.gpu_requests ?? "-")
        : "-",
      creationTime: n.creation_time ?? "-",
    }));
  },
  "pods": (ctx) => {
    return ctx.pods.value.map((p) => ({
      name: p.name,
      ns: p.namespace,
      phase: p.phase ?? "-",
      containerStatus: p.container_status ?? "-",
      podIp: p.pod_ip ?? "-",
      node: p.node_name ?? "-",
      creationTime: p.creation_time ?? "-",
    }));
  },
  "deployments": (ctx) => {
    return ctx.deployments.value.map((d) => ({
      name: d.name,
      ns: d.namespace,
      replicas: `${d.ready ?? 0}/${d.replicas ?? 0}`,
      creationTime: d.creation_time ?? "-",
      labelSelector: d.label_selector ?? null,
      podRollup: d.pod_rollup ?? null,
      recentRestart: d.pod_rollup?.last_container_restart ?? "-",
    }));
  },
  "services": (ctx) => {
    return ctx.services.value.map((s) => ({
      name: s.name,
      ns: s.namespace,
      type: s.service_type ?? "-",
      clusterIp: s.cluster_ip ?? "-",
      ports: s.ports ?? "-",
      creationTime: s.creation_time ?? "-",
    }));
  },
  "statefulsets": (ctx) => {
    return ctx.statefulSets.value.map((st) => ({
      name: st.name,
      ns: st.namespace,
      replicas: `${st.ready ?? 0}/${st.replicas ?? 0}`,
      creationTime: st.creation_time ?? "-",
      labelSelector: st.label_selector ?? null,
      podRollup: st.pod_rollup ?? null,
      recentRestart: st.pod_rollup?.last_container_restart ?? "-",
    }));
  },
  "configmaps": (ctx) => {
    return ctx.configMaps.value.map((c) => ({
      name: c.name,
      ns: c.namespace,
      keys: c.keys != null ? String(c.keys) : "-",
      creationTime: c.creation_time ?? "-",
    }));
  },
  "secrets": (ctx) => {
    return ctx.secrets.value.map((s) => ({
      name: s.name,
      ns: s.namespace,
      type: s.type_ ?? "-",
      keys: s.keys != null ? String(s.keys) : "-",
      creationTime: s.creation_time ?? "-",
    }));
  },
  "serviceaccounts": (ctx) => {
    return ctx.serviceAccounts.value.map((s) => ({
      name: s.name,
      ns: s.namespace,
      creationTime: s.creation_time ?? "-",
    }));
  },
  "roles": (ctx) => {
    return ctx.roles.value.map((r) => ({
      name: r.name,
      ns: r.namespace,
      creationTime: r.creation_time ?? "-",
    }));
  },
  "rolebindings": (ctx) => {
    return ctx.roleBindings.value.map((r) => ({
      name: r.name,
      ns: r.namespace,
      roleRef: r.role_ref ?? "-",
      roleRefKind: r.role_ref_kind ?? null,
      roleRefName: r.role_ref_name ?? null,
      subjects: r.subjects != null ? String(r.subjects) : "-",
      subjectsList: r.subjects_list ?? null,
      creationTime: r.creation_time ?? "-",
    }));
  },
  "clusterroles": (ctx) => {
    return ctx.clusterRoles.value.map((r) => ({
      name: r.name,
      creationTime: r.creation_time ?? "-",
    }));
  },
  "clusterrolebindings": (ctx) => {
    return ctx.clusterRoleBindings.value.map((r) => ({
      name: r.name,
      roleRef: r.role_ref ?? "-",
      roleRefKind: r.role_ref_kind ?? null,
      roleRefName: r.role_ref_name ?? null,
      subjects: r.subjects != null ? String(r.subjects) : "-",
      subjectsList: r.subjects_list ?? null,
      creationTime: r.creation_time ?? "-",
    }));
  },
  "daemonsets": (ctx) => {
    return ctx.daemonSets.value.map((d) => ({
      name: d.name,
      ns: d.namespace,
      replicas: `${d.ready ?? 0}/${d.desired ?? 0}`,
      creationTime: d.creation_time ?? "-",
      labelSelector: d.label_selector ?? null,
      podRollup: d.pod_rollup ?? null,
      recentRestart: d.pod_rollup?.last_container_restart ?? "-",
    }));
  },
  "persistentvolumeclaims": (ctx) => {
    return ctx.persistentVolumeClaims.value.map((p) => ({
      name: p.name,
      ns: p.namespace,
      status: p.status ?? "-",
      volume: p.volume ?? "-",
      capacity: p.capacity ?? "-",
      storageClass: p.storage_class ?? "-",
      creationTime: p.creation_time ?? "-",
    }));
  },
  "persistentvolumes": (ctx) => {
    return ctx.persistentVolumes.value.map((p) => ({
      name: p.name,
      capacity: p.capacity ?? "-",
      status: p.status ?? "-",
      creationTime: p.creation_time ?? "-",
    }));
  },
  "storageclasses": (ctx) => {
    return ctx.storageClasses.value.map((s) => ({
      name: s.name,
      provisioner: s.provisioner ?? "-",
      allowVolumeExpansion:
        s.allow_volume_expansion == null ? "-" : s.allow_volume_expansion ? "是" : "否",
      creationTime: s.creation_time ?? "-",
    }));
  },
  "endpoints": (ctx) => {
    return ctx.endpoints.value.map((e) => ({
      name: e.name,
      ns: e.namespace,
      subsets: e.subsets != null ? String(e.subsets) : "-",
      creationTime: e.creation_time ?? "-",
    }));
  },
  "endpointslices": (ctx) => {
    return ctx.endpointSlices.value.map((e) => ({
      name: e.name,
      ns: e.namespace,
      addressType: e.address_type ?? "-",
      endpoints: e.endpoints != null ? String(e.endpoints) : "-",
      creationTime: e.creation_time ?? "-",
    }));
  },
  "replicasets": (ctx) => {
    return ctx.replicaSets.value.map((r) => ({
      name: r.name,
      ns: r.namespace,
      replicas: `${r.ready ?? 0}/${r.replicas ?? 0}`,
      creationTime: r.creation_time ?? "-",
      labelSelector: r.label_selector ?? null,
    }));
  },
  "jobs": (ctx) => {
    return ctx.jobs.value.map((j) => ({
      name: j.name,
      ns: j.namespace,
      completions: j.completions ?? "-",
      duration: j.duration ?? "-",
      creationTime: j.creation_time ?? "-",
    }));
  },
  "cronjobs": (ctx) => {
    return ctx.cronJobs.value.map((c) => ({
      name: c.name,
      ns: c.namespace,
      schedule: c.schedule ?? "-",
      lastSchedule: c.last_schedule ?? "-",
      creationTime: c.creation_time ?? "-",
    }));
  },
  "ingresses": (ctx) => {
    return ctx.ingresses.value.map((i) => ({
      name: i.name,
      ns: i.namespace,
      class: i.class ?? "-",
      hosts: i.hosts ?? "-",
      creationTime: i.creation_time ?? "-",
    }));
  },
  "ingressclasses": (ctx) => {
    return ctx.ingressClasses.value.map((i) => ({
      name: i.name,
      controller: i.controller ?? "-",
      creationTime: i.creation_time ?? "-",
    }));
  },
  "networkpolicies": (ctx) => {
    return ctx.networkPolicies.value.map((n) => ({
      name: n.name,
      ns: n.namespace,
      creationTime: n.creation_time ?? "-",
    }));
  },
  "resourcequotas": (ctx) => {
    return ctx.resourceQuotas.value.map((r) => ({
      name: r.name,
      ns: r.namespace,
      creationTime: r.creation_time ?? "-",
    }));
  },
  "limitranges": (ctx) => {
    return ctx.limitRanges.value.map((l) => ({
      name: l.name,
      ns: l.namespace,
      creationTime: l.creation_time ?? "-",
    }));
  },
  "priorityclasses": (ctx) => {
    return ctx.priorityClasses.value.map((p) => ({
      name: p.name,
      value: p.value != null ? String(p.value) : "-",
      creationTime: p.creation_time ?? "-",
    }));
  },
  "horizontalpodautoscalers": (ctx) => {
    return ctx.horizontalPodAutoscalers.value.map((h) => ({
      name: h.name,
      ns: h.namespace,
      reference: h.reference ?? "-",
      replicas: h.replicas ?? "-",
      creationTime: h.creation_time ?? "-",
    }));
  },
  "poddisruptionbudgets": (ctx) => {
    return ctx.podDisruptionBudgets.value.map((p) => ({
      name: p.name,
      ns: p.namespace,
      minAvailable: p.min_available ?? "-",
      maxUnavailable: p.max_unavailable ?? "-",
      allowedDisruptions: p.allowed_disruptions ?? "-",
      creationTime: p.creation_time ?? "-",
    }));
  },
};

const COLUMN_BUILDERS: Record<ResourceKind, (ctx: WorkbenchTableDescriptorContext) => WorkbenchTableColumn[]> = {
  "namespaces": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "nodes": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "status", label: "状态" },
      { key: "taints", label: "污点" },
      { key: "internalIp", label: "Internal IP" },
      ...(_ctx.nodeResourceUsageEnabled.value
        ? [
            { key: "cpuRequests", label: "CPU 分配/总量" },
            { key: "memoryRequests", label: "内存分配/总量" },
            { key: "gpuRequests", label: "GPU 分配/总量" },
          ]
        : [
            { key: "cpuTotal", label: "CPU 总量" },
            { key: "memoryTotal", label: "内存总量" },
            { key: "gpuTotal", label: "GPU 总量" },
          ]),
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "pods": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "phase", label: "状态" },
      { key: "containerStatus", label: "容器启动" },
      { key: "podIp", label: "Pod IP" },
      { key: "node", label: "Node" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "deployments": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "replicas", label: "副本" },
      { key: "podRollup", label: "Pod 态势" },
      { key: "recentRestart", label: "最近重启" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "services": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "type", label: "Type" },
      { key: "clusterIp", label: "Cluster IP" },
      { key: "ports", label: "Ports" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "statefulsets": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "replicas", label: "副本" },
      { key: "podRollup", label: "Pod 态势" },
      { key: "recentRestart", label: "最近重启" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "configmaps": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "keys", label: "Keys" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "secrets": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "type", label: "Type" },
      { key: "keys", label: "Keys" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "serviceaccounts": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "roles": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "rolebindings": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "roleRef", label: "Role" },
      { key: "subjects", label: "Subjects" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "clusterroles": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "clusterrolebindings": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "roleRef", label: "Role" },
      { key: "subjects", label: "Subjects" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "daemonsets": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "replicas", label: "Ready/Desired" },
      { key: "podRollup", label: "Pod 态势" },
      { key: "recentRestart", label: "最近重启" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "persistentvolumeclaims": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "status", label: "Status" },
      { key: "volume", label: "Volume" },
      { key: "capacity", label: "Capacity" },
      { key: "storageClass", label: "StorageClass" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "persistentvolumes": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "capacity", label: "Capacity" },
      { key: "status", label: "Status" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "storageclasses": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "provisioner", label: "Provisioner" },
      { key: "allowVolumeExpansion", label: "允许扩容" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "endpoints": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "subsets", label: "Subsets" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "endpointslices": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "addressType", label: "AddressType" },
      { key: "endpoints", label: "Endpoints" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "replicasets": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "replicas", label: "副本" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "jobs": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "completions", label: "完成" },
      { key: "duration", label: "耗时" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "cronjobs": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "schedule", label: "Schedule" },
      { key: "lastSchedule", label: "上次调度" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "ingresses": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "class", label: "Class" },
      { key: "hosts", label: "Hosts" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "ingressclasses": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "controller", label: "Controller" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "networkpolicies": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "resourcequotas": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "limitranges": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "priorityclasses": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "value", label: "Value" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "horizontalpodautoscalers": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "reference", label: "Target" },
      { key: "replicas", label: "副本" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
  "poddisruptionbudgets": (_ctx) => {
    return [
      { key: "name", label: "名称" },
      { key: "ns", label: "Namespace" },
      { key: "minAvailable", label: "Min Available" },
      { key: "maxUnavailable", label: "Max Unavailable" },
      { key: "allowedDisruptions", label: "Allowed" },
      { key: "creationTime", label: "创建时间" },
    ];
  },
};

export function buildRowsForKind(kind: ResourceKind, ctx: WorkbenchTableDescriptorContext): Record<string, unknown>[] {
  return ROW_BUILDERS[kind](ctx);
}

export function buildColumnsForKind(kind: ResourceKind, ctx: WorkbenchTableDescriptorContext): WorkbenchTableColumn[] {
  return COLUMN_BUILDERS[kind](ctx);
}
