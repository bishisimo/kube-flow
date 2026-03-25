import { ref } from "vue";

type LogStreamController = {
  setStreamAllowed: (allowed: boolean) => void | Promise<void>;
};

const controllers = new Map<string, LogStreamController>();
const activationOrder = ref<string[]>([]);
const activeLimit = ref(3);

function reconcile() {
  const allowed = new Set(activationOrder.value.slice(0, activeLimit.value));
  for (const [sessionId, controller] of controllers.entries()) {
    void controller.setStreamAllowed(allowed.has(sessionId));
  }
}

export function registerLogStreamSession(sessionId: string, controller: LogStreamController) {
  controllers.set(sessionId, controller);
  if (!activationOrder.value.includes(sessionId)) {
    activationOrder.value = [sessionId, ...activationOrder.value];
  }
  reconcile();
}

export function unregisterLogStreamSession(sessionId: string) {
  controllers.delete(sessionId);
  activationOrder.value = activationOrder.value.filter((id) => id !== sessionId);
  reconcile();
}

export function touchLogStreamSession(sessionId: string | null) {
  if (!sessionId) return;
  activationOrder.value = [sessionId, ...activationOrder.value.filter((id) => id !== sessionId)];
  reconcile();
}

export function setLogStreamActiveLimit(limit: number) {
  activeLimit.value = Math.min(12, Math.max(1, Math.floor(Number.isFinite(limit) ? limit : 3)));
  reconcile();
}
