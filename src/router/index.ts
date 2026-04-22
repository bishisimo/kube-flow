import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "app-shell", component: () => import("../views/AppShell.vue"), meta: { title: "Kube-Flow" } },
  ],
});

router.afterEach((to) => {
  const title = (to.meta?.title as string) ?? "Kube-Flow";
  document.title = title;
});

export default router;
