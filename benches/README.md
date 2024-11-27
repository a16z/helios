# Helios Benchmarking

Helios performance is measured using [criterion](https://github.com/bheisler/criterion.rs) for comprehensive statistics-driven benchmarking.

Benchmarks are defined in the [benches](./) subdirectory and can be run using the cargo `bench` subcommand (e.g. `cargo bench`). To run a specific benchmark, you can use `cargo bench --bench <name>`, where `<name>` is one of the benchmarks defined in the [Cargo.toml](./Cargo.toml) file under a `[[bench]]` section.


#### Flamegraphs

[Flamegraph](https://github.com/brendangregg/FlameGraph) is a powerful rust crate for generating profile visualizations, which graphs the time a program spends in each function. Functions called during execution are displayed as horizontal rectangles with the width proportional to the time spent in that function. As the call stack grows (think nested function invocations), the rectangles are stacked vertically. This provides a powerful visualization for quickly understanding which parts of a codebase take up disproportionate amounts of time.

Check out Brendan Gregg's [Flame Graphs](http://www.brendangregg.com/flamegraphs.html) blog post if you're interested in learning more about flamegraphs and performance visualizations in general.

To generate a flamegraph for helios, you can use the `cargo flamegraph` subcommand. For example, to generate a flamegraph for the [`client`](./examples/client.rs) example, you can run:

```bash
cargo flamegraph --example client -o ./flamegraphs/client.svg
```
