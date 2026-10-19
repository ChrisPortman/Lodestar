<script setup lang="ts">
import { RouterView } from 'vue-router'
import AppHeader from './components/AppHeader.vue'
</script>

<template>
  <AppHeader />
  <main class="mx-auto max-w-5xl p-4 sm:p-8">
    <RouterView />
  </main>
</template>
