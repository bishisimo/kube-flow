<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as jsYaml from "js-yaml";
import { NAlert, NButton, NCard, NInput, NSpin } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";
import { extractErrorMessage } from "../utils/errorMessage";
import { kubeGetResource, kubePatchContainerImages } from "../api/kube";
import { createResourceSnapshot } from "../stores/resourceSnapshots";
import { ensureAutoSnapshotSettingLoaded } from "../stores/appSettings";

export interface ResourceRef {
  kind: string;
  name: string;
  namespace: string | null;
}

const props = defineProps<{
  visible: boolean;
  envId: string | null;
  resource: ResourceRef | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "success"): void;
}>();

const loading = ref(false);
const error = ref<string | null>(null);
const imagePatchSaving = ref(false);
const imagePatchError = ref<string | null>(null);
const imagePatchInfo = ref<string | null>(null);
const resourceYaml = ref("");
const containers = ref<
  { name: string; currentImage: string; imageName: string; imageTag: string }[]
>([]);

const modalTitle = computed(() =>
  props.resource ? `修改镜像 · ${props.resource.kind} / ${props.resource.name}` : "修改镜像"
);

const snapshotResourceRef = computed(() =>
  props.envId && props.resource
    ? {
        env_id: props.envId,
        resource_kind: props.resource.kind,
        resource_name: props.resource.name,
        resource_namespace: props.resource.namespace ?? null,
      }
    : null
);

function parseImage(image: string): { name: string; tag: string } {
  const lastColon = image.lastIndexOf(":");
  if (lastColon === -1) return { name: image, tag: "" };
  const possibleTag = image.slice(lastColon + 1);
  if (possibleTag.includes("/")) return { name: image, tag: "" };
  return { name: image.slice(0, lastColon), tag: possibleTag };
}

function buildFullImage(name: string, tag: string): string {
  const n = name.trim();
  const t = tag.trim();
  if (!n) return "";
  return t ? `${n}:${t}` : n;
}

function buildImageSnapshotYaml(
  items: { name: string; currentImage: string; imageName: string; imageTag: string }[],
  useCurrentImage = false
): string {
  const containersYaml = items.map((item) => ({
    name: item.name,
    image: useCurrentImage ? item.currentImage : buildFullImage(item.imageName, item.imageTag),
  }));
  return jsYaml.dump({ containers: containersYaml }, { lineWidth: -1 });
}

function summarizeImages(
  items: { name: string; currentImage: string; imageName: string; imageTag: string }[]
): string {
  if (!items.length) return "镜像快照";
  const preview = items
    .slice(0, 2)
    .map((item) => `${item.name}=${buildFullImage(item.imageName, item.imageTag) || item.currentImage || "-"}`)
    .join(" · ");
  return items.length > 2 ? `${items.length} 个容器 · ${preview} 等` : `${items.length} 个容器 · ${preview}`;
}

function parseContainersFromYaml(yaml: string) {
  const obj = jsYaml.load(yaml) as Record<string, unknown>;
  const template = (obj?.spec as Record<string, unknown>)?.["template"] as Record<string, unknown>;
  const spec = template?.spec as Record<string, unknown>;
  const arr = spec?.containers as { name?: string; image?: string }[] | undefined;
  if (!Array.isArray(arr) || arr.length === 0) {
    throw new Error("无法解析容器信息");
  }
  return arr.map((c) => {
    const img = c.image ?? "";
    const { name, tag } = parseImage(img);
    return {
      name: c.name ?? "",
      currentImage: img,
      imageName: name,
      imageTag: tag,
    };
  });
}

async function fetchAndParse() {
  if (!props.envId || !props.resource) return;
  loading.value = true;
  error.value = null;
  imagePatchInfo.value = null;
  containers.value = [];
  try {
    const yaml = await kubeGetResource(
      props.envId,
      props.resource.kind,
      props.resource.name,
      props.resource.namespace
    );
    resourceYaml.value = yaml;
    containers.value = parseContainersFromYaml(yaml);
  } catch (e) {
    error.value = extractErrorMessage(e);
  } finally {
    loading.value = false;
  }
}

async function applyPatch() {
  if (!props.envId || !props.resource) return;
  const patches = containers.value
    .filter((c) => {
      const full = buildFullImage(c.imageName, c.imageTag);
      return full && full !== c.currentImage;
    })
    .map((c) => ({
      container_name: c.name,
      new_image: buildFullImage(c.imageName, c.imageTag),
    }));
  if (patches.length === 0) {
    emit("close");
    return;
  }
  imagePatchSaving.value = true;
  imagePatchError.value = null;
  imagePatchInfo.value = null;
  try {
    const autoSnapshotEnabled = await ensureAutoSnapshotSettingLoaded();
    if (autoSnapshotEnabled && resourceYaml.value && snapshotResourceRef.value) {
      createResourceSnapshot(snapshotResourceRef.value, {
        yaml: buildImageSnapshotYaml(containers.value, true),
        afterYaml: buildImageSnapshotYaml(containers.value, false),
        category: "image",
        source: "before-image-patch",
        title: "镜像变更快照",
        summary: summarizeImages(containers.value),
      });
    }
    await kubePatchContainerImages(
      props.envId,
      props.resource.kind,
      props.resource.name,
      props.resource.namespace,
      patches
    );
    imagePatchInfo.value = "已自动保存镜像变更前后快照，可在快照中心统一查看。";
    emit("success");
    emit("close");
  } catch (e) {
    imagePatchError.value = extractErrorMessage(e);
  } finally {
    imagePatchSaving.value = false;
  }
}

watch(
  () => [props.visible, props.envId, props.resource?.kind, props.resource?.name, props.resource?.namespace] as const,
  ([visible]) => {
    if (visible && props.envId && props.resource) {
      fetchAndParse();
    } else {
      containers.value = [];
      error.value = null;
      imagePatchError.value = null;
      imagePatchInfo.value = null;
      resourceYaml.value = "";
    }
  },
  { immediate: true }
);
</script>

<template>
  <BaseModal :visible="visible" :title="modalTitle" width="720px" @close="emit('close')">
    <NSpin v-if="loading" size="small" class="change-image-loading">
      <template #description>
        <span class="change-image-loading-text">加载容器信息…</span>
      </template>
    </NSpin>
    <NAlert v-else-if="error" type="error" :show-icon="true" class="change-image-alert">
      {{ error }}
    </NAlert>
    <template v-else>
      <NAlert
        v-if="imagePatchError"
        type="error"
        :show-icon="true"
        class="change-image-alert"
      >
        {{ imagePatchError }}
      </NAlert>
      <NAlert
        v-else-if="imagePatchInfo"
        type="success"
        :show-icon="true"
        class="change-image-alert"
      >
        {{ imagePatchInfo }}
      </NAlert>
      <div class="container-list">
        <NCard
          v-for="c in containers"
          :key="c.name"
          size="small"
          embedded
          :bordered="true"
          class="container-card"
        >
          <div class="container-head">
            <span class="container-name">{{ c.name }}</span>
            <code class="container-current" :title="c.currentImage">{{ c.currentImage || "—" }}</code>
          </div>
          <div class="edit-row">
            <div class="edit-col edit-col-name">
              <label class="edit-label">镜像</label>
              <NInput
                v-model:value="c.imageName"
                placeholder="nginx 或 registry.io/ns/nginx"
                size="small"
              />
            </div>
            <div class="edit-col edit-col-tag">
              <label class="edit-label">Tag</label>
              <NInput v-model:value="c.imageTag" placeholder="1.21" size="small" />
            </div>
          </div>
        </NCard>
      </div>
    </template>
    <template #footer>
      <NButton secondary :disabled="imagePatchSaving" @click="emit('close')">取消</NButton>
      <NButton
        type="primary"
        :loading="imagePatchSaving"
        :disabled="loading || !containers.length"
        @click="applyPatch"
      >
        {{ imagePatchSaving ? "应用中…" : "应用" }}
      </NButton>
    </template>
  </BaseModal>
</template>

<style scoped>
.change-image-loading {
  display: flex;
  justify-content: center;
  padding: 1.25rem 0;
}
.change-image-loading-text {
  color: var(--kf-text-secondary, #64748b);
  font-size: 0.85rem;
}
.change-image-alert {
  margin-bottom: 0.75rem;
}
.container-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  max-height: min(60vh, 520px);
  overflow-y: auto;
  padding-right: 2px;
}
.container-card :deep(.n-card__content) {
  padding: 0.7rem 0.9rem;
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}
.container-head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}
.container-name {
  font-weight: 650;
  font-size: 0.85rem;
  color: var(--kf-text-primary, #0f172a);
}
.container-current {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.78rem;
  color: var(--kf-text-secondary, #64748b);
  background: var(--wb-surface-soft, rgba(241, 245, 249, 0.7));
  border: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
  border-radius: 6px;
  padding: 0.25rem 0.5rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.edit-row {
  display: flex;
  gap: 0.6rem;
  flex-wrap: wrap;
}
.edit-col {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  min-width: 0;
}
.edit-col-name {
  flex: 1 1 220px;
}
.edit-col-tag {
  flex: 0 0 160px;
}
.edit-label {
  font-size: 0.72rem;
  color: var(--kf-text-secondary, #64748b);
  font-weight: 600;
}
</style>
