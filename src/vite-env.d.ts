interface ImportMetaEnv {
  readonly VITE_CONSTRUCT_CHANNEL?: "dev" | "release";
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
