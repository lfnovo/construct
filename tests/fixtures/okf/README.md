# OKF compatibility fixtures

These synthetic fixtures define Construct's tolerant OKF consumption contract.
They contain no user data and are shared by native parser, bundle, and future
indexing tests.

Add a new directory when a specification version or compatibility case changes
the expected interpretation, then register document-level expectations in
`cases.json`. The native table-driven contract test will include it
automatically. Keep each fixture small and cover:

- the original YAML value as written;
- the normalized fields Construct consumes;
- stable finding codes for malformed or unsupported input;
- internal links that should and should not enter the graph.

The fixture bundles are inputs only. Tests must never rewrite them.

Run the compatibility suite with:

```bash
cargo test --manifest-path src-tauri/Cargo.toml okf::tests
```

Run the opt-in 10,000-document capacity probe with:

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  okf::tests::parses_10k_documents -- --ignored --nocapture
```
