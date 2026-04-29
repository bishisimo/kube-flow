<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NCheckbox, NCheckboxGroup, NInput, NSelect, NSpace } from "naive-ui";
import { kfSpace } from "../../kf";
import BaseModal from "../base/BaseModal.vue";
import { formatDateTime } from "../../utils/dateFormat";
import { extractErrorMessage } from "../../utils/errorMessage";
import { appSettingsGetResourceDeployStrategy } from "../../api/config";
import { kubeDeployResource, kubeGetResource } from "../../api/kube";
import { parseKubeObject } from "../../utils/yaml";
import { createResourceSnapshot, summarizeResourceYaml } from "../../stores/resourceSnapshots";
import { ensureAutoSnapshotSettingLoaded } from "../../stores/appSettings";
import {
  useOrchestratorPackagesStore,
  type OrchestratorPackage,
  type OrchestratorPackageVersion,
  type OrchestratorPackageResourceSnapshot,
} from "../../stores/orchestratorPackages";
import { useOrchestratorStore } from "../../stores/orchestrator";

const props = defineProps<{
  selectedEnvId: string;
  environments: Array<{ id: string; display_name: string }>;
  components: string[];
  manifestsByEnv: Array<{
    id: string;
    env_id: string;
    component: string;
    resource_kind: string;
    resource_name: string;
    resource_namespace: string | null;
  }>;
}>();

const emit = defineEmits<{
  opMessage: [msg: string];
  opError: [msg: string];
}>();

const {
  packages,
  createPackage,
  deletePackage,
  createPackageVersion,
  setPackageVersionTag,
  deletePackageVersion,
  syncPackageVersionToEnv,
  recordPackageDeployment,
} = useOrchestratorPackagesStore();
const { manifests, saveManifestYaml } = useOrchestratorStore();

const selectedPackageId = ref("");
const selectedPackageVersionId = ref("");
const packageNameInput = ref("");
const packageDescriptionInput = ref("");
const packageComponentDraft = ref<string[]>([]);
const packageTargetEnvId = ref("");
const packageOverwrite = ref(true);
const packageWorking = ref(false);
const packageActionDialogVisible = ref(false);
const packageActionMode = ref<"sync" | "apply">("sync");
const packageDeleteDialogVisible = ref(false);
const versionDeleteDialogVisible = ref(false);
const editingVersionTagId = ref("");
const editingVersionTagValue = ref("");

const selectedPackage = computed<OrchestratorPackage | null>(
  () => packages.value.find((p) => p.id === selectedPackageId.value) ?? null
);
const selectedPackageVersion = computed<OrchestratorPackageVersion | null>(
  () => selectedPackage.value?.versions.find((v) => v.id === selectedPackageVersionId.value) ?? null
);
const packageDeployments = computed(() => selectedPackage.value?.deployments ?? []);
const packageDraftComponents = computed(() =>
  props.components.map((name) => ({
    name,
    count: props.manifestsByEnv.filter((m) => m.component === name).length,
  }))
);
const packageEnvOptions = computed(() =>
  props.environments.map((e) => ({ label: e.display_name, value: e.id }))
);
const canCreatePackageVersion = computed(
  () => Boolean(selectedPackage.value && props.selectedEnvId && packageComponentDraft.value.length)
);
const canOpenPackageActionDialog = computed(
  () => Boolean(selectedPackageVersion.value && props.environments.length)
);
const canOperatePackageDeploy = computed(
  () => Boolean(selectedPackageVersion.value && packageTargetEnvId.value)
);
const selectedPackageStats = computed(() => {
  const pkg = selectedPackage.value;
  if (!pkg) return { versions: 0, resources: 0 };
  const resources = pkg.versions.reduce((sum, v) => sum + v.resources.length, 0);
  return { versions: pkg.versions.length, resources };
});

watch(
  () => [packages.value.map((p) => p.id).join(","), selectedPackageId.value] as const,
  () => {
    if (!packages.value.length) {
      selectedPackageId.value = "";
      selectedPackageVersionId.value = "";
      return;
    }
    if (!selectedPackageId.value || !packages.value.some((p) => p.id === selectedPackageId.value)) {
      selectedPackageId.value = packages.value[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => [selectedPackage.value?.id ?? "", selectedPackage.value?.versions.map((v) => v.id).join(",") ?? ""] as const,
  () => {
    const versions = selectedPackage.value?.versions ?? [];
    if (!versions.length) {
      selectedPackageVersionId.value = "";
      return;
    }
    if (!selectedPackageVersionId.value || !versions.some((v) => v.id === selectedPackageVersionId.value)) {
      selectedPackageVersionId.value = versions[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => [props.selectedEnvId, props.environments.map((e) => e.id).join(",")] as const,
  () => {
    const candidates = props.environments;
    if (!candidates.length) {
      packageTargetEnvId.value = "";
      return;
    }
    if (!packageTargetEnvId.value || !candidates.some((e) => e.id === packageTargetEnvId.value)) {
      packageTargetEnvId.value = candidates[0].id;
    }
  },
  { immediate: true }
);

function resourceIdentityKey(kind: string, name: string, namespace: string | null) {
  return `${kind}|${namespace ?? ""}|${name}`;
}

function isWebhookConfigurationKind(kind: string) {
  return kind === "MutatingWebhookConfiguration" || kind === "ValidatingWebhookConfiguration";
}

function isWorkloadKind(kind: string) {
  return ["Deployment", "StatefulSet", "DaemonSet", "ReplicaSet", "Job", "CronJob", "Pod"].includes(kind);
}

function extractWebhookServiceKeys(yaml: string): string[] {
  try {
    const obj = parseKubeObject(yaml)?.raw ?? null;
    if (!obj) return [];
    const webhooks = Array.isArray(obj.webhooks) ? (obj.webhooks as Array<Record<string, unknown>>) : [];
    const keys: string[] = [];
    for (const webhook of webhooks) {
      const clientConfig =
        webhook.clientConfig && typeof webhook.clientConfig === "object"
          ? (webhook.clientConfig as Record<string, unknown>)
          : null;
      const service =
        clientConfig?.service && typeof clientConfig.service === "object"
          ? (clientConfig.service as Record<string, unknown>)
          : null;
      if (!service) continue;
      const name = typeof service.name === "string" ? service.name.trim() : "";
      if (!name) continue;
      const namespace = typeof service.namespace === "string" && service.namespace.trim() ? service.namespace.trim() : "default";
      keys.push(resourceIdentityKey("Service", name, namespace));
    }
    return keys;
  } catch {
    return [];
  }
}

function buildDelayedWebhookKeys<T extends { resource_kind: string; resource_name: string; resource_namespace: string | null; yaml: string }>(
  list: T[]
): Set<string> {
  const serviceKeys = new Set(
    list
      .filter((item) => item.resource_kind === "Service")
      .map((item) => resourceIdentityKey("Service", item.resource_name, item.resource_namespace || "default"))
  );
  const workloadNamespaces = new Set(
    list
      .filter((item) => isWorkloadKind(item.resource_kind))
      .map((item) => item.resource_namespace || "default")
  );
  const delayed = new Set<string>();
  for (const item of list) {
    if (!isWebhookConfigurationKind(item.resource_kind)) continue;
    const targetServiceKeys = extractWebhookServiceKeys(item.yaml);
    const shouldDelay = targetServiceKeys.some((svcKey) => {
      if (!serviceKeys.has(svcKey)) return false;
      const namespace = svcKey.split("|")[1] || "default";
      return workloadNamespaces.has(namespace);
    });
    if (shouldDelay) {
      delayed.add(resourceIdentityKey(item.resource_kind, item.resource_name, item.resource_namespace));
    }
  }
  return delayed;
}

const APPLY_ORDER: Record<string, number> = {
  CustomResourceDefinition: 0,
  Namespace: 1,
  PriorityClass: 2,
  StorageClass: 3,
  IngressClass: 4,
  ServiceAccount: 10,
  Role: 11,
  ClusterRole: 12,
  RoleBinding: 13,
  ClusterRoleBinding: 14,
  ConfigMap: 20,
  Secret: 21,
  PersistentVolume: 22,
  PersistentVolumeClaim: 23,
  ResourceQuota: 24,
  LimitRange: 25,
  MutatingWebhookConfiguration: 26,
  ValidatingWebhookConfiguration: 27,
  NetworkPolicy: 28,
  Service: 30,
  Endpoints: 31,
  EndpointSlice: 32,
  Deployment: 40,
  StatefulSet: 41,
  DaemonSet: 42,
  ReplicaSet: 43,
  Job: 44,
  CronJob: 45,
  Pod: 46,
  Ingress: 50,
  HorizontalPodAutoscaler: 51,
  PodDisruptionBudget: 52,
};

function applyWeight(kind: string, delayedWebhook = false) {
  if (delayedWebhook && isWebhookConfigurationKind(kind)) return 49;
  return APPLY_ORDER[kind] ?? 999;
}

function sortPackageResources(list: OrchestratorPackageResourceSnapshot[]): OrchestratorPackageResourceSnapshot[] {
  const delayedWebhookKeys = buildDelayedWebhookKeys(
    list.map((item) => ({
      resource_kind: item.resource_kind,
      resource_name: item.resource_name,
      resource_namespace: item.resource_namespace,
      yaml: item.yaml,
    }))
  );
  return [...list].sort((a, b) => {
    const wa = applyWeight(
      a.resource_kind,
      delayedWebhookKeys.has(resourceIdentityKey(a.resource_kind, a.resource_name, a.resource_namespace))
    );
    const wb = applyWeight(
      b.resource_kind,
      delayedWebhookKeys.has(resourceIdentityKey(b.resource_kind, b.resource_name, b.resource_namespace))
    );
    if (wa !== wb) return wa - wb;
    if (a.component !== b.component) return a.component.localeCompare(b.component);
    if ((a.resource_namespace || "") !== (b.resource_namespace || "")) {
      return (a.resource_namespace || "").localeCompare(b.resource_namespace || "");
    }
    return a.resource_name.localeCompare(b.resource_name);
  });
}

function findManifestIdForPackageResource(
  targetEnvId: string,
  resource: OrchestratorPackageResourceSnapshot
): string | null {
  const found = manifests.value.find(
    (m) =>
      m.env_id === targetEnvId &&
      m.component === resource.component &&
      m.resource_kind === resource.resource_kind &&
      m.resource_name === resource.resource_name &&
      (m.resource_namespace ?? null) === (resource.resource_namespace ?? null)
  );
  return found?.id ?? null;
}

function validatePackageVersion(version: OrchestratorPackageVersion): string[] {
  const errs: string[] = [];
  const keys = new Set<string>();
  for (const r of version.resources) {
    const key = `${r.component}|${r.resource_kind}|${r.resource_namespace ?? ""}|${r.resource_name}`;
    if (keys.has(key)) errs.push(`存在重复资源：${r.component} / ${r.resource_kind}/${r.resource_name}`);
    keys.add(key);
  }
  return errs;
}

async function deployYamlToEnv(envId: string, yaml: string) {
  const strategy = await appSettingsGetResourceDeployStrategy();
  await kubeDeployResource(envId, yaml, strategy);
}

function onCreatePackage() {
  try {
    const pkg = createPackage(packageNameInput.value, packageDescriptionInput.value);
    selectedPackageId.value = pkg.id;
    packageNameInput.value = "";
    packageDescriptionInput.value = "";
    emit("opMessage", `已创建应用包：${pkg.name}`);
  } catch (e) {
    emit("opError", extractErrorMessage(e));
  }
}

function openDeletePackageDialog() {
  if (!selectedPackage.value) return;
  packageDeleteDialogVisible.value = true;
}

function closeDeletePackageDialog() {
  if (packageWorking.value) return;
  packageDeleteDialogVisible.value = false;
}

function onDeletePackage() {
  if (!selectedPackage.value) return;
  const ok = deletePackage(selectedPackage.value.id);
  packageDeleteDialogVisible.value = false;
  if (ok) {
    emit("opMessage", "应用包已删除。");
    return;
  }
  emit("opError", "删除应用包失败。");
}

function openDeleteVersionDialog() {
  if (!selectedPackage.value || !selectedPackageVersion.value) return;
  versionDeleteDialogVisible.value = true;
}

function closeDeleteVersionDialog() {
  if (packageWorking.value) return;
  versionDeleteDialogVisible.value = false;
}

function onDeletePackageVersion() {
  if (!selectedPackage.value || !selectedPackageVersion.value) return;
  const ok = deletePackageVersion(selectedPackage.value.id, selectedPackageVersion.value.id);
  versionDeleteDialogVisible.value = false;
  if (ok) {
    emit("opMessage", "版本已删除。");
    return;
  }
  emit("opError", "删除版本失败。");
}

function startEditVersionTag(version: OrchestratorPackageVersion) {
  editingVersionTagId.value = version.id;
  editingVersionTagValue.value = version.tag ?? "";
}

function cancelEditVersionTag() {
  editingVersionTagId.value = "";
  editingVersionTagValue.value = "";
}

function onSaveVersionTag(versionId: string) {
  if (!selectedPackage.value) return;
  const ok = setPackageVersionTag(selectedPackage.value.id, versionId, editingVersionTagValue.value);
  if (!ok) {
    emit("opError", "保存版本 Tag 失败。");
    return;
  }
  cancelEditVersionTag();
  emit("opMessage", "版本 Tag 已更新。");
}

function onCreatePackageVersion() {
  if (!selectedPackage.value || !props.selectedEnvId) return;
  const env = props.environments.find((e) => e.id === props.selectedEnvId);
  if (!env) return;
  try {
    const version = createPackageVersion(
      selectedPackage.value.id,
      props.selectedEnvId,
      env.display_name,
      packageComponentDraft.value
    );
    selectedPackageVersionId.value = version.id;
    emit("opMessage", `已生成版本 ${version.label}（${version.resources.length} 个资源）。`);
  } catch (e) {
    emit("opError", extractErrorMessage(e));
  }
}

function openPackageActionDialog(mode: "sync" | "apply") {
  if (!canOpenPackageActionDialog.value || packageWorking.value) return;
  packageActionMode.value = mode;
  packageActionDialogVisible.value = true;
}

function closePackageActionDialog() {
  if (packageWorking.value) return;
  packageActionDialogVisible.value = false;
}

async function onSyncPackageToEnv() {
  if (!selectedPackage.value || !selectedPackageVersion.value || !canOperatePackageDeploy.value) return;
  const target = props.environments.find((e) => e.id === packageTargetEnvId.value);
  if (!target) return;
  const precheckErrors = validatePackageVersion(selectedPackageVersion.value);
  if (precheckErrors.length) {
    emit("opError", `预检失败：${precheckErrors.join("；")}`);
    return;
  }
  packageWorking.value = true;
  try {
    const result = syncPackageVersionToEnv(
      selectedPackage.value.id,
      selectedPackageVersion.value.id,
      target.id,
      target.display_name,
      packageOverwrite.value
    );
    recordPackageDeployment(
      selectedPackage.value.id,
      selectedPackageVersion.value.id,
      target.id,
      target.display_name,
      "sync",
      result.copied + result.updated,
      0,
      []
    );
    emit("opMessage", `已同步到 ${target.display_name}：新增 ${result.copied}，更新 ${result.updated}，跳过 ${result.skipped}`);
  } catch (e) {
    emit("opError", extractErrorMessage(e));
  } finally {
    packageWorking.value = false;
  }
}

async function onApplyPackageToEnv() {
  if (!selectedPackage.value || !selectedPackageVersion.value || !canOperatePackageDeploy.value) return;
  const target = props.environments.find((e) => e.id === packageTargetEnvId.value);
  if (!target) return;
  const precheckErrors = validatePackageVersion(selectedPackageVersion.value);
  if (precheckErrors.length) {
    emit("opError", `预检失败：${precheckErrors.join("；")}`);
    return;
  }
  packageWorking.value = true;
  const version = selectedPackageVersion.value;
  try {
    syncPackageVersionToEnv(
      selectedPackage.value.id,
      version.id,
      target.id,
      target.display_name,
      packageOverwrite.value
    );
    const resources = sortPackageResources(version.resources);
    const errors: string[] = [];
    let success = 0;
    const autoSnapshotEnabled = await ensureAutoSnapshotSettingLoaded();
    for (const r of resources) {
      try {
        if (autoSnapshotEnabled) {
          try {
            const liveYaml = await kubeGetResource(target.id, r.resource_kind, r.resource_name, r.resource_namespace);
            if (liveYaml) {
              createResourceSnapshot(
                { env_id: target.id, resource_kind: r.resource_kind, resource_name: r.resource_name, resource_namespace: r.resource_namespace },
                { yaml: liveYaml, category: "resource", source: "before-apply", title: "应用前资源快照", summary: summarizeResourceYaml(liveYaml) }
              );
            }
          } catch {
            // 资源不存在或获取失败，无需快照
          }
        }
        await deployYamlToEnv(target.id, r.yaml);
        const mid = findManifestIdForPackageResource(target.id, r);
        if (mid) saveManifestYaml(mid, r.yaml, "apply");
        success += 1;
      } catch (e) {
        errors.push(`${r.component}/${r.resource_kind}/${r.resource_name}: ${extractErrorMessage(e)}`);
      }
    }
    recordPackageDeployment(
      selectedPackage.value.id,
      version.id,
      target.id,
      target.display_name,
      "apply",
      success,
      errors.length,
      errors
    );
    if (errors.length) {
      emit("opError", `应用包部分失败：${errors.join("；")}`);
    } else {
      emit("opMessage", `应用包 ${selectedPackage.value.name}@${version.label} 已发布到 ${target.display_name}。`);
    }
  } catch (e) {
    emit("opError", extractErrorMessage(e));
  } finally {
    packageWorking.value = false;
  }
}

async function onConfirmPackageAction() {
  if (!canOperatePackageDeploy.value || packageWorking.value) return;
  packageActionDialogVisible.value = false;
  if (packageActionMode.value === "sync") {
    await onSyncPackageToEnv();
    return;
  }
  await onApplyPackageToEnv();
}
</script>

<template>
  <div class="pkg-layout">
    <aside class="pkg-side">
      <div class="pkg-panel pkg-create-panel">
        <div class="pkg-side-title">应用包管理</div>
        <div class="pkg-create">
          <NInput v-model:value="packageNameInput" class="pkg-input-naive" size="small" placeholder="新应用包名称" />
          <NInput
            v-model:value="packageDescriptionInput"
            class="pkg-input-naive"
            size="small"
            placeholder="描述（可选）"
          />
          <NButton
            type="primary"
            class="btn-package-create-naive"
            :disabled="!packageNameInput.trim()"
            @click="onCreatePackage"
          >
            创建应用包
          </NButton>
        </div>
        <div class="pkg-side-subtitle">共 {{ packages.length }} 个应用包</div>
      </div>
      <div class="pkg-panel pkg-list-panel">
        <div class="pkg-side-title">应用包列表</div>
        <div
          v-for="pkg in packages"
          :key="pkg.id"
          class="pkg-item"
          :class="{ active: selectedPackageId === pkg.id }"
          @click="selectedPackageId = pkg.id"
        >
          <div class="pkg-item-title">{{ pkg.name }}</div>
          <div class="pkg-item-sub">{{ pkg.description || "无描述" }}</div>
          <div class="pkg-item-meta">版本 {{ pkg.versions.length }} · 更新于 {{ formatDateTime(pkg.updated_at) }}</div>
        </div>
        <div v-if="!packages.length" class="empty">暂无应用包</div>
      </div>
    </aside>

    <section class="pkg-main">
      <template v-if="selectedPackage">
        <div class="pkg-head pkg-panel">
          <div>
            <div class="pkg-name">{{ selectedPackage.name }}</div>
            <div class="pkg-desc">{{ selectedPackage.description || "用于将多个组件打包后按版本发布到环境。" }}</div>
            <div class="pkg-summary-row">
              <span class="pkg-summary-pill">版本 {{ selectedPackageStats.versions }}</span>
              <span class="pkg-summary-pill">资源快照 {{ selectedPackageStats.resources }}</span>
              <span class="pkg-summary-pill">更新于 {{ formatDateTime(selectedPackage.updated_at) }}</span>
            </div>
          </div>
          <NButton type="error" @click="openDeletePackageDialog">删除应用包</NButton>
        </div>

        <div class="pkg-main-grid">
          <div class="pkg-compose pkg-panel">
            <div class="pkg-block-title">版本构建（来源：当前环境）</div>
            <NCheckboxGroup v-model:value="packageComponentDraft" class="pkg-comp-list-naive">
              <NCheckbox
                v-for="item in packageDraftComponents"
                :key="item.name"
                :value="item.name"
                class="pkg-check-naive"
              >
                <span class="pkg-check-name">{{ item.name }}</span>
                <small class="pkg-check-count">{{ item.count }} 资源</small>
              </NCheckbox>
            </NCheckboxGroup>
            <NButton
              class="btn-package-version-naive"
              type="primary"
              :disabled="!canCreatePackageVersion"
              @click="onCreatePackageVersion"
            >
              生成新版本
            </NButton>
          </div>

          <div class="pkg-versions pkg-panel">
            <div class="pkg-block-title">版本列表</div>
            <div class="pkg-version-list">
              <div
                v-for="v in selectedPackage.versions"
                :key="v.id"
                class="pkg-version-item"
                :class="{ active: selectedPackageVersionId === v.id }"
                @click="selectedPackageVersionId = v.id"
              >
                <div class="pkg-version-title-row">
                  <span>{{ v.label }}</span>
                  <NButton
                    v-if="editingVersionTagId !== v.id"
                    quaternary
                    size="small"
                    class="version-tag-edit-btn-naive"
                    :title="v.tag ? '编辑 Tag' : '设置 Tag'"
                    :aria-label="v.tag ? '编辑 Tag' : '设置 Tag'"
                    @click.stop="startEditVersionTag(v)"
                  >
                    <span aria-hidden="true">🏷</span>
                  </NButton>
                </div>
                <div v-if="editingVersionTagId === v.id" class="version-inline-edit-row" @click.stop>
                  <NInput
                    v-model:value="editingVersionTagValue"
                    size="small"
                    class="pkg-input-naive version-inline-input-naive"
                    placeholder="输入正式 Tag，例如 prod-20260318"
                  />
                  <NButton
                    class="version-inline-btn-naive"
                    type="primary"
                    size="small"
                    @click.stop="onSaveVersionTag(v.id)"
                    >保存</NButton
                  >
                  <NButton
                    quaternary
                    size="small"
                    class="version-inline-btn-naive"
                    @click.stop="cancelEditVersionTag"
                    >取消</NButton
                  >
                </div>
                <div v-else class="version-tag-display">
                  <strong v-if="v.tag" class="version-tag">#{{ v.tag }}</strong>
                  <span v-else class="version-tag-empty">未设置 Tag</span>
                </div>
                <small>组件数 {{ v.component_names.length }} · 资源数 {{ v.resources.length }}</small>
                <small>{{ v.component_names.join(" / ") }}</small>
                <small>创建于 {{ formatDateTime(v.created_at) }}</small>
              </div>
              <div v-if="!selectedPackage.versions.length" class="empty">还没有版本，先选择组件生成一个版本。</div>
            </div>
          </div>
        </div>
      </template>
      <div v-else class="empty pkg-empty-card">请先创建或选择应用包</div>
    </section>

    <aside class="pkg-deploy">
      <template v-if="selectedPackageVersion">
        <div class="pkg-panel pkg-action-panel">
          <div class="pkg-block-title">发布操作</div>
          <div class="pkg-version-meta">
            <strong>{{ selectedPackage?.name }} @ {{ selectedPackageVersion.label }}</strong>
            <span v-if="selectedPackageVersion.tag">正式 Tag：#{{ selectedPackageVersion.tag }}</span>
            <span>来源环境：{{ selectedPackageVersion.source_env_name }}</span>
            <span>组件数：{{ selectedPackageVersion.component_names.length }}</span>
            <span>资源数：{{ selectedPackageVersion.resources.length }}</span>
            <span>组件：{{ selectedPackageVersion.component_names.join(" / ") }}</span>
          </div>
          <NSpace v-bind="kfSpace.compactActions">
            <NButton
              secondary
              :disabled="!canOpenPackageActionDialog || packageWorking"
              @click="openPackageActionDialog('sync')"
            >
              {{ packageWorking ? "处理中…" : "同步到环境…" }}
            </NButton>
            <NButton
              type="primary"
              :disabled="!canOpenPackageActionDialog || packageWorking"
              @click="openPackageActionDialog('apply')"
            >
              {{ packageWorking ? "发布中…" : "发布到环境…" }}
            </NButton>
          </NSpace>
          <div class="copy-tip">点击按钮后会弹出确认窗口，选择目标环境后再执行。</div>
          <NButton type="error" :disabled="packageWorking" @click="openDeleteVersionDialog">删除当前版本</NButton>
        </div>

        <div class="pkg-panel">
          <div class="pkg-block-title">资源清单</div>
          <div class="pkg-resource-list">
            <div v-for="r in selectedPackageVersion.resources" :key="r.id" class="pkg-resource-item">
              <span>{{ r.component }}</span>
              <span>{{ r.resource_kind }}/{{ r.resource_name }}</span>
              <small>{{ r.resource_namespace || "default" }}</small>
            </div>
          </div>
        </div>

        <div class="pkg-panel">
          <div class="pkg-block-title">发布记录</div>
          <div class="pkg-deploy-history">
            <div v-for="d in packageDeployments" :key="d.id" class="pkg-deploy-item">
              <span>{{ d.mode === "apply" ? "发布" : "同步" }} · {{ d.version_label }}</span>
              <small>{{ d.target_env_name }} · 成功 {{ d.success }} / 失败 {{ d.failed }}</small>
              <small>{{ formatDateTime(d.at) }}</small>
            </div>
            <div v-if="!packageDeployments.length" class="empty">暂无发布记录</div>
          </div>
        </div>
      </template>
      <div v-else class="empty pkg-empty-card">请选择应用包版本</div>
    </aside>
  </div>

  <BaseModal
    :visible="packageActionDialogVisible"
    :title="packageActionMode === 'apply' ? '发布到环境' : '同步到环境'"
    width="520px"
    @close="closePackageActionDialog"
  >
    <div class="package-action-body">
      <div class="pkg-version-meta">
        <strong>{{ selectedPackage?.name }} @ {{ selectedPackageVersion?.label }}</strong>
        <span>组件数：{{ selectedPackageVersion?.component_names.length ?? 0 }}</span>
        <span>资源数：{{ selectedPackageVersion?.resources.length ?? 0 }}</span>
      </div>
      <label class="field-label">
        <span>目标环境</span>
        <NSelect
          v-model:value="packageTargetEnvId"
          :options="packageEnvOptions"
          :disabled="packageWorking"
          filterable
          class="env-select-naive"
        />
      </label>
      <NCheckbox v-model:checked="packageOverwrite" :disabled="packageWorking" class="package-overwrite-naive">
        覆盖同名资源
      </NCheckbox>
      <div class="copy-tip">
        {{
          packageActionMode === "apply"
            ? "将先同步编排资产，再按顺序发布到集群。"
            : "仅同步到编排资产，不会直接写入集群。"
        }}
      </div>
    </div>
    <template #footer>
      <NButton secondary :disabled="packageWorking" @click="closePackageActionDialog">取消</NButton>
      <NButton
        type="primary"
        :disabled="!canOperatePackageDeploy || packageWorking"
        :loading="packageWorking"
        @click="onConfirmPackageAction"
      >
        {{ packageActionMode === "apply" ? "确认发布" : "确认同步" }}
      </NButton>
    </template>
  </BaseModal>

  <BaseModal
    :visible="packageDeleteDialogVisible"
    title="确认删除应用包"
    width="480px"
    @close="closeDeletePackageDialog"
  >
    <div class="package-action-body">
      <div class="copy-tip">
        将删除应用包
        <strong>{{ selectedPackage?.name }}</strong>，以及该应用包下所有版本和发布记录。
      </div>
    </div>
    <template #footer>
      <NButton secondary :disabled="packageWorking" @click="closeDeletePackageDialog">取消</NButton>
      <NButton type="error" :disabled="packageWorking" @click="onDeletePackage">确认删除</NButton>
    </template>
  </BaseModal>

  <BaseModal
    :visible="versionDeleteDialogVisible"
    title="确认删除版本"
    width="480px"
    @close="closeDeleteVersionDialog"
  >
    <div class="package-action-body">
      <div class="copy-tip">
        将删除版本
        <strong>{{ selectedPackageVersion?.label }}</strong>
        <span v-if="selectedPackageVersion?.tag">（#{{ selectedPackageVersion?.tag }}）</span>
        及其发布记录。
      </div>
    </div>
    <template #footer>
      <NButton secondary :disabled="packageWorking" @click="closeDeleteVersionDialog">取消</NButton>
      <NButton type="error" :disabled="packageWorking" @click="onDeletePackageVersion">确认删除</NButton>
    </template>
  </BaseModal>
</template>

<style scoped>
.pkg-input-naive {
  width: 100%;
  min-width: 0;
}
.btn-package-create-naive,
.btn-package-version-naive {
  margin-top: 0.1rem;
}
.pkg-comp-list-naive {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  margin-bottom: 0.5rem;
}
.pkg-check-naive {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.8rem;
}
.pkg-check-naive :deep(.n-checkbox__label) {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.pkg-check-name {
  font-weight: 500;
  color: #0f172a;
}
.pkg-check-count {
  color: #64748b;
  font-size: 0.75rem;
}
.version-tag-edit-btn-naive {
  width: 1.65rem;
  min-width: 1.65rem;
  height: 1.65rem;
  font-size: 0.78rem;
  padding: 0 !important;
  border-radius: 999px;
}
.version-inline-input-naive {
  flex: 1;
  min-width: 0;
}
.version-inline-edit-row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-wrap: wrap;
}
.version-inline-btn-naive {
  flex-shrink: 0;
}
.package-action-body {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  padding: 0.1rem 0 0.2rem;
}
.field-label {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.8rem;
  color: #334155;
}
.env-select-naive {
  width: 100%;
  min-width: 0;
}
.package-overwrite-naive {
  font-size: 0.8rem;
  color: #334155;
}
.copy-tip {
  font-size: 0.75rem;
  color: #64748b;
  line-height: 1.45;
}
</style>
