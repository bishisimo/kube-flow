import { ref } from "vue";
import {
  appSettingsGetAutoSnapshotEnabled,
  appSettingsGetAutoSnapshotLimitPerResource,
  appSettingsGetLogActiveStreamLimit,
  appSettingsGetNodeResourceUsageEnabled,
  appSettingsGetTerminalInstanceCacheLimit,
  appSettingsGetWhitespaceRenderEnabled,
  appSettingsSetAutoSnapshotEnabled,
  appSettingsSetAutoSnapshotLimitPerResource,
  appSettingsSetLogActiveStreamLimit,
  appSettingsSetNodeResourceUsageEnabled,
  appSettingsSetTerminalInstanceCacheLimit,
  appSettingsSetWhitespaceRenderEnabled,
} from "../api/config";

const autoSnapshotEnabled = ref(true);
const autoSnapshotLimitPerResource = ref(10);
const terminalInstanceCacheLimit = ref(6);
const logActiveStreamLimit = ref(3);
const nodeResourceUsageEnabled = ref(false);
const whitespaceRenderEnabled = ref(true);
const loaded = ref(false);

export async function ensureAppSettingsLoaded() {
  if (loaded.value) {
    return {
      autoSnapshotEnabled: autoSnapshotEnabled.value,
      autoSnapshotLimitPerResource: autoSnapshotLimitPerResource.value,
      terminalInstanceCacheLimit: terminalInstanceCacheLimit.value,
      logActiveStreamLimit: logActiveStreamLimit.value,
      nodeResourceUsageEnabled: nodeResourceUsageEnabled.value,
      whitespaceRenderEnabled: whitespaceRenderEnabled.value,
    };
  }
  try {
    const [enabled, limit, cacheLimit, activeLogLimit, nodeUsageEnabled, whitespaceEnabled] = await Promise.all([
      appSettingsGetAutoSnapshotEnabled(),
      appSettingsGetAutoSnapshotLimitPerResource(),
      appSettingsGetTerminalInstanceCacheLimit(),
      appSettingsGetLogActiveStreamLimit(),
      appSettingsGetNodeResourceUsageEnabled(),
      appSettingsGetWhitespaceRenderEnabled(),
    ]);
    autoSnapshotEnabled.value = enabled;
    autoSnapshotLimitPerResource.value = Number.isFinite(limit) ? Math.max(0, Math.floor(limit)) : 10;
    terminalInstanceCacheLimit.value = Number.isFinite(cacheLimit)
      ? Math.min(20, Math.max(1, Math.floor(cacheLimit)))
      : 6;
    logActiveStreamLimit.value = Number.isFinite(activeLogLimit)
      ? Math.min(12, Math.max(1, Math.floor(activeLogLimit)))
      : 3;
    nodeResourceUsageEnabled.value = !!nodeUsageEnabled;
    whitespaceRenderEnabled.value = !!whitespaceEnabled;
  } catch {
    autoSnapshotEnabled.value = true;
    autoSnapshotLimitPerResource.value = 10;
    terminalInstanceCacheLimit.value = 6;
    logActiveStreamLimit.value = 3;
    nodeResourceUsageEnabled.value = false;
    whitespaceRenderEnabled.value = true;
  } finally {
    loaded.value = true;
  }
  return {
    autoSnapshotEnabled: autoSnapshotEnabled.value,
    autoSnapshotLimitPerResource: autoSnapshotLimitPerResource.value,
    terminalInstanceCacheLimit: terminalInstanceCacheLimit.value,
    logActiveStreamLimit: logActiveStreamLimit.value,
    nodeResourceUsageEnabled: nodeResourceUsageEnabled.value,
    whitespaceRenderEnabled: whitespaceRenderEnabled.value,
  };
}

export async function ensureAutoSnapshotSettingLoaded() {
  const settings = await ensureAppSettingsLoaded();
  return settings.autoSnapshotEnabled;
}

export async function setAutoSnapshotEnabled(enabled: boolean) {
  await appSettingsSetAutoSnapshotEnabled(enabled);
  autoSnapshotEnabled.value = enabled;
  loaded.value = true;
}

export async function setAutoSnapshotLimitPerResource(limit: number) {
  const normalized = Math.max(0, Math.floor(Number.isFinite(limit) ? limit : 10));
  await appSettingsSetAutoSnapshotLimitPerResource(normalized);
  autoSnapshotLimitPerResource.value = normalized;
  loaded.value = true;
}

export async function setTerminalInstanceCacheLimit(limit: number) {
  const normalized = Math.min(20, Math.max(1, Math.floor(Number.isFinite(limit) ? limit : 6)));
  await appSettingsSetTerminalInstanceCacheLimit(normalized);
  terminalInstanceCacheLimit.value = normalized;
  loaded.value = true;
}

export async function setLogActiveStreamLimit(limit: number) {
  const normalized = Math.min(12, Math.max(1, Math.floor(Number.isFinite(limit) ? limit : 3)));
  await appSettingsSetLogActiveStreamLimit(normalized);
  logActiveStreamLimit.value = normalized;
  loaded.value = true;
}

export async function setNodeResourceUsageEnabled(enabled: boolean) {
  await appSettingsSetNodeResourceUsageEnabled(enabled);
  nodeResourceUsageEnabled.value = enabled;
  loaded.value = true;
}

export async function setWhitespaceRenderEnabled(enabled: boolean) {
  await appSettingsSetWhitespaceRenderEnabled(enabled);
  whitespaceRenderEnabled.value = enabled;
  loaded.value = true;
}

export function useAppSettingsStore() {
  return {
    autoSnapshotEnabled,
    autoSnapshotLimitPerResource,
    terminalInstanceCacheLimit,
    logActiveStreamLimit,
    nodeResourceUsageEnabled,
    whitespaceRenderEnabled,
    ensureAppSettingsLoaded,
    ensureAutoSnapshotSettingLoaded,
    setAutoSnapshotEnabled,
    setAutoSnapshotLimitPerResource,
    setTerminalInstanceCacheLimit,
    setLogActiveStreamLimit,
    setNodeResourceUsageEnabled,
    setWhitespaceRenderEnabled,
  };
}
