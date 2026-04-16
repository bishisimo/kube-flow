import type { ResourceKind } from "../../constants/resourceKinds";

export interface SelectedResource {
  kind: string;
  name: string;
  namespace: string | null;
  /** 动态 API 资源（如 CRD）：走专用 get/describe。 */
  dynamic?: { api_version: string; namespaced: boolean };
}

export type WorkbenchResourceCapabilities = {
  supportsWatch: boolean;
  clusterScoped: boolean;
  supportsIpFilter: boolean;
};

export type WorkbenchResourceDescriptor = {
  kind: ResourceKind;
  capabilities: WorkbenchResourceCapabilities;
  fetchList: (
    envId: string,
    namespace: string | null,
    labelSelector: string | null
  ) => Promise<unknown[]>;
};
