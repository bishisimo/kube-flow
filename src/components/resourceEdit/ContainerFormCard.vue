<script setup lang="ts">
import { NInput, NInputNumber, NSelect } from "naive-ui";
import KeyValueListEditor from "./KeyValueListEditor.vue";
import ItemTabsEditor from "./ItemTabsEditor.vue";
import type { ContainerDraft, PortDraft, VolumeMountDraft } from "../../features/resourceEdit/workloadDraft";

const props = defineProps<{
  container: ContainerDraft;
  index: number;
  init?: boolean;
  /** 在 Tab 面板内展示时隐藏卡片顶栏 */
  embedded?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:container", value: ContainerDraft): void;
  (e: "remove"): void;
}>();

function patch(partial: Partial<ContainerDraft>) {
  emit("update:container", { ...props.container, ...partial });
}

function createEmptyPort(): PortDraft {
  return { name: "", containerPort: null, protocol: "TCP" };
}

function createEmptyMount(): VolumeMountDraft {
  return { name: "", mountPath: "", readOnly: false };
}

function portLabel(port: PortDraft, index: number) {
  if (port.name.trim()) return port.name.trim();
  if (port.containerPort != null) return `:${port.containerPort}`;
  return `Port ${index + 1}`;
}

function mountLabel(mount: VolumeMountDraft, index: number) {
  if (mount.name.trim()) return mount.name.trim();
  if (mount.mountPath.trim()) return mount.mountPath.trim();
  return `Mount ${index + 1}`;
}
</script>

<template>
  <div class="re-container-card" :class="{ 're-container-card--init': init, 're-container-card--embedded': embedded }">
    <div v-if="!embedded" class="re-container-head">
      <span class="re-container-badge">{{ init ? "Init" : "Container" }}</span>
      <span class="re-container-name">{{ container.name || `容器 ${index + 1}` }}</span>
    </div>

    <div class="re-container-body">
      <div class="re-field-grid re-field-grid--2">
        <label class="re-field">
          <span class="re-label">名称</span>
          <NInput :value="container.name" size="small" @update:value="patch({ name: $event })" />
        </label>
        <label class="re-field">
          <span class="re-label">imagePullPolicy</span>
          <NSelect
            :value="container.imagePullPolicy"
            size="small"
            :options="[
              { label: 'IfNotPresent', value: 'IfNotPresent' },
              { label: 'Always', value: 'Always' },
              { label: 'Never', value: 'Never' },
            ]"
            @update:value="patch({ imagePullPolicy: $event })"
          />
        </label>
      </div>

      <label class="re-field">
        <span class="re-label">镜像</span>
        <NInput :value="container.image" size="small" spellcheck="false" @update:value="patch({ image: $event })" />
      </label>

      <div class="re-field-grid re-field-grid--2">
        <label class="re-field">
          <span class="re-label">Command（每行一条）</span>
          <NInput
            :value="container.commandText"
            type="textarea"
            :rows="2"
            spellcheck="false"
            @update:value="patch({ commandText: $event })"
          />
        </label>
        <label class="re-field">
          <span class="re-label">Args（每行一条）</span>
          <NInput
            :value="container.argsText"
            type="textarea"
            :rows="2"
            spellcheck="false"
            @update:value="patch({ argsText: $event })"
          />
        </label>
      </div>

      <KeyValueListEditor
        title="环境变量"
        :pairs="container.env.map((e) => ({ key: e.name, value: e.value }))"
        @update:pairs="patch({ env: $event.map((p) => ({ name: p.key, value: p.value })) })"
      />

      <div class="re-block">
        <div class="re-block-title">Resources</div>
        <div class="re-field-grid re-field-grid--2">
          <label class="re-field">
            <span class="re-label">CPU Request</span>
            <NInput :value="container.cpuRequest" size="small" @update:value="patch({ cpuRequest: $event })" />
          </label>
          <label class="re-field">
            <span class="re-label">Memory Request</span>
            <NInput :value="container.memoryRequest" size="small" @update:value="patch({ memoryRequest: $event })" />
          </label>
          <label class="re-field">
            <span class="re-label">CPU Limit</span>
            <NInput :value="container.cpuLimit" size="small" @update:value="patch({ cpuLimit: $event })" />
          </label>
          <label class="re-field">
            <span class="re-label">Memory Limit</span>
            <NInput :value="container.memoryLimit" size="small" @update:value="patch({ memoryLimit: $event })" />
          </label>
        </div>
      </div>

      <div class="re-block">
        <div class="re-block-title">Ports</div>
        <ItemTabsEditor
          :items="container.ports"
          variant="port"
          compact
          empty-hint="未配置端口"
          :create-item="createEmptyPort"
          :get-label="portLabel"
          add-label="添加端口"
          @update:items="patch({ ports: $event })"
        >
          <template #default="{ item, update }">
            <div class="re-item-panel re-item-panel--compact">
              <label class="re-field">
                <span class="re-label">name</span>
                <NInput
                  :value="item.name"
                  size="small"
                  placeholder="http"
                  @update:value="update({ ...item, name: $event })"
                />
              </label>
              <div class="re-field-grid re-field-grid--port-meta">
                <label class="re-field">
                  <span class="re-label">containerPort</span>
                  <NInputNumber
                    :value="item.containerPort"
                    size="small"
                    :min="1"
                    class="re-input-full"
                    @update:value="update({ ...item, containerPort: $event })"
                  />
                </label>
                <label class="re-field">
                  <span class="re-label">protocol</span>
                  <NSelect
                    :value="item.protocol"
                    size="small"
                    :options="[
                      { label: 'TCP', value: 'TCP' },
                      { label: 'UDP', value: 'UDP' },
                    ]"
                    @update:value="update({ ...item, protocol: $event })"
                  />
                </label>
              </div>
            </div>
          </template>
        </ItemTabsEditor>
      </div>

      <div class="re-block">
        <div class="re-block-title">Volume Mounts</div>
        <ItemTabsEditor
          :items="container.volumeMounts"
          variant="mount"
          compact
          empty-hint="未配置挂载"
          :create-item="createEmptyMount"
          :get-label="mountLabel"
          add-label="添加挂载"
          @update:items="patch({ volumeMounts: $event })"
        >
          <template #default="{ item, update }">
            <div class="re-item-panel re-item-panel--compact">
              <label class="re-field">
                <span class="re-label">volume</span>
                <NInput
                  :value="item.name"
                  size="small"
                  placeholder="data"
                  @update:value="update({ ...item, name: $event })"
                />
              </label>
              <label class="re-field">
                <span class="re-label">mountPath</span>
                <NInput
                  :value="item.mountPath"
                  size="small"
                  placeholder="/data"
                  @update:value="update({ ...item, mountPath: $event })"
                />
              </label>
              <label class="re-field">
                <span class="re-label">readOnly</span>
                <NSelect
                  :value="item.readOnly ? 'true' : 'false'"
                  size="small"
                  :options="[
                    { label: 'RW', value: 'false' },
                    { label: 'RO', value: 'true' },
                  ]"
                  @update:value="update({ ...item, readOnly: $event === 'true' })"
                />
              </label>
            </div>
          </template>
        </ItemTabsEditor>
      </div>
    </div>
  </div>
</template>

<style src="./resourceEditUi.css"></style>
