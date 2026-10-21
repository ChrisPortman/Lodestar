<script setup lang="ts">
// PROTOTYPE (#75): picks the Meetings variant from ?variant=. Throwaway.
import type { Project } from '../../api/client'
import { useVariant } from './meetingStub.prototype'
import VariantA from './VariantA.prototype.vue'
import VariantB from './VariantB.prototype.vue'
import VariantC from './VariantC.prototype.vue'

defineProps<{ project: Project; place: 'panel' | 'page' }>()
const emit = defineEmits<{ close: [] }>()
const variant = useVariant()
</script>

<template>
  <template v-if="variant === 'A'">
    <VariantA v-if="place === 'panel'" :project="project" />
  </template>
  <VariantB v-else-if="variant === 'B'" :project="project" :place="place" @close="emit('close')" />
  <VariantC v-else :project="project" :place="place" @close="emit('close')" />
</template>
