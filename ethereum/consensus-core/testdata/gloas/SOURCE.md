Gloas light-client sync vectors and the Gloas fork/legacy-data cases in the
sibling fork directories come from `minimal.tar.gz` in consensus-specs
[v1.7.0-beta.2](https://github.com/ethereum/consensus-specs/releases/tag/v1.7.0-beta.2).
The files are copied unchanged from `tests/minimal/<fork>/light_client/sync/pyspec_tests/`.

Archive SHA-256: `86645aa51423de5dadcc279c785c1f0d03b0e07e054232c7f36ce0e3ec7ffa37`.

To run every sync case from Capella through Gloas in the release:

```sh
CONSENSUS_SPEC_TESTS=/path/to/extracted/tests cargo test -p helios-consensus-core --test sync official_sync_vectors -- --ignored
```

The release generates sync scenarios only for the minimal preset. Mainnet
uses the same verification code with a larger sync committee.

Gloas SSZ decoding vectors for both presets can also be checked after extracting
`mainnet.tar.gz` alongside `minimal.tar.gz`:

```sh
CONSENSUS_SPEC_TESTS=/path/to/extracted/tests cargo test -p helios-consensus-core --test ssz -- --ignored
```

Mainnet archive SHA-256: `0047b48f19fe6f74114291a46d0a16804871338ebec7166de6f3e4690da4313b`.

To run sync, SSZ decoding, and update-ranking vectors together:

```sh
CONSENSUS_SPEC_TESTS=/path/to/extracted/tests cargo test -p helios-consensus-core -- --include-ignored
```
