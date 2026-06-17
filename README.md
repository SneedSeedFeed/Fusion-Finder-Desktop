# Fusion Finder

A desktop pokedex for the Pokémon fan game [Infinite Fusion](https://discord.com/invite/infinitefusion), built with Tauri, SvelteKit and caffeine.

# How to use?
On first launch you'll get a page just asking you to select where your game is. Select the directory you have the game installed to and you should see the following.
![Launch page](docs/fusionfinderlaunch.png)
Then just hit "Load game" and you're off to the races.

---
![Fusion finder in action](docs/fusionfinderinaction.png)
Above is the main view, to the right is the filter pane where you can select conditions for which fusions you wish to appear. To the top right is the clear all button and to the left of that is the sort buttons (and the option to switch game).

Clicking a card will bring up the "inspect" view (below)

---
![The inspect view](docs/fusionfinderinspect.png)
In the inspect view you'll find the meat and potatoes of the fusion. How do its stats compare to the base form? What moves does it get access to? Where can you find the components to make it?

# What are all the sort functions for?!?!?!
Glad you asked, let's run through them.

- **Dex order** - Default ordering, it's head major dex order
- **Base Stats** - Order by the given base stat
- **Synergy**: Synergy calculations are focused on finding pokemon that are better than the sum of their parts. All synergy calculations allow you to exclude certain stats you don't care about from synergy calculation.
    - **Sum of Parts** - How much better is this fusion than its base components by just adding the raw value together? Shows the most raw stats gained.
    - **Synergy Ratio** - How much better is this fusion than its base components by percentage?
    - **Surplus vs Best** - How much better is this fusion than its highest stat parent?
    - **Balanced Synergy** - How much better is this fusion than its base components by percentile? Intended to reduce the influence of "extreme" pokemon like Blissey and Shedinja that influence synergy calculations a lot.
- **Effective HP**: How bulky is this pokemon, taking both their defensive stat and HP into account. Values equal spreads between HP and a defense.
    - **Physical eHP** - Product of Defense and HP
    - **Special eHP** - Product of Sp. Defense and HP.
    - **Combined eHP** - Harmonic mean of physical and special effective HP, values pokemon with equal bulk on both sides.
- **Type-adjusted eHP** - The same as Effective HP but modified by how many resistances/immunities/weaknesses a pokemon has.
- **Sweep Score**: A product of a Pokemon's offense and speed. Values high speeds with diminishing returns, floating pokemon with good offense and a solid speed tier to the top like Garchomp and Salamence. Speeds below the 10th percentile score incredibly low as 10 vs 15 speed doesn't matter if you're still getting outsped by 90% of the game. Speeds above the 90th percentile experience diminishing returns to try and punish "too fast" pokemon like Electrode or Regieleki that waste stat points on speed.
    - **Physical Sweep** - Atk x Speed (to find physical sweepers)
    - **Special Sweep** - Sp. Atk x Speed (to find special sweepers)
    - **Combined Sweep** - Maximum attack stat x Speed (to find any sweeper)
    - **Mixed Sweep** - Harmonic mean of attack stats x Speed (to find mixed attackers)
- **Type-Adjusted Sweep**: The same as Sweep Score but modified by how many type combinations its STAB moves can hit super effective/neutral/not very effective.

All of those sort options (referred internally as a `Metric`) can then be selected again to divide the first metric by the second to sort by a ratio. This allows you to find all sorts of pokemon, go absolutely crazy. Maybe some of them combine to find something useful, maybe they don't. All of the metrics individually were intended to surface "hidden gems" and accurately float to the top what I believe are reasonably "meta" pokemon by stats.

## Development
[Tauri docs](https://v2.tauri.app/start/), get 'er installed then you can just do `cargo tauri dev` or your equivalent in `npm`/`bun`/`deno`/`pnpm`.
Tests require a copy of infinite fusion + infinite fusion hoenn, I set it in my `.cargo/config.toml`:
```toml
[env]
INFINITE_FUSION_DIR = "C:\\Users\\YOURNAMEHERE\\WHEREVERYOUHAVEITINSTALLED\\InfiniteFusion"
INFINITE_FUSION_HOENN_DIR = "C:\\Users\\YOURNAMEHERE\\WHEREVERYOUHAVEITINSTALLED\\InfiniteFusion2"
```