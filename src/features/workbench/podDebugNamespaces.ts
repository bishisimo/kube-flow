import type { PodDebugNamespace } from "../../api/terminal";

export type PodDebugNamespaceOption = {
  value: PodDebugNamespace;
  label: string;
  description: string;
  recommended?: boolean;
};

/** Pod 调试侧可选 namespace 说明，供工作台 Pod 调试弹窗使用。 */
export const WORKBENCH_POD_DEBUG_NAMESPACE_OPTIONS: PodDebugNamespaceOption[] = [
  {
    value: "net",
    label: "网络",
    description: "保留主机工具，排查连接、路由、DNS、端口与抓包。",
    recommended: true,
  },
  { value: "pid", label: "进程", description: "观察容器进程视图，配合 ps、lsof、ss 做进程级排障。" },
  { value: "mnt", label: "挂载", description: "查看容器文件系统与挂载点，适合卷与配置文件排查。" },
  { value: "uts", label: "主机名", description: "进入容器 UTS 环境，确认 hostname 与域名行为。" },
  { value: "ipc", label: "IPC", description: "排查共享内存、信号量等 IPC 相关问题。" },
];
