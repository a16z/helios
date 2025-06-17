/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_ALCHEMY_KEY: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}