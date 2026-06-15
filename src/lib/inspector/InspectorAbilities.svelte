<script lang="ts">
  import type { FusionDetail } from "$lib/bindings";

  // Regular and hidden abilities (merged across both parents), with descriptions.
  let { detail }: { detail: FusionDetail } = $props();
</script>

<section class="grid gap-4 sm:grid-cols-2">
  {#each [{ title: "Abilities", list: detail.regular_abilities }, { title: "Hidden", list: detail.hidden_abilities }] as col (col.title)}
    {#if col.list.length}
      <div>
        <h3 class="mb-1 border-b border-gray-800 pb-1 font-semibold">
          {col.title}
        </h3>
        <ul class="space-y-1 text-sm">
          {#each col.list as a, ai (ai)}
            <li>
              <span class="font-medium">{a.name}</span>
              <span class="text-gray-500">- {a.description}</span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  {/each}
</section>
