<script lang="ts">
  import { onMount } from "svelte";
  import { Store } from "@tauri-apps/plugin-store";
  import { goto } from "$app/navigation";

  // todo:
  // secure token access and storage will be implemented with:
  // https://github.com/tauri-apps/plugins-workspace/tree/v1/plugins/stronghold

  onMount(async () => {
    const store = await Store.load("settings.json");
    const result = await store.get<{ value: string }>("token");

    if (!result) {
      goto("/login");
      return;
    }

    // todo: check login token validity, if invalid go to login

    if (!result.value) {
      goto("/login");
      return;
    }

    console.log("token", result.value);
  });
</script>

<main class="flex items-center justify-center">
  <h1>Scope</h1>
</main>
