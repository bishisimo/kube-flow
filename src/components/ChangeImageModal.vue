<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as jsYaml from "js-yaml";
import { extractErrorMessage } from "../utils/errorMessage";
import { kubeGetResource, kubePatchContainerImages } from "../api/kube";
import {
  createResourceSnapshot,
} from "../stores/resourceSnapshots";
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

function summarizeImages(items: { name: string; currentImage: string; imageName: string; imageTag: string }[]): string {
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
        yaml: buildImageSnapshotYaml(containers.value),
        category: "image",
        source: "before-image-patch",
        title: "镜像变更前快照",
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
    imagePatchInfo.value = "已自动保存变更前镜像快照，可在快照中心统一查看。";
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
  <Teleport to="body">
    <div v-if="visible" class="modal-overlay">
      <div class="modal-content" role="dialog" aria-labelledby="change-image-title" @click.stop>
        <header class="modal-header">
          <h2 id="change-image-title" class="modal-title">
            修改镜像{{ resource ? ` - ${resource.kind} / ${resource.name}` : "" }}
          </h2>
          <button type="button" class="modal-close" aria-label="关闭" @click="emit('close')">×</button>
        </header>
        <div v-if="loading" class="modal-loading">加载中…</div>
        <div v-else-if="error" class="modal-error">{{ error }}</div>
        <div v-else class="modal-body">
          <div v-if="imagePatchError" class="modal-error-inline">{{ imagePatchError }}</div>
          <div v-else-if="imagePatchInfo" class="modal-info">{{ imagePatchInfo }}</div>
          <div class="modal-edit-area">
            <div v-for="c in containers" :key="c.name" class="container-card">
              <div class="container-name">{{ c.name }}</div>
              <div class="container-current">
                <span class="current-label">当前镜像</span>
                <code class="current-value" :title="c.currentImage">{{ c.currentImage || "—" }}</code>
              </div>
              <div class="container-edit">
                <div class="edit-row">
                  <label class="edit-label">镜像</label>
                  <input
                    v-model="c.imageName"
                    type="text"
                    class="image-input"
                    placeholder="nginx 或 registry.io/ns/nginx"
                  />
                </div>
                <div class="edit-row">
                  <label class="edit-label">Tag</label>
                  <input
                    v-model="c.imageTag"
                    type="text"
                    class="image-input image-tag-input"
                    placeholder="1.21"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
        <div class="modal-actions">
          <button type="button" class="tab-btn" @click="emit('close')">取消</button>
          <button
            type="button"
            class="btn-primary"
            :disabled="loading || imagePatchSaving || !containers.length"
            @click="applyPatch"
          >
            {{ imagePatchSaving ? "应用中…" : "应用" }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  z-index: 1100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
}
.modal-content {
  background: #fff;
  border-radius: 10px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.2);
  min-width: 560px;
  max-width: min(90vw, 720px);
  max-height: 85vh;
  display: flex;
  flex-direction: column;
}
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1.25rem;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.modal-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  color: #1e293b;
}
.modal-close {
  width: 2rem;
  height: 2rem;
  border: none;
  background: none;
  font-size: 1.25rem;
  line-height: 1;
  color: #64748b;
  cursor: pointer;
  border-radius: 6px;
}
.modal-close:hover {
  background: #f1f5f9;
  color: #334155;
}
.modal-loading,
.modal-error {
  padding: 1.25rem 1.5rem;
  font-size: 0.875rem;
}
.modal-error {
  color: #dc2626;
  background: #fef2f2;
}
.modal-body {
  padding: 1rem 1.25rem;
  overflow: auto;
  flex: 1;
  min-height: 0;
}
.modal-edit-area {
  min-width: 0;
}
.modal-error-inline,
.modal-info {
  margin-bottom: 0.9rem;
  padding: 0.75rem 0.9rem;
  border-radius: 10px;
  font-size: 0.8125rem;
}
.modal-error-inline {
  color: #dc2626;
  background: #fef2f2;
}
.modal-info {
  color: #0f766e;
  background: #ecfeff;
}
.container-card {
  padding: 1rem 1.25rem;
  margin-bottom: 0.75rem;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
}
.container-card:last-child {
  margin-bottom: 0;
}
.container-name {
  font-weight: 600;
  font-size: 0.875rem;
  color: #1e293b;
  margin-bottom: 0.5rem;
}
.container-current {
  margin-bottom: 0.75rem;
}
.current-label {
  font-size: 0.75rem;
  color: #64748b;
  display: block;
  margin-bottom: 0.25rem;
}
.current-value {
  display: block;
  font-size: 0.8125rem;
  font-family: ui-monospace, "SF Mono", monospace;
  color: #475569;
  word-break: break-all;
  padding: 0.5rem 0.6rem;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
}
.container-edit {
  display: flex;
  gap: 1rem;
  flex-wrap: wrap;
}
.edit-row {
  flex: 1;
  min-width: 180px;
}
.edit-label {
  display: block;
  font-size: 0.75rem;
  color: #64748b;
  margin-bottom: 0.25rem;
}
.image-input {
  width: 100%;
  padding: 0.5rem 0.6rem;
  font-size: 0.8125rem;
  font-family: ui-monospace, monospace;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  box-sizing: border-box;
}
.image-input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.15);
}
.image-tag-input {
  min-width: 100px;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  padding: 1rem 1.25rem;
  border-top: 1px solid #e2e8f0;
  background: #fff;
  flex-shrink: 0;
}
.tab-btn {
  padding: 0.5rem 1rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
  color: #475569;
}
.tab-btn:hover {
  background: #f8fafc;
}
.btn-primary {
  padding: 0.5rem 1rem;
  border: none;
  border-radius: 6px;
  background: #2563eb;
  font-size: 0.8125rem;
  color: #fff;
  cursor: pointer;
}
.btn-primary:hover:not(:disabled) {
  background: #1d4ed8;
}
.btn-primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

@media (max-width: 960px) {
  .modal-content {
    min-width: 0;
    width: min(96vw, 720px);
  }
  .modal-layout {
    flex-direction: column;
  }
  .modal-edit-area {
    padding-right: 0;
  }
}
</style>
