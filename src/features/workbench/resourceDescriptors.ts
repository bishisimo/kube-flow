import type { ResourceKind } from "../../constants/resourceKinds";
import {
  RESOURCE_REGISTRY,
  fetchResourceList,
  resourceIsClusterScoped,
  resourceSupportsWatch,
} from "../../resources/resourceRegistry";
import type { WorkbenchResourceDescriptor } from "./contracts";

const IP_FILTER_KINDS = new Set<ResourceKind>(["pods", "services"]);

function createDescriptor(kind: ResourceKind): WorkbenchResourceDescriptor {
  return {
    kind,
    capabilities: {
      supportsWatch: resourceSupportsWatch(kind),
      clusterScoped: resourceIsClusterScoped(kind),
      supportsIpFilter: IP_FILTER_KINDS.has(kind),
    },
    fetchList: (envId, namespace, labelSelector) => fetchResourceList(kind, envId, namespace, labelSelector),
  };
}

const DESCRIPTORS = Object.keys(RESOURCE_REGISTRY).reduce(
  (acc, key) => {
    const kind = key as ResourceKind;
    acc[kind] = createDescriptor(kind);
    return acc;
  },
  {} as Record<ResourceKind, WorkbenchResourceDescriptor>
);

export function getWorkbenchResourceDescriptor(kind: ResourceKind): WorkbenchResourceDescriptor {
  return DESCRIPTORS[kind];
}
