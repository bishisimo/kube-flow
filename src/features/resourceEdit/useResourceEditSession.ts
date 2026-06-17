import { ref, computed, watch, type Ref } from "vue";
import * as jsYaml from "js-yaml";
import { useMessage } from "naive-ui";
import { kubeApplyResource } from "../../api/kube";
import { extractErrorMessage } from "../../utils/errorMessage";
import { stripManagedFields } from "../../utils/yaml";
import {
  createResourceSnapshot,
  type ResourceSnapshotRef,
} from "../../stores/resourceSnapshots";
import { ensureAutoSnapshotSettingLoaded } from "../../stores/appSettings";
import type { SelectedResource } from "../workbench/contracts";
import { resolveEditProfile, resolveEditability } from "./registry";
import type { EditIntent, K8sObject } from "./types";

export interface UseResourceEditSessionOptions {
  envId: Ref<string | null>;
  resource: Ref<SelectedResource | null>;
  rawYaml: Ref<string>;
  initialIntent?: Ref<EditIntent | null | undefined>;
  onRefresh: () => Promise<void>;
  onStrongholdLocked: (message: string, retry: () => void) => Promise<boolean>;
}

export function useResourceEditSession(options: UseResourceEditSessionOptions) {
  const message = useMessage();

  const configYaml = ref("");
  const saving = ref(false);
  const error = ref<string | null>(null);

  const parsedObject = computed<K8sObject | null>(() => {
    const yaml = options.rawYaml.value;
    if (!yaml.trim()) return null;
    try {
      const obj = jsYaml.load(yaml) as K8sObject | null;
      return obj && typeof obj === "object" ? obj : null;
    } catch {
      return null;
    }
  });

  const profile = computed(() => {
    const kind = options.resource.value?.kind ?? "";
    return resolveEditProfile(kind);
  });

  const editability = computed(() => {
    const resource = options.resource.value;
    if (!resource) return { allowed: true, structuredAllowed: true };
    return resolveEditability({ resource, obj: parsedObject.value });
  });

  const snapshotRef = computed<ResourceSnapshotRef | null>(() => {
    const envId = options.envId.value;
    const resource = options.resource.value;
    if (!envId || !resource) return null;
    return {
      env_id: envId,
      resource_kind: resource.kind,
      resource_name: resource.name,
      resource_namespace: resource.namespace ?? null,
    };
  });

  function resetConfigYaml() {
    configYaml.value = stripManagedFields(options.rawYaml.value);
  }

  watch(
    () => options.rawYaml.value,
    (yaml) => {
      if (!yaml) return;
      resetConfigYaml();
    },
    { immediate: true },
  );

  async function snapshotBeforeApply(category: "resource" | "config") {
    const autoSnapshotEnabled = await ensureAutoSnapshotSettingLoaded();
    if (!autoSnapshotEnabled || !snapshotRef.value) return;
    const snapshotYaml = options.rawYaml.value.trim();
    if (!snapshotYaml) return;
    createResourceSnapshot(snapshotRef.value, {
      yaml: snapshotYaml,
      category,
      source: "before-apply",
      title: category === "config" ? "应用前配置快照" : "应用前资源快照",
    });
  }

  async function applyFullYaml(yamlOverride?: string) {
    const yaml = (yamlOverride ?? configYaml.value).trim();
    const envId = options.envId.value;
    const resource = options.resource.value;
    if (!envId || !resource || !yaml) return;

    saving.value = true;
    error.value = null;
    try {
      await snapshotBeforeApply(profile.value.snapshotCategory);
      await kubeApplyResource(envId, yaml);
      await options.onRefresh();
      resetConfigYaml();
      message.success("资源已保存");
    } catch (e) {
      const msg = extractErrorMessage(e);
      const locked = await options.onStrongholdLocked(msg, () => {
        void applyFullYaml(yaml);
      });
      if (locked) return;
      error.value = msg;
      message.error(msg);
    } finally {
      saving.value = false;
    }
  }

  function setConfigYaml(yaml: string) {
    configYaml.value = yaml;
  }

  function handleEditorError(msg: string) {
    error.value = msg;
    message.error(msg);
  }

  return {
    configYaml,
    saving,
    error,
    parsedObject,
    profile,
    editability,
    resetConfigYaml,
    applyFullYaml,
    setConfigYaml,
    handleEditorError,
  };
}
