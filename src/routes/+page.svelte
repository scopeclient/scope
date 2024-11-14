<script lang="ts">
  import { onMount } from "svelte";
  import { Store } from "@tauri-apps/plugin-store";
  import { goto } from "$app/navigation";

  // todo: dummy page with dummy data. mockups here:
  // https://www.figma.com/design/5BKeaIZWerEF18WGVNBlsI/Scope?node-id=103-15&t=dEKdon9m1HOUlC5b-1

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

    // todo: check local storage for last open path, then
    // goto(/channel/[channel_id])
  });
</script>

<main class="flex items-center justify-center">
  <h1>Scope</h1>
</main>
