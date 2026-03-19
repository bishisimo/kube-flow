import { ref } from "vue";
import {
  appSettingsGetAutoSnapshotEnabled,
  appSettingsSetAutoSnapshotEnabled,
} from "../api/config";

const autoSnapshotEnabled = ref(true);
const loaded = ref(false);

export async function ensureAutoSnapshotSettingLoaded() {
  if (loaded.value) return autoSnapshotEnabled.value;
  try {
    autoSnapshotEnabled.value = await appSettingsGetAutoSnapshotEnabled();
  } catch {
    autoSnapshotEnabled.value = true;
  } finally {
    loaded.value = true;
  }
  return autoSnapshotEnabled.value;
}

export async function setAutoSnapshotEnabled(enabled: boolean) {
  await appSettingsSetAutoSnapshotEnabled(enabled);
  autoSnapshotEnabled.value = enabled;
  loaded.value = true;
}

export function useAppSettingsStore() {
  return {
    autoSnapshotEnabled,
    ensureAutoSnapshotSettingLoaded,
    setAutoSnapshotEnabled,
  };
}
