import { ref } from "vue";
import { strongholdGetStatus, type StrongholdStatus } from "../api/credential";

const strongholdStatus = ref<StrongholdStatus>("uninitialized");

async function refreshStrongholdStatus() {
  strongholdStatus.value = await strongholdGetStatus();
  return strongholdStatus.value;
}

function setStrongholdStatus(status: StrongholdStatus) {
  strongholdStatus.value = status;
}

export function useStrongholdStatusStore() {
  return {
    strongholdStatus,
    refreshStrongholdStatus,
    setStrongholdStatus,
  };
}
