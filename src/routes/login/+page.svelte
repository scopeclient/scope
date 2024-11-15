<script lang="ts">
  import { goto } from "$app/navigation";
  import type { Store as StoreType } from "@tauri-apps/plugin-store";
  import { onMount } from "svelte";
  import { Store } from "@tauri-apps/plugin-store";

  // login mockups in the top left here:
  // https://www.figma.com/design/5BKeaIZWerEF18WGVNBlsI/Scope?node-id=103-15&t=dEKdon9m1HOUlC5b-1

  let tokenInput = $state("");
  let store: StoreType;

  onMount(async () => {
    // todo: ecure token access and storage will be implemented with:
    // https://github.com/tauri-apps/plugins-workspace/tree/v1/plugins/stronghold

    store = await Store.load("settings.json");
    // this file is in the app bundle.
    // on Mac its ~/Library/Application Suppport/com.scope
  });
</script>

<main class="w-full h-full flex">
  <div class="flex flex-col items-center m-auto">
    <input placeholder="auth token" bind:value={tokenInput} type="text" />
    <button
      class="bg-white mt-2 text-dark rounded-md px-3 py-1.5 w-full"
      onclick={async () => {
        await store.set("token", { value: tokenInput });
        store.save();
        goto("/");
      }}>Continue</button
    >
  </div>
</main>
