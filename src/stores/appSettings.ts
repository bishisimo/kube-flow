import { ref } from "vue";
import {
  appSettingsGetAutoSnapshotEnabled,
  appSettingsGetAutoSnapshotLimitPerResource,
  appSettingsSetAutoSnapshotEnabled,
  appSettingsSetAutoSnapshotLimitPerResource,
} from "../api/config";

const autoSnapshotEnabled = ref(true);
const autoSnapshotLimitPerResource = ref(10);
const loaded = ref(false);

export async function ensureAppSettingsLoaded() {
  if (loaded.value) {
    return {
      autoSnapshotEnabled: autoSnapshotEnabled.value,
      autoSnapshotLimitPerResource: autoSnapshotLimitPerResource.value,
    };
  }
  try {
    const [enabled, limit] = await Promise.all([
      appSettingsGetAutoSnapshotEnabled(),
      appSettingsGetAutoSnapshotLimitPerResource(),
    ]);
    autoSnapshotEnabled.value = enabled;
    autoSnapshotLimitPerResource.value = Number.isFinite(limit) ? Math.max(0, Math.floor(limit)) : 10;
  } catch {
    autoSnapshotEnabled.value = true;
    autoSnapshotLimitPerResource.value = 10;
  } finally {
    loaded.value = true;
  }
  return {
    autoSnapshotEnabled: autoSnapshotEnabled.value,
    autoSnapshotLimitPerResource: autoSnapshotLimitPerResource.value,
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

export function useAppSettingsStore() {
  return {
    autoSnapshotEnabled,
    autoSnapshotLimitPerResource,
    ensureAppSettingsLoaded,
    ensureAutoSnapshotSettingLoaded,
    setAutoSnapshotEnabled,
    setAutoSnapshotLimitPerResource,
  };
}
