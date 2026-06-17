<script setup lang="ts">
import { NButton, NCollapse, NCollapseItem, NInput, NInputNumber, NSelect } from "naive-ui";
import KeyValueListEditor from "./KeyValueListEditor.vue";
import type { ContainerDraft } from "../../features/resourceEdit/workloadDraft";

const props = defineProps<{
  container: ContainerDraft;
  index: number;
  init?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:container", value: ContainerDraft): void;
  (e: "remove"): void;
}>();

function patch(partial: Partial<ContainerDraft>) {
  emit("update:container", { ...props.container, ...partial });
}

function addPort() {
  patch({
    ports: [...props.container.ports, { name: "", containerPort: null, protocol: "TCP" }],
  });
}

function removePort(i: number) {
  patch({ ports: props.container.ports.filter((_, idx) => idx !== i) });
}

function updatePort(i: number, field: "name" | "containerPort" | "protocol", value: string | number | null) {
  const ports = props.container.ports.map((p, idx) =>
    idx === i ? { ...p, [field]: value } : p,
  );
  patch({ ports });
}

function addMount() {
  patch({
    volumeMounts: [...props.container.volumeMounts, { name: "", mountPath: "", readOnly: false }],
  });
}

function removeMount(i: number) {
  patch({ volumeMounts: props.container.volumeMounts.filter((_, idx) => idx !== i) });
}

function updateMount(
  i: number,
  field: "name" | "mountPath" | "readOnly",
  value: string | boolean,
) {
  const volumeMounts = props.container.volumeMounts.map((m, idx) =>
    idx === i ? { ...m, [field]: value } : m,
  );
  patch({ volumeMounts });
}
</script>

<template>
  <NCollapse>
    <NCollapseItem :title="`${init ? 'Init ' : ''}容器 ${index + 1}: ${container.name || '(未命名)'}`" :name="String(index)">
      <template #header-extra>
        <NButton text type="error" size="tiny" @click.stop="emit('remove')">删除</NButton>
      </template>
      <div class="container-form">
        <label class="field">
          <span class="label">名称</span>
          <NInput :value="container.name" size="small" @update:value="patch({ name: $event })" />
        </label>
        <label class="field">
          <span class="label">镜像</span>
          <NInput :value="container.image" size="small" spellcheck="false" @update:value="patch({ image: $event })" />
        </label>
        <label class="field">
          <span class="label">imagePullPolicy</span>
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
        <label class="field">
          <span class="label">Command（每行一条）</span>
          <NInput
            :value="container.commandText"
            type="textarea"
            :rows="2"
            spellcheck="false"
            @update:value="patch({ commandText: $event })"
          />
        </label>
        <label class="field">
          <span class="label">Args（每行一条）</span>
          <NInput
            :value="container.argsText"
            type="textarea"
            :rows="2"
            spellcheck="false"
            @update:value="patch({ argsText: $event })"
          />
        </label>

        <KeyValueListEditor
          title="环境变量 (name / value)"
          :pairs="container.env.map((e) => ({ key: e.name, value: e.value }))"
          @update:pairs="patch({ env: $event.map((p) => ({ name: p.key, value: p.value })) })"
        />

        <div class="sub-grid">
          <label class="field">
            <span class="label">CPU Request</span>
            <NInput :value="container.cpuRequest" size="small" @update:value="patch({ cpuRequest: $event })" />
          </label>
          <label class="field">
            <span class="label">Memory Request</span>
            <NInput :value="container.memoryRequest" size="small" @update:value="patch({ memoryRequest: $event })" />
          </label>
          <label class="field">
            <span class="label">CPU Limit</span>
            <NInput :value="container.cpuLimit" size="small" @update:value="patch({ cpuLimit: $event })" />
          </label>
          <label class="field">
            <span class="label">Memory Limit</span>
            <NInput :value="container.memoryLimit" size="small" @update:value="patch({ memoryLimit: $event })" />
          </label>
        </div>

        <div class="list-block">
          <div class="list-head">
            <span class="label">Ports</span>
            <NButton size="tiny" quaternary @click="addPort">+ 添加</NButton>
          </div>
          <div v-for="(port, pi) in container.ports" :key="pi" class="list-row">
            <NInput :value="port.name" size="small" placeholder="name" @update:value="updatePort(pi, 'name', $event)" />
            <NInputNumber
              :value="port.containerPort"
              size="small"
              :min="1"
              placeholder="port"
              @update:value="updatePort(pi, 'containerPort', $event)"
            />
            <NSelect
              :value="port.protocol"
              size="small"
              :options="[
                { label: 'TCP', value: 'TCP' },
                { label: 'UDP', value: 'UDP' },
              ]"
              @update:value="updatePort(pi, 'protocol', $event)"
            />
            <NButton text type="error" size="tiny" @click="removePort(pi)">×</NButton>
          </div>
        </div>

        <div class="list-block">
          <div class="list-head">
            <span class="label">Volume Mounts</span>
            <NButton size="tiny" quaternary @click="addMount">+ 添加</NButton>
          </div>
          <div v-for="(mount, mi) in container.volumeMounts" :key="mi" class="list-row mounts">
            <NInput :value="mount.name" size="small" placeholder="volume name" @update:value="updateMount(mi, 'name', $event)" />
            <NInput :value="mount.mountPath" size="small" placeholder="mountPath" @update:value="updateMount(mi, 'mountPath', $event)" />
            <NSelect
              :value="mount.readOnly ? 'true' : 'false'"
              size="small"
              :options="[
                { label: 'RW', value: 'false' },
                { label: 'RO', value: 'true' },
              ]"
              @update:value="updateMount(mi, 'readOnly', $event === 'true')"
            />
            <NButton text type="error" size="tiny" @click="removeMount(mi)">×</NButton>
          </div>
        </div>
      </div>
    </NCollapseItem>
  </NCollapse>
</template>

<style scoped>
.container-form {
  display: grid;
  gap: 0.75rem;
}
.field {
  display: grid;
  gap: 0.3rem;
}
.label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--kf-text-secondary);
}
.sub-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
}
.list-block {
  display: grid;
  gap: 0.5rem;
}
.list-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.list-row {
  display: grid;
  grid-template-columns: 1fr 100px 90px auto;
  gap: 0.5rem;
  align-items: center;
}
.list-row.mounts {
  grid-template-columns: 1fr 1fr 80px auto;
}
</style>
