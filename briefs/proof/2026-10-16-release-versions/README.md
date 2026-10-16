# Proof: releases are versioned by their tag

A release is a pushed tag: the binary reports the tag's version, and nothing in the
repository changes per release. The crate version stays `0.0.0`, so `Cargo.lock`, the
OpenAPI document and the wasm glue no longer change when a version does.

Real values, from this change's builds (`mise run build` and
`CAIRN_VERSION=v0.0.1-test.1 mise run build:release`, the binary taken from the archive's
extracted tarball), with `cairn demo` answering an MCP `initialize`:

| | before (`0.3.0` in `Cargo.toml`) | `mise run build` | `CAIRN_VERSION=v0.0.1-test.1 mise run build:release` |
|---|---|---|---|
| `cairn version` | `cairn 0.3.0` | `cairn dev` | `cairn 0.0.1-test.1` |
| MCP `serverInfo.version` | `0.3.0` | `dev` | `0.0.1-test.1` |
| OpenAPI `info.version` | `0.3.0` | `unversioned` | `unversioned` |
| web build digest (`cairn-build`) | | `9a9b858c...b991` | the same: only `CAIRN_VERSION` differs |

The archive is `cairn-0.0.1-test.1-x86_64-unknown-linux-gnu.tar.gz` with its `.sha256`;
`sha256sum -c` passes, and it holds the binary, `LICENSE` and `README.md` in one folder.

Fail-closed, seen by hand (the semver table is also a unit test):

- Editing `Cargo.toml` to another version fails the wasm crate's build with "the crate
  version stays 0.0.0: a release takes its version from its git tag at build time".
- `CAIRN_VERSION=bogus` fails the build of the `cairn` crate with a message naming the
  value.
- A tag with a pre-release part (`v0.3.1-alpha.1`, `v1.0.0-rc.2`) is created as a GitHub
  pre-release; `v1.0.0` and `v1.0.0+b-5` are not.

## Known limits

- The release jobs, the release page and its assets first run on a pushed tag; none was
  pushed here. The aarch64 leg is unproven until then.
- Archives need the build runner's glibc or newer.
